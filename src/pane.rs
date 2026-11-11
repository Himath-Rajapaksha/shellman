use std::cell::{Cell, RefCell};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;

use crate::config;
use crate::i18n::{ntr, tr};
use crate::log;
use crate::panel::Panel;
use crate::shell::{self, Shell};
use crate::settings;
use crate::util::guard;
use crate::vte::{user_shell, Terminal};

pub struct Pane {
    panel: Weak<Panel>,
    pub term: Terminal,
    pub widget: gtk::Widget,
    actions: gio::SimpleActionGroup,
    pid: Cell<Option<i32>>,
    spawning: Cell<bool>,
    shell_dir: RefCell<Option<PathBuf>>,
    pending: RefCell<Option<PathBuf>>,
    follow_override: Cell<Option<bool>>,
    shell: Shell,
    integrated: Cell<bool>,
    cd_file: PathBuf,
    state_file: PathBuf,
}

pub fn system_monospace_font() -> Option<String> {
    let schema = "org.gnome.desktop.interface";
    gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    let s = gio::Settings::new(schema).string("monospace-font-name");
    (!s.is_empty()).then(|| s.to_string())
}

fn accel_for(action: &str) -> Option<String> {
    settings::accels(action).into_iter().next()
}

impl Pane {
    pub fn new(panel: &Rc<Panel>) -> Rc<Pane> {
        let term = Terminal::new();
        let tw = term.widget().clone();
        tw.set_hexpand(true);
        tw.set_vexpand(true);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&tw)
            .build();

        let pane = Rc::new(Pane {
            panel: Rc::downgrade(panel),
            term: term.clone(),
            widget: scrolled.upcast(),
            actions: gio::SimpleActionGroup::new(),
            pid: Cell::new(None),
            spawning: Cell::new(false),
            shell_dir: RefCell::new(None),
            pending: RefCell::new(None),
            follow_override: Cell::new(None),
            shell: Shell::of(&user_shell()),
            integrated: Cell::new(false),
            cd_file: shell::runtime_file("cd"),
            state_file: shell::runtime_file("state"),
        });
        pane.apply_settings();
        pane.widget.add_css_class("view");
        pane.apply_colors(&crate::colors::current().0);
        let weak = Rc::downgrade(&pane);
        tw.connect_map(move |_| {
            if let Some(p) = weak.upgrade() {
                guard("terminal colors", (), || p.apply_colors(&crate::colors::current().0));
            }
        });

