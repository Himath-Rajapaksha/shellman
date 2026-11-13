use crate::i18n::{ntr, tr};

pub fn summary(files: usize, skipped: usize, shortcut: Option<&str>) -> String {
    let mut text = if files == 0 {
        tr("No files selected")
    } else {
        ntr("{n} file selected", "{n} files selected", files as u32)
            .replace("{n}", &files.to_string())
    };
    if files > 0 {
        if let Some(shortcut) = shortcut {
            text.push_str(" · ");
            text.push_str(&tr("{shortcut} to insert").replace("{shortcut}", shortcut));
        }
    }
    if skipped > 0 {
        text.push_str(" · ");
        text.push_str(
            &ntr(
                "{n} remote item skipped",
                "{n} remote items skipped",
                skipped as u32,
            )
            .replace("{n}", &skipped.to_string()),
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_selection_says_so() {
        assert_eq!(summary(0, 0, Some("Ctrl+Shift+S")), tr("No files selected"));
    }

    #[test]
    fn one_file_uses_the_singular_form() {
        let s = summary(1, 0, Some("Ctrl+Shift+S"));
        assert_eq!(
            s,
            ntr("{n} file selected", "{n} files selected", 1).replace("{n}", "1")
                + " · "
                + &tr("{shortcut} to insert").replace("{shortcut}", "Ctrl+Shift+S")
        );
    }

    #[test]
    fn many_files_use_the_plural_form_and_shortcut() {
        let s = summary(3, 0, Some("Ctrl+Shift+S"));
        assert!(s.contains('3'));
        assert!(s.contains("Ctrl+Shift+S"));
    }

    #[test]
    fn disabled_shortcut_is_omitted() {
        let s = summary(2, 0, None);
        assert!(!s.contains("to insert"));
        assert!(s.contains('2'));
    }

    #[test]
    fn skipped_items_are_reported() {
        let s = summary(2, 1, Some("Ctrl+Shift+S"));
        assert!(s.contains('2'));
        assert!(s.contains('1'));
        assert!(s.contains(" · "));
    }

    #[test]
    fn skipped_only_selection_still_reports_them() {
        let s = summary(0, 2, None);
        assert_eq!(
            s,
            tr("No files selected")
                + " · "
                + &ntr("{n} remote item skipped", "{n} remote items skipped", 2)
                    .replace("{n}", "2")
        );
    }
}
