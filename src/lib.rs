#![allow(clippy::missing_safety_doc)]

pub mod colors;
pub mod config;
pub mod file_actions;
pub mod i18n;
pub mod icons;
pub mod location;
pub mod nautilus;
pub mod pane;
pub mod panel;
pub mod prefs;
pub mod selection;
pub mod settings;
pub mod shell;
pub mod status;
pub mod util;
pub mod vte;

use std::os::raw::c_int;
use std::ptr;

use gtk::glib::{ffi::GList, gobject_ffi};

use nautilus::{gpointer, GType, MenuItem, NautilusFileInfoC};
use i18n::tr;
use util::guard;

static mut TYPES: [GType; 1] = [0];

unsafe extern "C" fn get_file_items(_p: gpointer, _files: *mut GList) -> *mut GList {
    ptr::null_mut()
}

unsafe extern "C" fn get_background_items(_p: gpointer, _f: *mut NautilusFileInfoC) -> *mut GList {
    guard("background menu", ptr::null_mut(), || {
        let item = MenuItem::new("ShellMan::toggle", &tr("Embedded Terminal"),
                                 &tr("Show or hide the terminal at the bottom of the window"));
        item.on_activate(panel::toggle_active);
        nautilus::menu_list(vec![item])
    })
}

unsafe extern "C" fn menu_iface_init(iface: gpointer, _: gpointer) {
    let i = &mut *(iface as *mut nautilus::MenuProviderInterface);
    i.get_file_items = Some(get_file_items);
    i.get_background_items = Some(get_background_items);
}

unsafe fn register(module: *mut gobject_ffi::GTypeModule) -> GType {
    let mut q: gobject_ffi::GTypeQuery = std::mem::zeroed();
    gobject_ffi::g_type_query(gobject_ffi::G_TYPE_OBJECT, &mut q);
    let info = gobject_ffi::GTypeInfo {
        class_size: q.class_size as u16,
        base_init: None,
        base_finalize: None,
        class_init: None,
        class_finalize: None,
        class_data: ptr::null(),
        instance_size: q.instance_size as u16,
        n_preallocs: 0,
        instance_init: None,
        value_table: ptr::null(),
    };
    let t = gobject_ffi::g_type_module_register_type(
        module, gobject_ffi::G_TYPE_OBJECT, c"ShellMan".as_ptr(), &info, 0);
    let iface = gobject_ffi::GInterfaceInfo {
        interface_init: Some(menu_iface_init),
        interface_finalize: None,
        interface_data: ptr::null_mut(),
    };
    gobject_ffi::g_type_module_add_interface(module, t, nautilus::nautilus_menu_provider_get_type(),
                                             &iface);
    t
}

#[no_mangle]
pub unsafe extern "C" fn nautilus_module_initialize(module: *mut gobject_ffi::GTypeModule) {
    guard("initialization", (), || {
        i18n::init();
        let t = register(module);
        *ptr::addr_of_mut!(TYPES) = [t];
        log!("module loaded (ShellMan {})", env!("CARGO_PKG_VERSION"));
        panel::start();
    })
}

#[no_mangle]
pub unsafe extern "C" fn nautilus_module_list_types(types: *mut *const GType, n: *mut c_int) {
    *types = ptr::addr_of!(TYPES) as *const GType;
    *n = if (*ptr::addr_of!(TYPES))[0] == 0 { 0 } else { 1 };
}

#[no_mangle]
pub extern "C" fn nautilus_module_shutdown() {
    log!("module shutdown");
}
