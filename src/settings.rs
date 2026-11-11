use std::cell::RefCell;
use std::path::PathBuf;

use gtk::glib;

use crate::config;
use crate::log;

const GROUP: &str = "terminal";
const KEYS: &str = "shortcuts";

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub follow: bool,
    pub per_tab: bool,
    pub show_in_new_tabs: bool,
    pub rows: i32,
    pub scrollback: i64,
    pub font: Option<String>,
    pub palette: String,
    pub style: String,
    pub custom_light_fg: String,
    pub custom_light_bg: String,
    pub custom_dark_fg: String,
    pub custom_dark_bg: String,
    pub shortcuts: Vec<(String, Vec<String>)>,
}

pub fn default_shortcuts() -> Vec<(String, Vec<String>)> {
    config::ACTIONS.iter().map(|(action, _)| {
        let accels = config::DEFAULT_SHORTCUTS.iter()
            .filter(|(_, a)| a == action).map(|(k, _)| k.to_string()).collect();
        (action.to_string(), accels)
    }).collect()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            follow: true,
            per_tab: true,
            show_in_new_tabs: false,
            rows: config::MIN_ROWS,
            scrollback: config::SCROLLBACK_LINES,
            font: None,
            palette: crate::colors::FOLLOW_TERMINAL.into(),
            style: "system".into(),
            custom_light_fg: "#1d1d20".into(),
            custom_light_bg: "#ffffff".into(),
            custom_dark_fg: "#ffffff".into(),
            custom_dark_bg: "#1c1c1f".into(),
            shortcuts: default_shortcuts(),
        }
    }
}

fn path() -> PathBuf {
    glib::user_config_dir().join("shellman").join("settings.ini")
}

fn string(kf: &glib::KeyFile, key: &str, default: String) -> String {
    kf.string(GROUP, key).ok().map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or(default)
}

fn color(kf: &glib::KeyFile, key: &str, default: String) -> String {
    Some(string(kf, key, default.clone()))
        .filter(|c| gtk::gdk::RGBA::parse(c.as_str()).is_ok()).unwrap_or(default)
}

fn load() -> Settings {
    let d = Settings::default();
    let kf = glib::KeyFile::new();
    if kf.load_from_file(path(), glib::KeyFileFlags::NONE).is_err() {
        return d;
    }
    Settings {
        follow: kf.boolean(GROUP, "follow").unwrap_or(d.follow),
        per_tab: kf.boolean(GROUP, "per_tab").unwrap_or(d.per_tab),
        show_in_new_tabs: kf.boolean(GROUP, "show_in_new_tabs").unwrap_or(d.show_in_new_tabs),
        rows: kf.integer(GROUP, "rows").map(|v| v.clamp(1, 100)).unwrap_or(d.rows),
        scrollback: kf.int64(GROUP, "scrollback").map(|v| v.max(0)).unwrap_or(d.scrollback),
        font: kf.string(GROUP, "font").ok().map(|s| s.to_string()).filter(|s| !s.is_empty()),
        palette: string(&kf, "palette", d.palette),
        style: Some(string(&kf, "style", d.style.clone()))
            .filter(|v| ["system", "light", "dark"].contains(&v.as_str())).unwrap_or(d.style),
        custom_light_fg: color(&kf, "custom_light_fg", d.custom_light_fg),
        custom_light_bg: color(&kf, "custom_light_bg", d.custom_light_bg),
        custom_dark_fg: color(&kf, "custom_dark_fg", d.custom_dark_fg),
        custom_dark_bg: color(&kf, "custom_dark_bg", d.custom_dark_bg),
        shortcuts: d.shortcuts.into_iter().map(|(action, def)| {
            let accels = kf.string(KEYS, &action).ok()
                .map(|v| v.split_whitespace().map(String::from).collect())
                .unwrap_or(def);
            (action, accels)
        }).collect(),
    }
}

fn save(s: &Settings) {
    let kf = glib::KeyFile::new();
    kf.set_boolean(GROUP, "follow", s.follow);
    kf.set_boolean(GROUP, "per_tab", s.per_tab);
    kf.set_boolean(GROUP, "show_in_new_tabs", s.show_in_new_tabs);
    kf.set_integer(GROUP, "rows", s.rows);
    kf.set_int64(GROUP, "scrollback", s.scrollback);
    kf.set_string(GROUP, "font", s.font.as_deref().unwrap_or(""));
    kf.set_string(GROUP, "palette", &s.palette);
    kf.set_string(GROUP, "style", &s.style);
    kf.set_string(GROUP, "custom_light_fg", &s.custom_light_fg);
    kf.set_string(GROUP, "custom_light_bg", &s.custom_light_bg);
    kf.set_string(GROUP, "custom_dark_fg", &s.custom_dark_fg);
    kf.set_string(GROUP, "custom_dark_bg", &s.custom_dark_bg);
    for (action, accels) in &s.shortcuts {
        kf.set_string(KEYS, action, &accels.join(" "));
    }
    let p = path();
    let res = p.parent().map_or(Ok(()), std::fs::create_dir_all)
        .map_err(|e| e.to_string())
        .and_then(|_| kf.save_to_file(&p).map_err(|e| e.to_string()));
    if let Err(e) = res {
        log!("cannot save {}: {e}", p.display());
    }
}

thread_local! {
    static CURRENT: RefCell<Settings> = RefCell::new(load());
}

pub fn get() -> Settings {
    CURRENT.with(|c| c.borrow().clone())
}

pub fn with<R>(f: impl FnOnce(&Settings) -> R) -> R {
    CURRENT.with(|c| f(&c.borrow()))
}

pub fn update(f: impl FnOnce(&mut Settings)) {
    crate::util::guard("settings", (), || {
        let changed = CURRENT.with(|c| {
            let mut s = c.borrow_mut();
            let before = s.clone();
            f(&mut s);
            (*s != before).then(|| s.clone())
        });
        if let Some(s) = changed {
            save(&s);
            crate::panel::apply_settings_all();
        }
    })
}

pub fn accels(action: &str) -> Vec<String> {
    CURRENT.with(|c| c.borrow().shortcuts.iter()
        .find(|(a, _)| a == action).map(|(_, k)| k.clone()).unwrap_or_default())
}

pub fn action_for(ctrl: &gtk::EventControllerKey, key: gtk::gdk::Key, code: u32,
                  state: gtk::gdk::ModifierType) -> Option<String> {
    CURRENT.with(|c| c.borrow().shortcuts.iter()
        .find(|(_, accels)| accels.iter()
            .any(|a| crate::util::shortcut_matches(a, ctrl, key, code, state)))
        .map(|(action, _)| action.clone()))
}

pub fn owner_of(accel: &str, except: &str) -> Option<String> {
    let parsed = gtk::accelerator_parse(accel);
    CURRENT.with(|c| c.borrow().shortcuts.iter()
        .find(|(a, accels)| a != except
              && accels.iter().any(|k| gtk::accelerator_parse(k) == parsed))
        .map(|(a, _)| a.clone()))
}

pub fn set_accel(action: &str, accel: Option<String>) {
    let parsed = accel.as_deref().and_then(gtk::accelerator_parse);
    update(|s| {
        for (a, accels) in s.shortcuts.iter_mut() {
            if a == action {
                *accels = accel.iter().cloned().collect();
            } else if parsed.is_some() {
                accels.retain(|k| gtk::accelerator_parse(k) != parsed);
            }
        }
    });
}

pub fn reset_shortcuts() {
    update(|s| s.shortcuts = default_shortcuts());
}