        let weak = Rc::downgrade(&pane);
        term.connect_child_exited(move || {
            if let Some(p) = weak.upgrade() {
                p.pid.set(None);
                if let Some(panel) = p.panel.upgrade() {
                    panel.close(&p);
                }
            }
        });
        // the prompt shown after a command triggers the pending cd
        let weak = Rc::downgrade(&pane);
        term.connect_contents_changed(move || {
            if let Some(p) = weak.upgrade() {
                if p.pending.borrow().is_some() {
                    p.flush();
                }
            }
        });
        let weak = Rc::downgrade(&pane);
        term.connect_cwd_changed(move || {
            if let Some(p) = weak.upgrade() {
                if let Some(d) = p.term.cwd() {
                    p.shell_dir.replace(Some(d));
                }
            }
        });

        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(&pane);
        focus.connect_enter(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(panel) = p.panel.upgrade() {
                    guard("focus", (), || panel.pane_focused(&p));
                }
            }
        });
        let weak = Rc::downgrade(&pane);
        focus.connect_leave(move |_| {
            if let Some(panel) = weak.upgrade().and_then(|p| p.panel.upgrade()) {
                guard("focus", (), || panel.mute_app_shortcuts(false));
            }
        });
        tw.add_controller(focus);

        let add = |name: &str, f: fn(&Rc<Pane>)| {
            let a = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(&pane);
            let name = name.to_string();
            a.connect_activate(move |_, _| {
                if let Some(p) = weak.upgrade() {
                    guard(&name, (), || f(&p));
                }
            });
            pane.actions.add_action(&a);
        };
        add("copy", |p| p.term.copy());
        add("paste", |p| p.term.paste());
        add("select-all", |p| p.term.select_all());
        add("insert", |p| p.insert_selection());
        add("insert-var", |p| p.insert_variable());
        add("goto-tab-dir", |p| p.goto_tab_dir());
        add("split-right", |p| p.split(gtk::Orientation::Horizontal));
        add("split-down", |p| p.split(gtk::Orientation::Vertical));
        add("split-auto", |p| {
            let (w, h) = (p.widget.width(), p.widget.height());
            p.split(if w >= h { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical });
        });
        for (name, dir) in [("focus-left", gtk::DirectionType::Left),
                            ("focus-right", gtk::DirectionType::Right),
                            ("focus-up", gtk::DirectionType::Up),
                            ("focus-down", gtk::DirectionType::Down)] {
            let a = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(&pane);
            a.connect_activate(move |_, _| {
                if let Some(p) = weak.upgrade() {
                    if let Some(panel) = p.panel.upgrade() {
                        guard("navigation", (), || panel.focus_neighbor(&p, dir));
                    }
                }
            });
            pane.actions.add_action(&a);
        }
        let follow = gio::SimpleAction::new_stateful("follow", None, &true.to_variant());
        let weak = Rc::downgrade(&pane);
        follow.connect_activate(move |a, _| {
            let Some(p) = weak.upgrade() else { return };
            let on = !p.follows();
            a.set_state(&on.to_variant());
            p.follow_override.set((on != settings::with(|s| s.follow)).then_some(on));
            if let Some(panel) = p.panel.upgrade() {
                guard("follow folder", (), || panel.follow());
            }
        });
        pane.actions.add_action(&follow);

        add("close", |p| {
            if let Some(panel) = p.panel.upgrade() {
                panel.close(p);
            }
        });
        tw.insert_action_group("shellman", Some(&pane.actions));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let acts = pane.actions.clone();
        keys.connect_key_pressed(move |ctrl, key, code, state| guard("terminal key", glib::Propagation::Proceed, || {
            match settings::action_for(ctrl, key, code, state) {
                Some(action) if action != "toggle" && acts.has_action(&action) => {
                    acts.activate_action(&action, None);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }));
        tw.add_controller(keys);

        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&pane);
        click.connect_pressed(move |g, _, x, y| {
            if let Some(p) = weak.upgrade() {
                g.set_state(gtk::EventSequenceState::Claimed);
                p.term.widget().grab_focus();
                guard("menu", (), || p.popup_menu(x, y));
            }
        });
        tw.add_controller(click);

        pane
    }

    pub fn min_height(&self) -> i32 {
        let ch = self.term.char_height();
        let ch = if ch > 0 { ch } else { 18 };
        settings::with(|s| s.rows) * ch + 4
    }

    pub fn apply_settings(&self) {
        let s = settings::get();
        self.term.set_scrollback(s.scrollback);
        if let Some(f) = s.font.clone().or_else(system_monospace_font) {
            self.term.set_font(&f);
        }
        self.update_size_request();
    }

    pub fn update_size_request(&self) {
        self.widget.set_size_request(config::MIN_PANE_WIDTH, self.min_height());
    }

    pub fn follows(&self) -> bool {
        self.follow_override.get().unwrap_or_else(|| settings::with(|s| s.follow))
    }

    pub fn shell_dir(&self) -> Option<PathBuf> {
        self.shell_dir.borrow().clone()
    }

    pub fn has_focus(&self) -> bool {
        self.term.widget().has_focus()
    }

    pub fn focus(&self) {
        self.term.widget().grab_focus();
    }

    pub fn apply_colors(&self, v: &crate::colors::Variant) {
        let fg = v.fg.unwrap_or_else(|| {
            let c = self.term.widget().color();
            gtk::gdk::RGBA::new(c.red(), c.green(), c.blue(), 1.0)
        });
        let bg = v.bg.unwrap_or(gtk::gdk::RGBA::TRANSPARENT);
        self.term.set_colors(&fg, &bg, &v.palette, v.cursor.as_ref());
    }

    pub fn spawn(self: &Rc<Self>, dir: &Path) {
        if self.pid.get().is_some() || self.spawning.get() {
            return;
        }
        let Some(panel) = self.panel.upgrade() else { return };
        self.spawning.set(true);
        self.shell_dir.replace(Some(dir.to_path_buf()));
        let launch = shell::launch(&user_shell());
        self.integrated.set(launch.integrated);
        let _ = std::fs::remove_file(&self.state_file);
        let env: Vec<String> = [
            "SHELLMAN=1".to_string(),
            format!("SHELLMAN_SEL={}", panel.sel_file().display()),
            format!("SHELLMAN_CD={}", self.cd_file.display()),
            format!("SHELLMAN_STATE={}", self.state_file.display()),
        ].into_iter().chain(launch.env).collect();
        let weak = Rc::downgrade(self);
        self.term.spawn(dir, &launch.argv, &env, move |pid| {
            let Some(p) = weak.upgrade() else { return };
            p.spawning.set(false);
            p.pid.set(pid);
            if let (Some(_), Some(panel)) = (pid, p.panel.upgrade()) {
                panel.follow();
            }
        });
    }

    pub fn go_to(self: &Rc<Self>, dir: Option<PathBuf>) {
        if self.pid.get().is_none() || !self.integrated.get() {
            return;
        }
        let Some(dir) = dir else {
            self.pending.replace(None);
            return;
        };
        if self.shell_dir.borrow().as_deref() == Some(dir.as_path()) {
            self.pending.replace(None);
            return;
        }
        self.pending.replace(Some(dir));
        self.flush();
    }

    fn flush(&self) {
        let Some(pid) = self.pid.get() else {
            self.pending.replace(None);
            return;
        };
        if !self.panel.upgrade().is_some_and(|p| p.shown()) {
            self.pending.replace(None);
            return;
        }
        let at_prompt = std::fs::read(&self.state_file).is_ok_and(|s| s == b"prompt");
        if !at_prompt || !self.term.shell_idle(pid) {
            return;
        }
        if let Some(dir) = self.pending.take() {
            // the path goes through a file: only a fixed key is sent to the terminal
            let mut content = dir.as_os_str().as_bytes().to_vec();
            content.push(0);
            if let Err(e) = shell::write_atomic(&self.cd_file, &content) {
                log!("cannot write {}: {e}", self.cd_file.display());
                return;
            }
            self.term.feed(shell::CD_KEY);
            self.shell_dir.replace(Some(dir));
        }
    }

    fn goto_tab_dir(self: &Rc<Self>) {
        let dir = self.panel.upgrade().and_then(|p| p.current_dir());
        if !self.integrated.get() || dir.is_none() {
            self.term.widget().error_bell();
            return;
        }
        self.shell_dir.replace(None);
        self.go_to(dir);
    }

    fn split(self: &Rc<Self>, orientation: gtk::Orientation) {
        if let Some(panel) = self.panel.upgrade() {
            panel.split(self, orientation);
        }
    }

    fn insert_variable(&self) {
        match self.shell.sel_variable() {
            Some(v) => self.term.feed(&format!("{v} ")),
            None => self.insert_selection(),
        }
    }

    fn insert_selection(&self) {
        let Some(panel) = self.panel.upgrade() else { return };
        let sel = panel.selected();
        if sel.paths.is_empty() || sel.skipped > 0 {
            if sel.skipped > 0 {
                log!("insertion refused: {} selected item(s) have no local path", sel.skipped);
            }
            self.term.widget().error_bell();
            return;
        }
        match shell::words(self.shell, &sel.paths, shell::utf8_locale()) {
            Some(text) => self.term.feed(&text),
            None => {
                log!("insertion refused: a file name cannot be written safely for this shell");
                self.term.widget().error_bell();
            }
        }
    }

    fn popup_menu(&self, x: f64, y: f64) {
        let n = self.panel.upgrade().map_or(0, |p| p.selected().paths.len());
        let enable = |name: &str, on: bool| {
            if let Some(a) = self.actions.lookup_action(name)
                .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
            {
                a.set_enabled(on);
            }
        };
        enable("copy", self.term.has_selection());
        enable("insert", n > 0);
        let sel_var = self.shell.sel_variable();
        enable("insert-var", sel_var.is_some());
        enable("follow", self.integrated.get());
        enable("goto-tab-dir", self.integrated.get());
        if let Some(a) = self.actions.lookup_action("follow") {
            a.change_state(&self.follows().to_variant());
        }

        let item = |label: &str, action: &str| {
            let i = gio::MenuItem::new(Some(label), Some(&format!("shellman.{action}")));
            if let Some(a) = accel_for(action) {
                i.set_attribute_value("accel", Some(&a.to_variant()));
            }
            i
        };
        let edit = gio::Menu::new();
        edit.append_item(&item(&tr("Copy"), "copy"));
        edit.append_item(&item(&tr("Paste"), "paste"));
        edit.append_item(&item(&tr("Select All"), "select-all"));
        let files = gio::Menu::new();
        let label = ntr("Insert the Selected File", "Insert the {n} Selected Files", n.max(1) as u32)
            .replace("{n}", &n.to_string());
        files.append_item(&item(&label, "insert"));
        files.append_item(&item(&tr("Insert the {var} Variable").replace("{var}", sel_var.unwrap_or("SEL")), "insert-var"));
        let follow = gio::Menu::new();
        follow.append_item(&item(&tr("Go to Tab Folder"), "goto-tab-dir"));
        follow.append(Some(&tr("Follow Nautilus Folder")), Some("shellman.follow"));
        let layout = gio::Menu::new();
        layout.append_item(&item(&tr("Split Right"), "split-right"));
        layout.append_item(&item(&tr("Split Down"), "split-down"));
        layout.append_item(&item(&tr("Close Terminal"), "close"));
        let app = gio::Menu::new();
        app.append(Some(&tr("Preferences…")), Some("shellman-app.preferences"));
        let menu = gio::Menu::new();
        menu.append_section(None, &edit);
        menu.append_section(None, &files);
        menu.append_section(None, &follow);
        menu.append_section(None, &layout);
        menu.append_section(None, &app);

        let pop = gtk::PopoverMenu::from_model(Some(&menu));
        pop.set_parent(self.term.widget());
        pop.set_has_arrow(false);
        pop.set_halign(gtk::Align::Start);
        pop.set_position(gtk::PositionType::Top);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        let actions = self.actions.clone();
        pop.connect_closed(move |p| guard("menu closed", (), || {
            for name in ["copy", "insert", "insert-var", "follow", "goto-tab-dir"] {
                if let Some(a) = actions.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                    a.set_enabled(true);
                }
            }
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        }));
        pop.popup();
    }

    pub fn stop(&self) {
        self.pending.replace(None);
        let _ = std::fs::remove_file(&self.cd_file);
        let _ = std::fs::remove_file(&self.state_file);
        let mut c = self.term.widget().first_child();
        while let Some(w) = c {
            c = w.next_sibling();
            if w.is::<gtk::Popover>() {
                w.unparent();
            }
        }
    }
}
