use gtk::glib;

const ICONS: &[(&str, &str)] = &[
    ("shellman.svg", include_str!("../icons/shellman.svg")),
];

pub fn install() {
    let base = glib::user_runtime_dir().join("shellman").join("icons");
    let dir = base.join("hicolor").join("scalable").join("apps");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, svg) in ICONS {
        let _ = std::fs::write(dir.join(name), svg);
    }
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&base);
    }
}
