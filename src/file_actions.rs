use std::path::PathBuf;

use gtk::gio;
use gtk::gio::prelude::FileExt;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FileActions {
    pub open_here: Option<PathBuf>,
    pub insert: Vec<PathBuf>,
    pub copy: Vec<PathBuf>,
}

impl FileActions {
    pub fn from_paths(paths: Vec<PathBuf>, skipped: usize) -> FileActions {
        let open_here = match paths.as_slice() {
            [only] if skipped == 0 && only.is_dir() => Some(only.clone()),
            _ => None,
        };
        FileActions {
            open_here,
            insert: paths.clone(),
            copy: paths,
        }
    }
}

pub fn existing(paths: &[PathBuf]) -> (Vec<PathBuf>, usize) {
    let (found, missing): (Vec<PathBuf>, Vec<PathBuf>) =
        paths.iter().cloned().partition(|p| p.exists());
    (found, missing.len())
}

pub fn dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    unique
}

pub fn parse_uri_list(text: &str) -> Vec<PathBuf> {
    let paths = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| gio::File::for_uri(line).path())
        .filter(|path| path.is_absolute())
        .collect();
    dedupe(paths)
}

pub fn path_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn uri_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| gio::File::for_path(p).uri())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_folder_can_open_a_terminal() {
        let a = FileActions::from_paths(vec![PathBuf::from("/")], 0);
        assert_eq!(a.open_here, Some(PathBuf::from("/")));
        assert_eq!(a.insert, vec![PathBuf::from("/")]);
    }

    #[test]
    fn two_folders_cannot_open_a_terminal() {
        let a = FileActions::from_paths(vec![PathBuf::from("/"), PathBuf::from("/tmp")], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert.len(), 2);
    }

    #[test]
    fn a_file_and_a_folder_cannot_open_a_terminal() {
        let a = FileActions::from_paths(vec![PathBuf::from("/nope-file"), PathBuf::from("/")], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert.len(), 2);
    }

    #[test]
    fn a_remote_item_blocks_open_terminal_here() {
        let a = FileActions::from_paths(vec![PathBuf::from("/")], 1);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert, vec![PathBuf::from("/")]);
    }

    #[test]
    fn empty_selection_has_no_actions() {
        let a = FileActions::from_paths(Vec::new(), 0);
        assert_eq!(a.open_here, None);
        assert!(a.insert.is_empty());
        assert!(a.copy.is_empty());
    }

    #[test]
    fn vanished_paths_stay_in_insert_and_copy() {
        let p = PathBuf::from("/definitely-not-here-shellman");
        let a = FileActions::from_paths(vec![p.clone()], 0);
        assert_eq!(a.open_here, None);
        assert_eq!(a.insert, vec![p.clone()]);
        assert_eq!(a.copy, vec![p]);
    }

    #[test]
    fn existing_splits_paths_and_counts_missing() {
        let present = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let missing = present.with_extension("toml.shellman-missing");
        let (found, skipped) = existing(&[present.clone(), missing]);
        assert_eq!(found, vec![present]);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn dedupe_keeps_first_seen_order() {
        let a = PathBuf::from("/a");
        let b = PathBuf::from("/b");
        assert_eq!(dedupe(vec![a.clone(), b.clone(), a.clone()]), vec![a, b]);
    }

    #[test]
    fn parse_uri_list_ignores_comments_duplicates_and_non_files() {
        let text = "file:///tmp/a\n# comment\r\nfile:///tmp/a\nhttps://example.com/x\n";
        assert_eq!(parse_uri_list(text), vec![PathBuf::from("/tmp/a")]);
        assert_eq!(
            parse_uri_list("file:///tmp/a%20b"),
            vec![PathBuf::from("/tmp/a b")]
        );
    }

    #[test]
    fn clipboard_text_is_newline_separated() {
        let paths = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        assert_eq!(path_text(&paths), "/tmp/a\n/tmp/b");
        assert_eq!(uri_text(&paths), "file:///tmp/a\nfile:///tmp/b");
    }
}
