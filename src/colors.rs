use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gtk::gdk::RGBA;
use gtk::gio::{self, prelude::*};
use gtk::glib;

use crate::settings;
use crate::util::guard;

pub const FOLLOW_TERMINAL: &str = "terminal";
pub const NAUTILUS: &str = "nautilus";
pub const CUSTOM: &str = "custom";

const GNOME: &str = "gnome";
const BUILTIN: &[(&str, &str)] = &[
    ("gnome", include_str!("../palettes/gnome.palette")),
    ("gnome-high-contrast", include_str!("../palettes/gnome-high-contrast.palette")),
    ("Ubuntu", include_str!("../palettes/Ubuntu.palette")),
    ("tango", include_str!("../palettes/tango.palette")),
    ("linux", include_str!("../palettes/linux.palette")),
    ("xterm", include_str!("../palettes/xterm.palette")),
    ("campbell", include_str!("../palettes/campbell.palette")),
    ("Vs Code", include_str!("../palettes/Vs Code.palette")),
    ("solarized", include_str!("../palettes/solarized.palette")),
    ("dracula", include_str!("../palettes/dracula.palette")),
    ("nord", include_str!("../palettes/nord.palette")),
    ("Horizon", include_str!("../palettes/Horizon.palette")),
];

/// fg None: text color of the theme; bg None: transparent, the file view shows through
#[derive(Clone, Default)]
pub struct Variant {
    pub fg: Option<RGBA>,
    pub bg: Option<RGBA>,
    pub cursor: Option<RGBA>,
    pub palette: Vec<RGBA>,
}

#[derive(Clone)]
struct Scheme {
    light: Variant,
    dark: Variant,
    force_dark: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Terminal {
    Ptyxis,
    GnomeTerminal,
}

impl Terminal {
    pub fn name(self) -> &'static str {
        match self {
            Terminal::Ptyxis => "Ptyxis",
            Terminal::GnomeTerminal => "GNOME Terminal",
        }
    }
}

fn parse(text: &str) -> Option<(String, Variant, Variant)> {
    let kf = glib::KeyFile::new();
    kf.load_from_data(text, glib::KeyFileFlags::NONE).ok()?;
    let variant = |group: &str| -> Option<Variant> {
        let get = |key: &str| kf.string(group, key).ok().and_then(|s| RGBA::parse(s.as_str()).ok());
        let palette: Vec<RGBA> = (0..16).filter_map(|i| get(&format!("Color{i}"))).collect();
        Some(Variant {
            fg: Some(get("Foreground")?),
            bg: Some(get("Background")?),
            cursor: get("Cursor"),
            palette: if palette.len() == 16 { palette } else { Vec::new() },
        })
    };
    let name = kf.string("Palette", "Name").map(|s| s.to_string()).unwrap_or_default();
    Some((name, variant("Light")?, variant("Dark")?))
}

pub fn builtin() -> Vec<(&'static str, String)> {
    BUILTIN.iter()
        .filter_map(|(id, text)| parse(text).map(|(name, ..)| (*id, if name.is_empty() { id.to_string() } else { name })))
        .collect()
}

fn builtin_scheme(id: &str) -> Option<Scheme> {
    let (_, text) = BUILTIN.iter().find(|(i, _)| i.eq_ignore_ascii_case(id))?;
    let (_, light, dark) = parse(text)?;
    Some(Scheme { light, dark, force_dark: None })
}

fn gnome() -> Scheme {
    builtin_scheme(GNOME).unwrap_or(Scheme { light: Variant::default(), dark: Variant::default(), force_dark: None })
}

fn schema(id: &str) -> Option<gio::SettingsSchema> {
    gio::SettingsSchemaSource::default()?.lookup(id, true)
}

fn open(id: &str, path: Option<&str>) -> Option<(gio::Settings, gio::SettingsSchema)> {
    let schema = schema(id)?;
    if schema.path().is_none() && path.is_none() {
        return None;
    }
    let s = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, path);
    Some((s, schema))
}

fn string(s: &(gio::Settings, gio::SettingsSchema), key: &str) -> Option<String> {
    s.1.has_key(key).then(|| s.0.string(key).to_string())
}

