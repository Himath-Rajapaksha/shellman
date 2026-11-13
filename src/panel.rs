use std::cell::{Cell, OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::config;
use crate::i18n::tr;
use crate::location::LocationSource;
use crate::log;
use crate::pane::Pane;
use crate::selection;
use crate::settings;
use crate::prefs;
use crate::status;
use crate::util::{guard, type_name, walk};

const DATA_KEY: &str = "shellman-panel";

thread_local! {
    static HEIGHT: Cell<Option<i32>> = const { Cell::new(None) };
}

struct Group {
    tab: Option<glib::WeakRef<glib::Object>>,
    holder: adw::Bin,
    panes: Vec<Rc<Pane>>,
    active: Weak<Pane>,
    shown: bool,
}

impl Group {
    fn closed(&self) -> bool {
        self.tab.as_ref().is_some_and(|t| match t.upgrade() {
            None => true,
            Some(slot) => slot.downcast_ref::<gtk::Widget>().is_some_and(|w| w.root().is_none()),
        })
    }
}

pub struct Panel {
    win: gtk::Window,
    paned: gtk::Paned,
    stack: gtk::Stack,
    body: gtk::Box,
    status: gtk::Label,
    status_message: RefCell<Option<(u64, String)>>,
    status_seq: Cell<u64>,
    button: gtk::Widget,
    per_tab: bool,
    loc: OnceCell<Rc<LocationSource>>,
    groups: RefCell<Vec<Group>>,
    last_tab: RefCell<Option<glib::WeakRef<glib::Object>>>,
    prev_focus: glib::WeakRef<gtk::Widget>,
    muted: RefCell<Vec<(gtk::EventController, gtk::PropagationPhase)>>,
    sel_file: PathBuf,
}

impl Panel {
    pub fn current_dir(&self) -> Option<PathBuf> {
        self.loc.get().and_then(|l| l.location().0)
    }

    pub fn sel_file(&self) -> &std::path::Path {
        &self.sel_file
    }

    pub fn shown(&self) -> bool {
        self.current_group().is_some_and(|i| self.groups.borrow()[i].shown)
    }

    fn set_shown(&self, on: bool) {
        if let Some(i) = self.current_group() {
            self.groups.borrow_mut()[i].shown = on;
        }
    }

    pub fn toggle(self: &Rc<Self>) {
        if self.shown() { self.hide() } else { self.show() }
    }

    fn toggle_or_focus(self: &Rc<Self>) {
        if self.shown() && self.all_panes().iter().any(|p| p.has_focus()) { self.hide() } else { self.show() }
    }

    fn start_dir(&self) -> PathBuf {
        self.current_dir().filter(|p| p.is_dir()).unwrap_or_else(glib::home_dir)
    }

    fn current_tab(&self) -> Option<glib::Object> {
        if self.per_tab { self.loc.get().and_then(|l| l.slot()) } else { None }
    }

    fn current_group(&self) -> Option<usize> {
        let tab = self.current_tab();
        self.groups.borrow().iter().position(|g| match (&g.tab, &tab) {
            (None, None) => true,
            (Some(w), Some(t)) => w.upgrade().as_ref() == Some(t),
            _ => false,
        })
    }

    fn group_of(&self, pane: &Rc<Pane>) -> Option<usize> {
        self.groups.borrow().iter().position(|g| g.panes.iter().any(|p| Rc::ptr_eq(p, pane)))
    }

    fn all_panes(&self) -> Vec<Rc<Pane>> {
        self.groups.borrow().iter().flat_map(|g| g.panes.iter().cloned()).collect()
    }

    fn ensure_group(self: &Rc<Self>) -> usize {
        if let Some(i) = self.current_group() {
            if !self.groups.borrow()[i].panes.is_empty() {
                return i;
            }
        }
        let i = self.current_group().unwrap_or_else(|| {
            let holder = adw::Bin::new();
            self.stack.add_child(&holder);
            let tab = self.current_tab().map(|t| t.downgrade());
            self.groups.borrow_mut().push(Group { tab, holder, panes: Vec::new(), active: Weak::new(), shown: false });
            self.groups.borrow().len() - 1
        });
        let pane = Pane::new(self);
        {
            let mut groups = self.groups.borrow_mut();
            groups[i].holder.set_child(Some(&pane.widget));
            groups[i].panes.push(pane.clone());
            groups[i].active = Rc::downgrade(&pane);
        }
        pane.spawn(&self.start_dir());
        i
    }

    fn drop_closed_tabs(&self) {
        let closed: Vec<Group> = {
            let mut groups = self.groups.borrow_mut();
            let (closed, open) = groups.drain(..).partition(|g| g.closed());
            *groups = open;
            closed
        };
        for g in closed {
            for p in &g.panes {
                p.stop();
            }
            self.stack.remove(&g.holder);
        }
    }

    fn show_current_group(self: &Rc<Self>) {
        let i = self.ensure_group();
        let (holder, active) = {
            let g = &self.groups.borrow()[i];
            (g.holder.clone(), g.active.upgrade().or_else(|| g.panes.first().cloned()))
        };
        let had_focus = self.all_panes().iter().any(|p| p.has_focus());
        self.stack.set_visible_child(&holder);
        if had_focus {
            if let Some(p) = active {
                p.focus();
            }
        }
    }

    fn tab_changed(self: &Rc<Self>) {
        self.drop_closed_tabs();
        let tab = self.current_tab();
        let same_tab = self.last_tab.borrow().as_ref().and_then(|t| t.upgrade()) == tab;
        if self.per_tab && !same_tab {
            self.last_tab.replace(tab.map(|t| t.downgrade()));
            let show = match self.current_group() {
                Some(i) => self.groups.borrow()[i].shown,
                None => settings::with(|s| s.show_in_new_tabs),
            };
            if show {
                self.show_current_group();
                self.set_shown(true);
                self.reveal();
            } else {
                self.conceal();
            }
        }
        self.follow();
        self.update_status();
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(p) = weak.upgrade() {
                guard("closed tabs", (), || p.drop_closed_tabs());
            }
        });
    }

    fn show(self: &Rc<Self>) {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.win);
        let in_panel = focus.as_ref().is_some_and(|f| f.is_ancestor(&self.stack));
        if !in_panel {
            self.prev_focus.set(focus.as_ref());
        }
        self.show_current_group();
        self.set_shown(true);
        self.reveal();
        self.refresh_selection();
        self.follow();
        let active = self.current_group().and_then(|i| {
            let g = &self.groups.borrow()[i];
            g.active.upgrade().or_else(|| g.panes.first().cloned())
        });
        if let Some(p) = active {
            p.focus();
        }
    }

    fn reveal(&self) {
        for p in self.all_panes() {
            p.update_size_request();
        }
        self.body.set_visible(true);
        self.stack.set_visible(true);
        self.update_button();
        let h = self.paned.height();
        if h > 0 {
            let min = self.body.measure(gtk::Orientation::Vertical, -1).0;
            let th = HEIGHT.get().unwrap_or(min).clamp(min, (h - config::MIN_FILES_HEIGHT).max(min));
            self.paned.set_position(h - th);
        }
    }

    fn conceal(&self) {
        let focus = gtk::prelude::GtkWindowExt::focus(&self.win);
        if focus.is_some_and(|f| f.is_ancestor(&self.stack)) {
            gtk::prelude::GtkWindowExt::set_focus(&self.win, None::<&gtk::Widget>);
        }
        self.body.set_visible(false);
        self.stack.set_visible(false);
        self.update_button();
    }

    fn hide(&self) {
        // no child_focus: the file view would select the item that gets focus
        if self.all_panes().iter().any(|p| p.has_focus()) {
            let prev = self.prev_focus.upgrade().filter(|w| {
                w.is_mapped() && !w.is_ancestor(&self.stack)
                    && w.root().is_some_and(|r| r.upcast::<gtk::Widget>() == *self.win.upcast_ref::<gtk::Widget>())
            });
            if !prev.is_some_and(|w| w.grab_focus()) {
                gtk::prelude::GtkWindowExt::set_focus(&self.win, None::<&gtk::Widget>);
            }
        }
        self.set_shown(false);
        self.body.set_visible(false);
        self.stack.set_visible(false);
        self.update_button();
    }

    fn update_button(&self) {
        let key = settings::accels("toggle").first()
            .and_then(gtk::accelerator_parse)
            .map(|(k, m)| format!(" ({})", gtk::accelerator_get_label(k, m)))
            .unwrap_or_default();
        let what = if self.shown() { tr("Hide the terminal") } else { tr("Open a terminal here") };
        self.button.set_tooltip_text(Some(&format!("{what}{key}")));
    }

    fn apply_settings(&self) {
        for p in self.all_panes() {
            p.apply_settings();
        }
        self.update_button();
        self.follow();
        self.update_status();
    }

    pub fn follow(&self) {
        let dir = if self.shown() { self.current_dir() } else { None };
        let current = self.current_group();
        let groups: Vec<(bool, Vec<Rc<Pane>>)> = self.groups.borrow().iter().enumerate()
            .map(|(i, g)| (Some(i) == current, g.panes.clone()))
            .collect();
        for (is_current, panes) in groups {
            for p in panes {
                p.go_to(if is_current && p.follows() { dir.clone() } else { None });
            }
        }
    }

    pub fn split(self: &Rc<Self>, pane: &Rc<Pane>, orientation: gtk::Orientation) {
        let Some(gi) = self.group_of(pane) else { return };
        let old = pane.widget.clone();
        let Some(parent) = old.parent() else { return };
        let size = if orientation == gtk::Orientation::Horizontal { old.width() } else { old.height() };
        let pn = gtk::Paned::new(orientation);
        pn.set_wide_handle(true);
        pn.set_shrink_start_child(false);
        pn.set_shrink_end_child(false);
        if !replace_child(&parent, &old, pn.upcast_ref()) {
            log!("cannot split inside {}", type_name(&parent));
            return;
        }
        pn.set_start_child(Some(&old));
        let new = Pane::new(self);
        pn.set_end_child(Some(&new.widget));
        if size > 0 {
            pn.set_position(size / 2);
        }
        self.groups.borrow_mut()[gi].panes.push(new.clone());
        new.spawn(&pane.shell_dir().filter(|d| d.is_dir()).unwrap_or_else(|| self.start_dir()));
        new.focus();
    }

    pub fn close(&self, pane: &Rc<Pane>) {
        pane.stop();
        let Some(gi) = self.group_of(pane) else { return };
        let holder = self.groups.borrow()[gi].holder.clone();
        let w = pane.widget.clone();
        let parent = w.parent();
        let last = parent.as_ref() == Some(holder.upcast_ref::<gtk::Widget>());
        if last && Some(gi) == self.current_group() {
            self.hide();
        }
        self.groups.borrow_mut()[gi].panes.retain(|p| !Rc::ptr_eq(p, pane));
        if last {
            holder.set_child(None::<&gtk::Widget>);
            self.mute_app_shortcuts(false);
            return;
        }
        let Some(parent) = parent else { return };
        let Some(pn) = parent.downcast_ref::<gtk::Paned>() else { return };
        let sibling = if pn.start_child().as_ref() == Some(&w) { pn.end_child() } else { pn.start_child() };
        let (Some(sibling), Some(grand)) = (sibling, pn.parent()) else { return };
        let next = self.groups.borrow()[gi].panes.iter()
            .find(|p| p.widget == sibling || p.widget.is_ancestor(&sibling)).cloned();
        if let Some(p) = &next {
            self.groups.borrow_mut()[gi].active = Rc::downgrade(p);
            p.focus();
        }
        pn.set_start_child(None::<&gtk::Widget>);
        pn.set_end_child(None::<&gtk::Widget>);
        if !replace_child(&grand, pn.upcast_ref(), &sibling) {
            log!("close: unexpected container {}", type_name(&grand));
            return;
        }
        if let Some(p) = next {
            p.focus();
        }
    }

    pub fn focus_neighbor(&self, pane: &Rc<Pane>, dir: gtk::DirectionType) {
        use gtk::DirectionType as D;
        let Some(gi) = self.group_of(pane) else { return };
        let (root, panes) = {
            let g = &self.groups.borrow()[gi];
            (g.holder.clone().upcast::<gtk::Widget>(), g.panes.clone())
        };
        let Some(cur) = pane.widget.compute_bounds(&root) else { return };
        let (cx0, cy0) = (cur.x(), cur.y());
        let (cx1, cy1) = (cx0 + cur.width(), cy0 + cur.height());
        let best = panes.iter()
            .filter(|p| !Rc::ptr_eq(p, pane))
            .filter_map(|p| p.widget.compute_bounds(&root).map(|r| (p.clone(), r)))
            .filter_map(|(p, r)| {
                let (x0, y0, x1, y1) = (r.x(), r.y(), r.x() + r.width(), r.y() + r.height());
                let overlap_y = y0 < cy1 && y1 > cy0;
                let overlap_x = x0 < cx1 && x1 > cx0;
                let gap = match dir {
                    D::Left if overlap_y && x1 <= cx0 + 1.0 => cx0 - x1,
                    D::Right if overlap_y && x0 >= cx1 - 1.0 => x0 - cx1,
                    D::Up if overlap_x && y1 <= cy0 + 1.0 => cy0 - y1,
                    D::Down if overlap_x && y0 >= cy1 - 1.0 => y0 - cy1,
                    _ => return None,
                };
                Some((p, gap))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((p, _)) => p.focus(),
            None => pane.term.widget().error_bell(),
        }
    }

    pub fn pane_focused(&self, pane: &Rc<Pane>) {
        if let Some(gi) = self.group_of(pane) {
            self.groups.borrow_mut()[gi].active = Rc::downgrade(pane);
        }
        self.mute_app_shortcuts(true);
        self.refresh_selection();
    }

    pub fn mute_app_shortcuts(&self, mute: bool) {
        let mut muted = self.muted.borrow_mut();
        if !mute {
            for (c, phase) in muted.drain(..) {
                c.set_propagation_phase(phase);
            }
            return;
        }
        if !muted.is_empty() {
            return;
        }
        let list = self.win.observe_controllers();
        for i in 0..list.n_items() {
            let Some(c) = list.item(i).and_then(|o| o.downcast::<gtk::EventController>().ok())
            else { continue };
            let global = c.downcast_ref::<gtk::ShortcutController>()
                .is_some_and(|s| s.scope() == gtk::ShortcutScope::Global);
            if global && c.propagation_phase() != gtk::PropagationPhase::None {
                muted.push((c.clone(), c.propagation_phase()));
                c.set_propagation_phase(gtk::PropagationPhase::None);
            }
        }
    }

    pub fn selected(&self) -> selection::Selection {
        let slot = self.loc.get().and_then(|l| l.slot());
        slot.map(|s| selection::selected(&s)).unwrap_or_default()
    }

    fn refresh_selection(&self) {
        selection::write_sel_file(&self.sel_file, &self.selected().paths);
        self.update_status();
    }

    fn update_status(&self) {
        if let Some((_, msg)) = self.status_message.borrow().as_ref() {
            self.status.set_text(msg);
            return;
        }
        let sel = self.selected();
        let shortcut = settings::accels("insert").first()
            .and_then(gtk::accelerator_parse)
            .map(|(k, m)| gtk::accelerator_get_label(k, m).to_string());
        self.status.set_text(&status::summary(
            sel.paths.len(), sel.skipped, shortcut.as_deref()));
    }

    pub fn message(self: &Rc<Self>, text: String) {
        let seq = self.status_seq.get() + 1;
        self.status_seq.set(seq);
        self.status.set_text(&text);
        self.status_message.replace(Some((seq, text)));
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_secs(3), move || {
            if let Some(p) = weak.upgrade() {
                if p.status_seq.get() == seq {
                    p.status_message.replace(None);
                    p.update_status();
                }
            }
        });
    }

    fn stop(&self) {
        for p in self.all_panes() {
            p.stop();
        }
        for g in self.groups.take() {
            self.stack.remove(&g.holder);
        }
        let _ = std::fs::remove_file(&self.sel_file);
    }
}

