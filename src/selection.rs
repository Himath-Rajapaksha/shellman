use std::cell::Cell;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use gtk::glib::{self, prelude::*};
use gtk::prelude::*;

use crate::log;
use crate::nautilus;
use crate::shell;
use crate::util::walk;

pub fn view_of(root: &gtk::Widget) -> Option<(gtk::Widget, gtk::SelectionModel)> {
    walk(root).into_iter().filter(|w| w.is_mapped()).find_map(|w| {
        let model = if let Some(v) = w.downcast_ref::<gtk::GridView>() {
            v.model()
        } else if let Some(v) = w.downcast_ref::<gtk::ColumnView>() {
            v.model()
        } else {
            w.downcast_ref::<gtk::ListView>().and_then(|v| v.model())
        };
        model.map(|m| (w, m))
    })
}

#[derive(Default)]
pub struct Selection {
    pub paths: Vec<PathBuf>,
    /// selected items left out: no local path, or not a Nautilus file item
    pub skipped: usize,
}

thread_local! {
    static LAST_SKIPPED: Cell<usize> = const { Cell::new(0) };
}

fn item_path(item: glib::Object) -> Option<PathBuf> {
    let item = match item.downcast::<gtk::TreeListRow>() {
        Ok(row) => row.item()?,
        Err(item) => item,
    };
    if !item.find_property("file").is_some_and(|p| p.flags().contains(glib::ParamFlags::READABLE)) {
        return None;
    }
    nautilus::file_path(&item.property::<Option<glib::Object>>("file")?)
}

pub fn selected(slot: &glib::Object) -> Selection {
    let Some(root) = slot.downcast_ref::<gtk::Widget>() else { return Selection::default() };
    view_of(root).map(|(_, m)| selected_in(&m)).unwrap_or_default()
}

pub fn selected_in(model: &gtk::SelectionModel) -> Selection {
    let set = model.selection();
    let mut sel = Selection::default();
    for k in 0..set.size() as u32 {
        match model.item(set.nth(k)).and_then(item_path) {
            Some(p) => sel.paths.push(p),
            None => sel.skipped += 1,
        }
    }
    if sel.skipped != LAST_SKIPPED.replace(sel.skipped) && sel.skipped > 0 {
        log!("{} selected item(s) left out of SEL: no local path", sel.skipped);
    }
    sel
}

pub fn folder_target(sel: &Selection) -> Option<PathBuf> {
    let mut dirs = sel.paths.iter().filter(|p| p.is_dir());
    let first = dirs.next()?;
    if dirs.next().is_some() {
        return None;
    }
    Some(first.clone())
}

pub fn new_sel_file() -> PathBuf {
    shell::runtime_file("sel")
}

pub fn write_sel_file(file: &Path, paths: &[PathBuf]) {
    let mut content = Vec::new();
    for p in paths {
        content.extend_from_slice(p.as_os_str().as_bytes());
        content.push(0);
    }
    if let Err(e) = shell::write_atomic(file, &content) {
        log!("cannot write {}: {e}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_target_requires_exactly_one_directory() {
        let one = Selection { paths: vec![PathBuf::from("/")], skipped: 0 };
        assert_eq!(folder_target(&one), Some(PathBuf::from("/")));

        let with_file = Selection {
            paths: vec![PathBuf::from("/"), PathBuf::from("/nope-file")],
            skipped: 1,
        };
        assert_eq!(folder_target(&with_file), Some(PathBuf::from("/")));

        let file_only = Selection { paths: vec![PathBuf::from("/nope-file")], skipped: 0 };
        assert_eq!(folder_target(&file_only), None);

        let two = Selection {
            paths: vec![PathBuf::from("/"), PathBuf::from("/tmp")],
            skipped: 0,
        };
        assert_eq!(folder_target(&two), None);
    }
}
