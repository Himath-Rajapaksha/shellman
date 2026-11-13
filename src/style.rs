pub fn install() {
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        ".shellman-drop-target { box-shadow: inset 0 0 0 2px @accent_color; }",
    );
    gtk::style_context_add_provider_for_display(
        &display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}
