use adw::prelude::*;
use gtk::{gdk, glib, pango};

use crate::config;
use crate::i18n::tr;
use crate::log;
use crate::util::guard;
use crate::pane::system_monospace_font;
use crate::settings;

thread_local! {
    static CAPTURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    static LABELS: std::cell::RefCell<Vec<(String, glib::WeakRef<gtk::Label>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn refresh_labels() {
    LABELS.with(|l| for (action, label) in l.borrow().iter() {
        if let Some(label) = label.upgrade() {
            label.set_text(&accel_text(&settings::accels(action)));
        }
    });
}

pub fn capturing() -> bool {
    CAPTURING.get()
}

fn accel_text(accels: &[String]) -> String {
    let labels: Vec<String> = accels.iter()
        .filter_map(gtk::accelerator_parse)
        .map(|(k, m)| gtk::accelerator_get_label(k, m).to_string())
        .collect();
    if labels.is_empty() { tr("Disabled") } else { labels.join(&tr(" or ")) }
}

fn acceptable(key: gdk::Key, mods: gdk::ModifierType) -> bool {
    use gtk::glib::translate::IntoGlib;
    let f_key = (gdk::Key::F1.into_glib()..=gdk::Key::F35.into_glib()).contains(&key.into_glib());
    f_key || mods.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK
                             | gdk::ModifierType::SUPER_MASK)
}

fn capture_shortcut(win: &gtk::Window, parent: &impl IsA<gtk::Widget>, label: &str,
                    done: impl Fn(Option<String>) + 'static) {
    let dialog = adw::AlertDialog::new(
        Some(&tr("New Shortcut")),
        Some(&tr("Press the key combination for “{action}”.\n\nEscape: cancel · Backspace: disable").replace("{action}", label)));
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.set_close_response("cancel");
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let d = dialog.clone();
    let done = std::rc::Rc::new(done);
    keys.connect_key_pressed(move |ctrl, key, code, state| guard("shortcut capture", glib::Propagation::Stop, || {
        if ctrl.current_event().and_then(|e| e.downcast::<gdk::KeyEvent>().ok()).is_some_and(|e| e.is_modifier()) {
            return glib::Propagation::Proceed;
        }
        let mods = state & gtk::accelerator_get_default_mod_mask();
        let base = crate::util::base_key(ctrl, key, code);
        if mods.is_empty() && base == gdk::Key::Escape {
            d.close();
        } else if mods.is_empty() && base == gdk::Key::BackSpace {
            done(None);
            d.close();
        } else if acceptable(base, mods) {
            done(Some(gtk::accelerator_name(base, mods).to_string()));
            d.close();
        } else {
            d.set_body(&tr("Use Ctrl, Alt or Super with the key, or a function key (F1…F12).\n\nEscape: cancel · Backspace: disable"));
            if let Some(w) = ctrl.widget() { w.error_bell(); }
        }
        glib::Propagation::Stop
    }));
    dialog.add_controller(keys);
    CAPTURING.set(true);
    crate::panel::mute_window_shortcuts(win, true);
    let w = win.clone();
    dialog.connect_closed(move |_| {
        CAPTURING.set(false);
        guard("shortcut capture", (), || crate::panel::mute_window_shortcuts(&w, false));
    });
    dialog.present(Some(parent));
}

fn colors_group(s: &settings::Settings) -> adw::PreferencesGroup {
    use crate::colors::{self, CUSTOM, FOLLOW_TERMINAL, NAUTILUS};
    let group = adw::PreferencesGroup::builder().title(tr("Colors")).build();

    let styles = ["system", "light", "dark"];
    let style = adw::ComboRow::builder()
        .title(tr("Style"))
        .model(&gtk::StringList::new(&[&tr("Follow System"), &tr("Light"), &tr("Dark")]))
        .selected(styles.iter().position(|v| *v == s.style).unwrap_or(0) as u32)
        .build();
    style.connect_selected_notify(move |r| {
        let v = styles.get(r.selected() as usize).unwrap_or(&"system").to_string();
        settings::update(|s| s.style = v);
    });
    group.add(&style);

    let mut ids: Vec<String> = vec![FOLLOW_TERMINAL.into(), NAUTILUS.into()];
    let mut names: Vec<String> = vec![tr("Same as My Terminal"), tr("Same as Nautilus")];
    for (id, name) in colors::builtin() {
        ids.push(id.into());
        names.push(name);
    }
    ids.push(CUSTOM.into());
    names.push(tr("Custom"));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let palette = adw::ComboRow::builder()
        .title(tr("Palette"))
        .model(&gtk::StringList::new(&names))
        .selected(ids.iter().position(|v| *v == s.palette).unwrap_or(0) as u32)
        .build();
    let terminal = match colors::default_terminal() {
        Some(t) => tr("Colors of {terminal}").replace("{terminal}", t.name()),
        None => tr("Neither Ptyxis nor GNOME Terminal found: Nautilus colors"),
    };
    let subtitle = move |r: &adw::ComboRow, id: &str| match id {
        FOLLOW_TERMINAL => r.set_subtitle(&terminal),
        NAUTILUS => r.set_subtitle(&tr("The background of the file view")),
        _ => r.set_subtitle(""),
    };
    subtitle(&palette, &s.palette);
    group.add(&palette);

    let mut custom_rows = Vec::new();
    for (title, value, set) in [
        (tr("Text, Light Style"), s.custom_light_fg.clone(), (|s, v| s.custom_light_fg = v) as fn(&mut settings::Settings, String)),
        (tr("Background, Light Style"), s.custom_light_bg.clone(), |s, v| s.custom_light_bg = v),
        (tr("Text, Dark Style"), s.custom_dark_fg.clone(), |s, v| s.custom_dark_fg = v),
        (tr("Background, Dark Style"), s.custom_dark_bg.clone(), |s, v| s.custom_dark_bg = v),
    ] {
        let button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
        if let Ok(c) = gdk::RGBA::parse(value.as_str()) {
            button.set_rgba(&c);
        }
        button.set_valign(gtk::Align::Center);
        button.connect_rgba_notify(move |b| {
            let c = b.rgba();
            let hex = format!("#{:02x}{:02x}{:02x}", (c.red() * 255.0).round() as u8,
                              (c.green() * 255.0).round() as u8, (c.blue() * 255.0).round() as u8);
            settings::update(|s| set(s, hex));
        });
        let row = adw::ActionRow::builder().title(title).build();
        row.add_suffix(&button);
        row.set_activatable_widget(Some(&button));
        row.set_visible(s.palette == CUSTOM);
        group.add(&row);
        custom_rows.push(row);
    }

    palette.connect_selected_notify(move |r| {
        let id = ids.get(r.selected() as usize).cloned().unwrap_or_else(|| FOLLOW_TERMINAL.into());
        subtitle(r, &id);
        for row in &custom_rows {
            row.set_visible(id == CUSTOM);
        }
        settings::update(|s| s.palette = id);
    });
    group
}

pub fn show_preferences(win: &gtk::Window) {
    let s = settings::get();
    let dialog = adw::PreferencesDialog::builder().title(tr("Terminal Preferences")).build();
    let page = adw::PreferencesPage::new();

    let general = adw::PreferencesGroup::builder().title(tr("Terminal")).build();
    let follow = adw::SwitchRow::builder()
        .title(tr("Follow Nautilus Folder"))
        .subtitle(tr("The shell changes folder along with the window, when no command is running"))
        .active(s.follow)
        .build();
    follow.connect_active_notify(|r| {
        let on = r.is_active();
        settings::update(|s| s.follow = on);
    });
    general.add(&follow);

    let per_tab = adw::SwitchRow::builder()
        .title(tr("One Terminal per Tab"))
        .subtitle(tr("When off, one terminal per window follows the active tab. Applies to new windows."))
        .active(s.per_tab)
        .build();
    per_tab.connect_active_notify(|r| {
        let on = r.is_active();
        settings::update(|s| s.per_tab = on);
    });
    general.add(&per_tab);

    let new_tabs = adw::SwitchRow::builder()
        .title(tr("Show the Terminal in New Tabs"))
        .subtitle(tr("With one terminal per tab, showing or hiding the terminal only affects the current tab"))
        .active(s.show_in_new_tabs)
        .build();
    new_tabs.connect_active_notify(|r| {
        let on = r.is_active();
        settings::update(|s| s.show_in_new_tabs = on);
    });
    general.add(&new_tabs);

    let rows = adw::SpinRow::with_range(1.0, 100.0, 1.0);
    rows.set_title(&tr("Height When Opened"));
    rows.set_subtitle(&tr("In lines; also the minimum height of a terminal"));
    rows.set_value(s.rows as f64);
    rows.connect_value_notify(|r| {
        let v = r.value() as i32;
        settings::update(|s| s.rows = v);
    });
    general.add(&rows);

    let scrollback = adw::SpinRow::with_range(0.0, 1_000_000.0, 1000.0);
    scrollback.set_title(&tr("Scrollback Lines"));
    scrollback.set_value(s.scrollback as f64);
    scrollback.connect_value_notify(|r| {
        let v = r.value() as i64;
        settings::update(|s| s.scrollback = v);
    });
    general.add(&scrollback);
    page.add(&general);

    let fonts = adw::PreferencesGroup::builder().title(tr("Font")).build();
    let system = adw::SwitchRow::builder()
        .title(tr("Use System Monospace Font"))
        .active(s.font.is_none())
        .build();
    let current = s.font.clone().or_else(system_monospace_font).unwrap_or_else(|| "Monospace 11".into());
    let button = gtk::FontDialogButton::new(Some(gtk::FontDialog::new()));
    button.set_font_desc(&pango::FontDescription::from_string(&current));
    button.set_valign(gtk::Align::Center);
    let font_row = adw::ActionRow::builder().title(tr("Font")).build();
    font_row.add_suffix(&button);
    font_row.set_sensitive(s.font.is_some());
    let b = button.clone();
    let fr = font_row.clone();
    system.connect_active_notify(move |r| {
        let sys = r.is_active();
        fr.set_sensitive(!sys);
        let font = (!sys).then(|| b.font_desc().map(|d| d.to_str().to_string())).flatten();
        settings::update(|s| s.font = font);
    });
    let sys = system.clone();
    button.connect_font_desc_notify(move |b| {
        if !sys.is_active() {
            let font = b.font_desc().map(|d| d.to_str().to_string());
            settings::update(|s| s.font = font);
        }
    });
    fonts.add(&system);
    fonts.add(&font_row);
    page.add(&fonts);
    page.add(&colors_group(&s));

    let keys = adw::PreferencesGroup::builder()
        .title(tr("Keyboard Shortcuts"))
        .description(tr("Click an action, then press the new key combination"))
        .build();
    let mut labels = Vec::new();
    for (action, title) in config::ACTIONS {
        let title = tr(title);
        let row = adw::ActionRow::builder().title(&title).activatable(true).build();
        let label = gtk::Label::new(Some(&accel_text(&settings::accels(action))));
        label.add_css_class("dim-label");
        row.add_suffix(&label);
        let (a, t, w, dlg) = (action.to_string(), title, win.clone(), dialog.clone());
        row.connect_activated(move |_| guard("shortcut row", (), || {
            let a = a.clone();
            capture_shortcut(&w, &dlg, &t, move |accel| {
                if let Some(other) = accel.as_deref().and_then(|k| settings::owner_of(k, &a)) {
                    log!("shortcut removed from \"{other}\" (reassigned)");
                }
                settings::set_accel(&a, accel);
                refresh_labels();
            });
        }));
        keys.add(&row);
        labels.push((action.to_string(), label.downgrade()));
    }
    LABELS.with(|l| *l.borrow_mut() = labels);
    let reset = gtk::Button::with_label(&tr("Reset"));
    reset.add_css_class("flat");
    reset.set_valign(gtk::Align::Center);
    reset.set_tooltip_text(Some(&tr("Restore all default shortcuts")));
    reset.connect_clicked(|_| guard("reset shortcuts", (), || {
        settings::reset_shortcuts();
        refresh_labels();
    }));
    keys.set_header_suffix(Some(&reset));
    page.add(&keys);

    dialog.add(&page);
    dialog.present(Some(win));
}

pub fn show_about(win: &gtk::Window) {
    let about = adw::AboutDialog::builder()
        .application_name("ShellMan")
        .application_icon("shellman")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Sylvain Cresto, Himath Rajapaksha")
        .developers(["Sylvain Cresto", "Himath Rajapaksha"])
        .copyright("© 2026 Sylvain Cresto · fork modifications © 2026 Himath Rajapaksha")
        .license_type(gtk::License::Gpl30)
        .website(env!("CARGO_PKG_HOMEPAGE"))
        .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues"))
        .comments(tr("An embedded terminal at the bottom of Nautilus (GNOME Files) windows. It follows the displayed folder, and the files selected in Nautilus are available in the SEL variable."))
        .build();
    about.add_credit_section(Some(&tr("Built with")), &[&tr("VTE (GNOME terminal widget)"), "gtk4-rs, libadwaita-rs"]);
    about.present(Some(win));
}