fn boolean(s: &(gio::Settings, gio::SettingsSchema), key: &str) -> Option<bool> {
    s.1.has_key(key).then(|| s.0.boolean(key))
}

fn style(value: Option<String>) -> Option<bool> {
    match value.as_deref() {
        Some("dark") => Some(true),
        Some("light") => Some(false),
        _ => None,
    }
}

fn installed(t: Terminal) -> bool {
    match t {
        Terminal::Ptyxis => schema("org.gnome.Ptyxis").is_some(),
        Terminal::GnomeTerminal => schema("org.gnome.Terminal.Legacy.Profile").is_some(),
    }
}

/// The user's terminal, as chosen by xdg-terminal-exec
pub fn default_terminal() -> Option<Terminal> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
        .split(':').filter(|d| !d.is_empty()).map(str::to_lowercase).collect();
    let mut dirs: Vec<PathBuf> = vec![glib::user_config_dir()];
    dirs.extend(glib::system_config_dirs());
    dirs.push(glib::user_data_dir().join("xdg-terminal-exec"));
    dirs.extend(glib::system_data_dirs().into_iter().map(|d| d.join("xdg-terminal-exec")));
    let names: Vec<String> = desktops.iter().map(|d| format!("{d}-xdg-terminals.list"))
        .chain(std::iter::once("xdg-terminals.list".into())).collect();
    for dir in &dirs {
        for name in &names {
            if let Some(t) = first_entry(&dir.join(name)) {
                return t;
            }
        }
    }
    [Terminal::Ptyxis, Terminal::GnomeTerminal].into_iter().find(|t| installed(*t))
}

/// Some(None): another terminal comes first
fn first_entry(file: &Path) -> Option<Option<Terminal>> {
    let text = std::fs::read_to_string(file).ok()?;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let id = line.trim_start_matches('+').split(':').next().unwrap_or("");
        let t = match id {
            "org.gnome.Ptyxis.desktop" => Terminal::Ptyxis,
            "org.gnome.Terminal.desktop" => Terminal::GnomeTerminal,
            _ => {
                if gio::DesktopAppInfo::new(id).is_some() {
                    return Some(None);
                }
                continue;
            }
        };
        if installed(t) {
            return Some(Some(t));
        }
    }
    None
}

fn ptyxis_palette(id: &str) -> Scheme {
    let file = glib::user_data_dir().join("org.gnome.Ptyxis").join("palettes").join(format!("{id}.palette"));
    if let Some((_, light, dark)) = std::fs::read_to_string(file).ok().as_deref().and_then(parse) {
        return Scheme { light, dark, force_dark: None };
    }
    builtin_scheme(id)
        .or_else(|| BUILTIN.iter().find(|(_, t)| parse(t).is_some_and(|(n, ..)| n.eq_ignore_ascii_case(id)))
             .and_then(|(i, _)| builtin_scheme(i)))
        .unwrap_or_else(gnome)
}

fn ptyxis(watch: &mut Vec<gio::Settings>) -> Option<Scheme> {
    let app = open("org.gnome.Ptyxis", None)?;
    let uuid = string(&app, "default-profile-uuid").unwrap_or_default();
    let palette = if uuid.is_empty() {
        schema("org.gnome.Ptyxis.Profile")?.key("palette").default_value().str().map(String::from)
    } else {
        let profile = open("org.gnome.Ptyxis.Profile", Some(&format!("/org/gnome/Ptyxis/Profiles/{uuid}/")))?;
        let p = string(&profile, "palette");
        watch.push(profile.0);
        p
    };
    let mut scheme = ptyxis_palette(palette.as_deref().unwrap_or(GNOME));
    scheme.force_dark = style(string(&app, "interface-style"));
    watch.push(app.0);
    Some(scheme)
}

fn css_block<'a>(css: &'a str, selector: &str) -> Option<&'a str> {
    let start = css.find(&format!("{selector} {{"))? + selector.len() + 2;
    let len = css[start..].find('}')?;
    Some(&css[start..start + len])
}

