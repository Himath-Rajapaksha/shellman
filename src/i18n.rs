use std::path::Path;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, dgettext, dngettext};

pub const DOMAIN: &str = "shellman";

#[macro_export]
macro_rules! N_ {
    ($s:literal) => {
        $s
    };
}

// dgettext only: textdomain() would switch Nautilus's own language
pub fn init() {
    let build_dir = env!("SHELLMAN_BUILD_LOCALEDIR");
    let dev = !build_dir.is_empty() && Path::new(build_dir).is_dir();
    let dir = if dev { build_dir } else { env!("SHELLMAN_LOCALEDIR") };
    let _ = bindtextdomain(DOMAIN, dir);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
}

pub fn tr(msgid: &str) -> String {
    dgettext(DOMAIN, msgid)
}

pub fn ntr(singular: &str, plural: &str, n: u32) -> String {
    dngettext(DOMAIN, singular, plural, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gettextrs::{setlocale, LocaleCategory};

    #[test]
    fn follows_the_locale() {
        init();
        if unsafe { setlocale(LocaleCategory::LcAll, "C.UTF-8") }.is_some() {
            assert_eq!(tr("Copy"), "Copy");
        }
        if unsafe { setlocale(LocaleCategory::LcAll, "fr_FR.UTF-8") }.is_some() {
            assert_eq!(tr("Copy"), "Copier");
            assert_eq!(ntr("Insert the Selected File", "Insert the {n} Selected Files", 3),
                       "Insérer les {n} fichiers sélectionnés");
            assert_eq!(ntr("Insert the Selected File", "Insert the {n} Selected Files", 1),
                       "Insérer le fichier sélectionné");
        }
    }
}
