use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{SystemTime, UNIX_EPOCH};

#[macro_export]
macro_rules! log {
    ($($a:tt)*) => { $crate::util::log_line(&format!($($a)*)) };
}

pub fn log_line(msg: &str) {
    if std::env::var_os("SHELLMAN_QUIET").is_some() {
        return;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let local = glib::DateTime::now_local()
        .ok()
        .and_then(|d| d.format("%H:%M:%S").ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    eprintln!("[shellman {local}.{:03}] {msg}", now.subsec_millis());
}

use gtk::glib;

pub fn guard<T>(what: &str, default: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(e) => {
            let msg = e.downcast_ref::<&str>().map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "?".into());
            log_line(&format!("panic caught ({what}): {msg}"));
            default
        }
    }
}

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn walk(root: &impl gtk::prelude::IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    use gtk::prelude::*;
    let mut out = Vec::new();
    let mut stack = vec![root.clone().upcast::<gtk::Widget>()];
    while let Some(w) = stack.pop() {
        let mut children = Vec::new();
        let mut c = w.first_child();
        while let Some(ch) = c {
            c = ch.next_sibling();
            children.push(ch);
        }
        out.push(w);
        stack.extend(children.into_iter().rev());
    }
    out
}

pub fn type_name(o: &impl gtk::prelude::IsA<glib::Object>) -> String {
    use gtk::prelude::*;
    o.type_().name().to_string()
}

pub fn shortcut_matches(accel: &str, ctrl: &gtk::EventControllerKey, keyval: gtk::gdk::Key,
                        keycode: u32, state: gtk::gdk::ModifierType) -> bool {
    let Some((key, mods)) = gtk::accelerator_parse(accel) else { return false };
    if state & gtk::accelerator_get_default_mod_mask() != mods {
        return false;
    }
    let key = key.to_lower();
    base_key(ctrl, keyval, keycode) == key || keyval.to_lower() == key
}

pub fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s.replace('\0', "")).unwrap_or_default()
}

pub fn copy_text(text: &str) -> bool {
    use gtk::prelude::*;
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    display.clipboard().set_text(text);
    true
}

pub fn base_key(ctrl: &gtk::EventControllerKey, keyval: gtk::gdk::Key, keycode: u32) -> gtk::gdk::Key {
    use gtk::prelude::*;
    ctrl.widget()
        .and_then(|w| w.display().translate_key(keycode, gtk::gdk::ModifierType::empty(),
                                                ctrl.group() as i32))
        .map(|(k, ..)| k)
        .unwrap_or(keyval)
        .to_lower()
}