fn css_color(block: &str, property: &str) -> Option<RGBA> {
    block.split(';').find_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        (k.trim() == property).then(|| RGBA::parse(v.trim()).ok()).flatten()
    })
}

/// Text and background that a GTK 3 theme (Yaru…) gives to GNOME Terminal
fn gtk3_terminal_colors(theme: &str, dark: bool) -> Option<(RGBA, RGBA)> {
    if theme.is_empty() || theme.contains('/') || theme.starts_with('.') {
        return None;
    }
    let mut dirs = vec![glib::user_data_dir().join("themes"), glib::home_dir().join(".themes")];
    dirs.extend(glib::system_data_dirs().into_iter().map(|d| d.join("themes")));
    let dir = dirs.into_iter().map(|d| d.join(theme).join("gtk-3.0")).find(|d| d.is_dir())?;
    let file = if dark && dir.join("gtk-dark.css").is_file() { "gtk-dark.css" } else { "gtk.css" };
    let mut css = std::fs::read_to_string(dir.join(file)).ok()?;
    let resource = gio::Resource::load(dir.join("gtk.gresource")).ok();
    for line in css.clone().lines() {
        let Some(path) = line.trim().strip_prefix("@import url(\"resource://")
            .and_then(|r| r.strip_suffix("\");")) else { continue };
        if let Some(data) = resource.as_ref()
            .and_then(|r| r.lookup_data(path, gio::ResourceLookupFlags::NONE).ok()) {
            css.push_str(&String::from_utf8_lossy(&data));
        }
    }
    let block = css_block(&css, "terminal-window .terminal-screen")?;
    Some((css_color(block, "color")?, css_color(block, "background-color")?))
}

fn gnome_terminal(watch: &mut Vec<gio::Settings>) -> Option<Scheme> {
    let list = open("org.gnome.Terminal.ProfilesList", None)?;
    let uuid = string(&list, "default").filter(|u| !u.is_empty())?;
    let profile = open("org.gnome.Terminal.Legacy.Profile",
                       Some(&format!("/org/gnome/terminal/legacy/profiles:/:{uuid}/")))?;
    let color = |key: &str| string(&profile, key).and_then(|s| RGBA::parse(s.as_str()).ok());
    let palette: Vec<RGBA> = profile.1.has_key("palette").then(|| profile.0.strv("palette"))
        .map(|v| v.iter().filter_map(|c| RGBA::parse(c.as_str()).ok()).collect())
        .unwrap_or_default();
    let palette = if palette.len() >= 16 { palette[..16].to_vec() } else { Vec::new() };
    let theme = boolean(&profile, "use-theme-colors").unwrap_or(true);
    let cursor = boolean(&profile, "cursor-colors-set").unwrap_or(false)
        .then(|| color("cursor-background-color")).flatten();
    let interface = open("org.gnome.desktop.interface", None);
    let gtk_theme = interface.as_ref().and_then(|i| string(i, "gtk-theme")).unwrap_or_default();
    if let Some(i) = interface {
        watch.push(i.0);
    }
    let v = |fallback: &Variant, dark: bool| {
        let (fg, bg) = if theme {
            gtk3_terminal_colors(&gtk_theme, dark).map_or((None, None), |(f, b)| (Some(f), Some(b)))
        } else {
            (color("foreground-color").or(fallback.fg), color("background-color").or(fallback.bg))
        };
        Variant {
            fg, bg, cursor,
            palette: if palette.is_empty() { fallback.palette.clone() } else { palette.clone() },
        }
    };
    let g = gnome();
    let force_dark = open("org.gnome.Terminal.Legacy.Settings", None)
        .and_then(|s| { let v = style(string(&s, "theme-variant")); watch.push(s.0); v });
    let scheme = Scheme { light: v(&g.light, false), dark: v(&g.dark, true), force_dark };
    watch.push(list.0);
    watch.push(profile.0);
    Some(scheme)
}

fn nautilus() -> Scheme {
    let g = gnome();
    let clear = |v: Variant| Variant { fg: None, bg: None, cursor: None, palette: v.palette };
    Scheme { light: clear(g.light), dark: clear(g.dark), force_dark: None }
}

