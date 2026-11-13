#![allow(non_camel_case_types)]

use std::os::raw::c_char;
use std::path::PathBuf;

use gtk::gio::{self, prelude::FileExt};

use crate::util::cstr;
use gtk::glib::{self, ffi::GList, gobject_ffi, prelude::*, translate::*};

pub type GType = glib::ffi::GType;
pub type gpointer = glib::ffi::gpointer;

pub enum NautilusFileInfoC {}
pub enum NautilusMenuItemC {}

#[repr(C)]
pub struct MenuProviderInterface {
    pub g_iface: gobject_ffi::GTypeInterface,
    pub get_file_items: Option<unsafe extern "C" fn(gpointer, *mut GList) -> *mut GList>,
    pub get_background_items:
        Option<unsafe extern "C" fn(gpointer, *mut NautilusFileInfoC) -> *mut GList>,
}

#[link(name = "libnautilus-extension.so.4", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn nautilus_menu_provider_get_type() -> GType;
    fn nautilus_file_info_get_type() -> GType;
    fn nautilus_menu_item_new(name: *const c_char, label: *const c_char,
                              tip: *const c_char, icon: *const c_char) -> *mut NautilusMenuItemC;
    fn nautilus_file_info_get_location(f: *mut NautilusFileInfoC) -> *mut gio::ffi::GFile;
}


pub fn file_path(file: &glib::Object) -> Option<PathBuf> {
    let iface = unsafe { glib::Type::from_glib(nautilus_file_info_get_type()) };
    if !file.type_().is_a(iface) {
        return None;
    }
    let loc: Option<gio::File> = unsafe {
        from_glib_full(nautilus_file_info_get_location(file.as_ptr() as *mut NautilusFileInfoC))
    };
    loc?.path()
}

pub fn file_paths(list: *mut GList) -> (Vec<PathBuf>, usize) {
    let mut paths = Vec::new();
    let mut skipped = 0;
    let mut node = list;
    while !node.is_null() {
        let data = unsafe { (*node).data };
        let path = if data.is_null() {
            None
        } else {
            let obj: glib::Object =
                unsafe { from_glib_none(data as *mut gobject_ffi::GObject) };
            file_path(&obj)
        };
        match path {
            Some(p) => paths.push(p),
            None => skipped += 1,
        }
        node = unsafe { (*node).next };
    }
    (paths, skipped)
}

pub struct MenuItem(glib::Object);

impl MenuItem {
    pub fn new(name: &str, label: &str, tip: &str) -> MenuItem {
        let ptr = unsafe {
            nautilus_menu_item_new(cstr(name).as_ptr(), cstr(label).as_ptr(),
                                   cstr(tip).as_ptr(), std::ptr::null())
        };
        MenuItem(unsafe { from_glib_full(ptr as *mut gobject_ffi::GObject) })
    }

    pub fn on_activate(&self, f: impl Fn() + 'static) {
        self.0.connect_local("activate", false, move |_| {
            crate::util::guard("menu item", (), &f);
            None
        });
    }
}

pub fn menu_list(items: Vec<MenuItem>) -> *mut GList {
    let mut list = std::ptr::null_mut();
    for i in items {
        let p: *mut gobject_ffi::GObject = i.0.into_glib_ptr();
        list = unsafe { glib::ffi::g_list_append(list, p as gpointer) };
    }
    list
}