fn build(win: &gtk::Window, paned: gtk::Paned, button: gtk::Widget) -> Rc<Panel> {
    let stack = gtk::Stack::new();
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(false);
    stack.set_visible(false);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.set_visible(false);
    stack.set_vexpand(true);
    body.append(&stack);

    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    status.set_focusable(false);
    status.set_selectable(false);
    status.set_margin_start(6);
    status.set_margin_end(6);
    status.set_margin_top(2);
    status.set_margin_bottom(2);
    status.add_css_class("dim-label");
    body.append(&status);

    paned.set_end_child(Some(&body));

    let panel = Rc::new(Panel {
        win: win.clone(),
        paned: paned.clone(),
        stack,
        body,
        status,
        status_message: RefCell::new(None),
        status_seq: Cell::new(0),
        button: button.clone(),
        per_tab: settings::with(|s| s.per_tab),
        loc: OnceCell::new(),
        groups: RefCell::new(Vec::new()),
        last_tab: RefCell::new(None),
        prev_focus: glib::WeakRef::new(),
        muted: RefCell::new(Vec::new()),
        sel_file: selection::new_sel_file(),
    });

    panel.update_button();
    let weak = Rc::downgrade(&panel);
    let on_click = move || {
        if let Some(p) = weak.upgrade() {
            guard("button", (), || p.toggle());
        }
    };
    if let Some(b) = button.downcast_ref::<adw::SplitButton>() {
        b.connect_clicked(move |_| on_click());
    } else if let Some(b) = button.downcast_ref::<gtk::Button>() {
        b.connect_clicked(move |_| on_click());
    }

    let actions = gio::SimpleActionGroup::new();
    for (name, f) in [("preferences", prefs::show_preferences as fn(&gtk::Window)),
                      ("about", prefs::show_about)] {
        let a = gio::SimpleAction::new(name, None);
        let w = win.downgrade();
        a.connect_activate(move |_, _| {
            if let Some(w) = w.upgrade() {
                guard(name, (), || f(&w));
            }
        });
        actions.add_action(&a);
    }
    win.insert_action_group("shellman-app", Some(&actions));

    let weak = Rc::downgrade(&panel);
    let loc = LocationSource::new(win, move || {
        if let Some(p) = weak.upgrade() {
            guard("location change", (), || p.tab_changed());
        }
    });
    let _ = panel.loc.set(loc);

    let weak = Rc::downgrade(&panel);
    paned.connect_position_notify(move |pn| guard("terminal height", (), || {
        if let Some(p) = weak.upgrade() {
            let h = pn.height();
            if p.shown() && h > 0 && pn.position() > 0 {
                HEIGHT.set(Some(h - pn.position()));
            }
        }
    }));

    let weak = Rc::downgrade(&panel);
    adw::StyleManager::default().connect_dark_notify(move |_| guard("theme", (), || {
        if weak.upgrade().is_some() {
            glib::idle_add_local_once(|| guard("theme", (), restyle_all));
        }
    }));

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak: Weak<Panel> = Rc::downgrade(&panel);
    keys.connect_key_pressed(move |ctrl, key, code, state| {
        if !crate::prefs::capturing()
            && settings::action_for(ctrl, key, code, state).as_deref() == Some("toggle")
        {
            if let Some(p) = weak.upgrade() {
                guard("toggle", (), || p.toggle_or_focus());
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    win.add_controller(keys);

    panel
}

fn replace_child(parent: &gtk::Widget, old: &gtk::Widget, new: &gtk::Widget) -> bool {
    if let Some(b) = parent.downcast_ref::<gtk::Box>() {
        let prev = old.prev_sibling();
        b.remove(old);
        b.insert_child_after(new, prev.as_ref());
        return true;
    }
    if let Some(t) = parent.downcast_ref::<adw::ToolbarView>() {
        if t.content().as_ref() == Some(old) {
            t.set_content(Some(new));
            return true;
        }
    }
    if let Some(s) = parent.downcast_ref::<adw::OverlaySplitView>() {
        if s.content().as_ref() == Some(old) {
            s.set_content(Some(new));
            return true;
        }
    }
    if let Some(b) = parent.downcast_ref::<adw::Bin>() {
        b.set_child(Some(new));
        return true;
    }
    if let Some(b) = parent.downcast_ref::<adw::BreakpointBin>() {
        b.set_child(Some(new));
        return true;
    }
    if let Some(t) = parent.downcast_ref::<adw::ToastOverlay>() {
        t.set_child(Some(new));
        return true;
    }
    if let Some(o) = parent.downcast_ref::<gtk::Overlay>() {
        if o.child().as_ref() == Some(old) {
            o.set_child(Some(new));
            return true;
        }
    }
    if let Some(p) = parent.downcast_ref::<gtk::Paned>() {
        if p.start_child().as_ref() == Some(old) {
            p.set_start_child(Some(new));
            return true;
        }
        if p.end_child().as_ref() == Some(old) {
            p.set_end_child(Some(new));
            return true;
        }
    }
    false
}

fn dump_tree(win: &gtk::Window) {
    fn rec(w: &gtk::Widget, depth: usize, out: &mut Vec<String>) {
        out.push(format!("{}{} css={:?}", "  ".repeat(depth), type_name(w),
                         w.css_classes().iter().map(|c| c.to_string()).collect::<Vec<_>>()));
        let mut c = w.first_child();
        while let Some(ch) = c {
            rec(&ch, depth + 1, out);
            c = ch.next_sibling();
        }
    }
    let mut out = Vec::new();
    rec(win.upcast_ref(), 0, &mut out);
    let path = glib::home_dir().join("shellman-tree.txt");
    if std::fs::write(&path, out.join("\n") + "\n").is_ok() {
        log!("widget tree written to {}", path.display());
    }
}

fn sidebar_toolbar(win: &gtk::Window) -> Option<adw::ToolbarView> {
    let sidebar = walk(win).into_iter().find_map(|w| {
        if let Some(s) = w.downcast_ref::<adw::OverlaySplitView>() {
            return s.sidebar();
        }
        w.downcast_ref::<adw::NavigationSplitView>()
            .and_then(|s| s.sidebar()).map(|p| p.upcast())
    })?;
    walk(&sidebar).into_iter().find_map(|w| w.downcast::<adw::ToolbarView>().ok())
}

fn place_button(win: &gtk::Window, tab: &gtk::Widget) -> (gtk::Widget, gtk::Widget) {
    if let Some(tv) = sidebar_toolbar(win) {
        let content = adw::ButtonContent::builder()
            .icon_name(config::BUTTON_ICON)
            .label(tr("Terminal"))
            .build();
        let menu = gio::Menu::new();
        menu.append(Some(&tr("Preferences…")), Some("shellman-app.preferences"));
        menu.append(Some(&tr("About ShellMan")), Some("shellman-app.about"));
        let button = adw::SplitButton::builder()
            .child(&content)
            .menu_model(&menu)
            .dropdown_tooltip(tr("Preferences, About"))
            .halign(gtk::Align::Start)
            .focusable(false)
            .build();
        button.add_css_class("flat");
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.set_margin_start(6);
        bar.set_margin_end(6);
        bar.set_margin_top(6);
        bar.set_margin_bottom(6);
        bar.append(&button);
        // not add_bottom_bar: Nautilus hides its bottom bar unless a copy is running
        if let Some(list) = tv.content() {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
            tv.set_content(Some(&column));
            list.set_vexpand(true);
            column.append(&list);
            column.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            column.append(&bar);
            log!("button placed at the bottom of the sidebar");
            return (tab.clone(), button.upcast());
        }
    }
    log!("sidebar not found: floating button over the files");
    let button = gtk::Button::from_icon_name(config::BUTTON_ICON);
    button.set_focusable(false);
    button.add_css_class("osd");
    button.add_css_class("circular");
    button.set_halign(gtk::Align::Start);
    button.set_valign(gtk::Align::End);
    button.set_margin_start(12);
    button.set_margin_bottom(12);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(tab));
    overlay.add_overlay(&button);
    (overlay.upcast(), button.upcast())
}

fn inject(win: &gtk::Window, tries: &Cell<u32>) -> bool {
    if unsafe { win.data::<Rc<Panel>>(DATA_KEY) }.is_some() {
        return false;
    }
    if std::env::var_os("SHELLMAN_DUMP").is_some() && tries.get() == 0 {
        dump_tree(win);
    }
    let tab = walk(win).into_iter().find(|w| type_name(w) == "AdwTabView");
    let Some((tab, parent)) = tab.and_then(|t| t.parent().map(|p| (t, p))) else {
        tries.set(tries.get() + 1);
        if tries.get() < 40 {
            return true;
        }
        log!("AdwTabView not found after {} attempts", tries.get());
        dump_tree(win);
        return false;
    };
    let paned = gtk::Paned::new(gtk::Orientation::Vertical);
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    paned.set_wide_handle(true);
    paned.set_resize_start_child(true);
    paned.set_shrink_start_child(false);
    paned.set_resize_end_child(false);
    paned.set_shrink_end_child(false);
    if !replace_child(&parent, &tab, paned.upcast_ref()) {
        log!("unsupported AdwTabView container: {}", type_name(&parent));
        dump_tree(win);
        return false;
    }
    let (top, button) = place_button(win, &tab);
    paned.set_start_child(Some(&top));
    let panel = build(win, paned, button);
    if std::env::var_os("SHELLMAN_TEST_SHOW").is_some() {
        let weak = Rc::downgrade(&panel);
        glib::timeout_add_local_once(Duration::from_millis(500), move || {
            if let Some(p) = weak.upgrade() {
                guard("test show", (), || p.show());
            }
        });
    }
    unsafe { win.set_data::<Rc<Panel>>(DATA_KEY, panel) };
    log!("terminal injected into {}", type_name(&parent));
    false
}

fn schedule_inject(win: gtk::Window) {
    if type_name(&win) != "NautilusWindow" {
        return;
    }
    glib::idle_add_local_once(move || {
        let tries = Cell::new(0);
        if !guard("injection", false, || inject(&win, &tries)) {
            return;
        }
        log!("window not ready yet: injection deferred");
        glib::timeout_add_local(Duration::from_millis(250), move || {
            let again = guard("injection", false, || inject(&win, &tries));
            if again { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
        });
    });
}

fn app() -> Option<gtk::Application> {
    gio::Application::default().and_then(|a| a.downcast::<gtk::Application>().ok())
}

pub fn mute_window_shortcuts(win: &gtk::Window, mute: bool) {
    if let Some(p) = unsafe { win.data::<Rc<Panel>>(DATA_KEY) } {
        unsafe { p.as_ref() }.mute_app_shortcuts(mute);
    }
}

pub fn apply_settings_all() {
    let Some(app) = app() else { return };
    for w in app.windows() {
        if let Some(p) = unsafe { w.data::<Rc<Panel>>(DATA_KEY) } {
            unsafe { p.as_ref() }.apply_settings();
        }
    }
    restyle_all();
}

pub fn restyle_all() {
    let Some(app) = app() else { return };
    let (colors, _) = crate::colors::current();
    for w in app.windows() {
        if let Some(p) = unsafe { w.data::<Rc<Panel>>(DATA_KEY) } {
            for pane in unsafe { p.as_ref() }.all_panes() {
                pane.apply_colors(&colors);
            }
        }
    }
}

pub fn toggle_active() {
    let Some(win) = app().and_then(|a| a.active_window()) else { return };
    match unsafe { win.data::<Rc<Panel>>(DATA_KEY) } {
        Some(p) => unsafe { p.as_ref() }.clone().toggle(),
        None => log!("no terminal in this window"),
    }
}

fn setup(app: &gtk::Application) {
    for w in app.windows() {
        schedule_inject(w);
    }
    app.connect_window_added(|_, w| guard("window added", (), || schedule_inject(w.clone())));
    app.connect_window_removed(|_, w| guard("window removed", (), || {
        if let Some(p) = unsafe { w.steal_data::<Rc<Panel>>(DATA_KEY) } {
            p.stop();
        }
    }));
}

// the extension API does not expose Nautilus' version: ask the running binary
fn log_nautilus_version() {
    std::thread::spawn(|| {
        let out = std::process::Command::new("/proc/self/exe").arg("--version").output();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let Some(version) = text.split_whitespace().last().filter(|v| v.contains('.')) else {
            log!("Nautilus version unknown");
            return;
        };
        log!("Nautilus {version}");
        let major = version.split('.').next().and_then(|m| m.parse::<u32>().ok());
        if !major.is_some_and(|m| config::TESTED_NAUTILUS.contains(&m)) {
            log!("warning: ShellMan has not been tested with Nautilus {version} (tested: {:?})",
                 config::TESTED_NAUTILUS);
        }
    });
}

fn try_start() -> bool {
    guard("startup", true, || {
        if unsafe { gtk::ffi::gtk_is_initialized() } == 0 {
            return false;
        }
        unsafe { gtk::set_initialized() };
        let Some(app) = app() else { return false };
        crate::icons::install();
        crate::shell::remove_stale_files();
        log_nautilus_version();
        setup(&app);
        true
    })
}

pub fn start() {
    if try_start() {
        return;
    }
    log!("application not ready yet: startup deferred");
    let tries = Cell::new(0u32);
    glib::timeout_add_local(Duration::from_millis(100), move || {
        tries.set(tries.get() + 1);
        if try_start() || tries.get() >= 500 { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
    });
}