fn custom(s: &settings::Settings) -> Scheme {
    let g = gnome();
    let parse = |c: &str| RGBA::parse(c).ok();
    let v = |base: Variant, fg: &str, bg: &str| Variant {
        fg: parse(fg).or(base.fg), bg: parse(bg).or(base.bg), cursor: None, palette: base.palette,
    };
    Scheme {
        light: v(g.light, &s.custom_light_fg, &s.custom_light_bg),
        dark: v(g.dark, &s.custom_dark_fg, &s.custom_dark_bg),
        force_dark: None,
    }
}

thread_local! {
    static WATCH: RefCell<Vec<gio::Settings>> = const { RefCell::new(Vec::new()) };
}

fn scheme() -> Scheme {
    let s = settings::get();
    let mut watch = Vec::new();
    let scheme = match s.palette.as_str() {
        FOLLOW_TERMINAL => match default_terminal() {
            Some(Terminal::Ptyxis) => ptyxis(&mut watch),
            Some(Terminal::GnomeTerminal) => gnome_terminal(&mut watch),
            None => None,
        }.unwrap_or_else(nautilus),
        NAUTILUS => nautilus(),
        CUSTOM => custom(&s),
        id => builtin_scheme(id).unwrap_or_else(nautilus),
    };
    for w in &watch {
        w.connect_changed(None, |_, _| guard("terminal colors", (), || {
            glib::idle_add_local_once(|| guard("terminal colors", (), crate::panel::restyle_all));
        }));
        // GSettings only reports keys read after connecting
        if let Some(schema) = w.settings_schema() {
            for key in schema.list_keys() {
                let _ = w.value(&key);
            }
        }
    }
    WATCH.with(|c| c.replace(watch));
    scheme
}

/// Colors to use now; dark: whether the variant is dark
pub fn current() -> (Variant, bool) {
    let scheme = scheme();
    let system_dark = adw::StyleManager::default().is_dark();
    let dark = match settings::with(|s| s.style.clone()).as_str() {
        "dark" => true,
        "light" => false,
        _ => scheme.force_dark.unwrap_or(system_dark),
    };
    let mut v = if dark { scheme.dark } else { scheme.light };
    if (v.fg.is_none() || v.bg.is_none()) && dark != system_dark {
        // the file view has the other style: it cannot show through
        let g = gnome();
        let g = if dark { g.dark } else { g.light };
        v.fg = v.fg.or(g.fg);
        v.bg = v.bg.or(g.bg);
    }
    (v, dark)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_palettes_are_complete() {
        for (id, text) in BUILTIN {
            let (name, light, dark) = parse(text).unwrap_or_else(|| panic!("{id}"));
            assert!(!name.is_empty(), "{id}");
            assert_eq!(light.palette.len(), 16, "{id}");
            assert_eq!(dark.palette.len(), 16, "{id}");
        }
    }

    #[test]
    fn theme_terminal_css() {
        let css = "a { color: red; }\nterminal-window .terminal-screen {\n  background-color: #300A24;\n  color: white; }\n";
        let block = css_block(css, "terminal-window .terminal-screen").unwrap();
        assert_eq!(css_color(block, "background-color"), RGBA::parse("#300a24").ok());
        assert_eq!(css_color(block, "color"), RGBA::parse("white").ok());
        assert_eq!(css_block(css, "nothing"), None);
    }

    #[test]
    fn yaru_terminal_colors() {
        if !Path::new("/usr/share/themes/Yaru-dark/gtk-3.0").is_dir() {
            return;
        }
        let (fg, bg) = gtk3_terminal_colors("Yaru-dark", true).unwrap();
        assert_eq!(bg, RGBA::parse("#300a24").unwrap());
        assert_eq!(fg, RGBA::parse("white").unwrap());
        assert!(gtk3_terminal_colors("../Yaru-dark", true).is_none());
    }

    #[test]
    fn terminal_list_entries() {
        let dir = std::env::temp_dir().join(format!("shellman-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("xdg-terminals.list");
        std::fs::write(&f, "# comment\n\n-org.gnome.Ptyxis.desktop\nno.such.terminal.desktop\n").unwrap();
        assert_eq!(first_entry(&f), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
