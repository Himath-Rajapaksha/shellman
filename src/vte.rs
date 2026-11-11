#![allow(non_camel_case_types)]

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::os::raw::{c_char, c_int, c_long};
use std::path::{Path, PathBuf};
use std::ptr;

use gtk::gdk::ffi::GdkRGBA;
use gtk::gio::{self, prelude::FileExt};
use gtk::glib::{self, ffi::gpointer, prelude::*, translate::*};
use gtk::pango;

use crate::log;
use crate::util::cstr;
use crate::util::guard;

pub enum VteTerminalC {}
pub enum VtePtyC {}

type SpawnCallback =
    unsafe extern "C" fn(*mut VteTerminalC, glib::ffi::GPid, *mut glib::ffi::GError, gpointer);

const VTE_PTY_DEFAULT: c_int = 0;
const VTE_FORMAT_TEXT: c_int = 1;
const G_SPAWN_SEARCH_PATH: c_int = 1 << 2;

#[link(name = "libvte-2.91-gtk4.so.0", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    fn vte_terminal_new() -> *mut gtk::ffi::GtkWidget;
    #[allow(clippy::too_many_arguments)]
    fn vte_terminal_spawn_async(
        t: *mut VteTerminalC, pty_flags: c_int, working_directory: *const c_char,
        argv: *mut *mut c_char, envv: *mut *mut c_char, spawn_flags: c_int,
        child_setup: Option<unsafe extern "C" fn(gpointer)>, child_setup_data: gpointer,
        child_setup_data_destroy: Option<unsafe extern "C" fn(gpointer)>, timeout: c_int,
        cancellable: *mut gio::ffi::GCancellable, callback: Option<SpawnCallback>,
        user_data: gpointer);
    fn vte_terminal_feed_child(t: *mut VteTerminalC, text: *const c_char, length: isize);
    fn vte_terminal_copy_clipboard_format(t: *mut VteTerminalC, format: c_int);
    fn vte_terminal_paste_clipboard(t: *mut VteTerminalC);
    fn vte_terminal_set_scrollback_lines(t: *mut VteTerminalC, lines: c_long);
    fn vte_terminal_get_pty(t: *mut VteTerminalC) -> *mut VtePtyC;
    fn vte_pty_get_fd(p: *mut VtePtyC) -> c_int;
    fn vte_terminal_set_colors(t: *mut VteTerminalC, fg: *const GdkRGBA, bg: *const GdkRGBA,
                               palette: *const GdkRGBA, palette_size: usize);
    fn vte_terminal_set_color_cursor(t: *mut VteTerminalC, color: *const GdkRGBA);
    fn vte_terminal_set_font(t: *mut VteTerminalC,
                             desc: *const pango::ffi::PangoFontDescription);
    fn vte_terminal_get_current_directory_uri(t: *mut VteTerminalC) -> *const c_char;
    fn vte_get_user_shell() -> *mut c_char;
    fn vte_terminal_get_has_selection(t: *mut VteTerminalC) -> glib::ffi::gboolean;
    fn vte_terminal_select_all(t: *mut VteTerminalC);
    fn vte_terminal_get_char_height(t: *mut VteTerminalC) -> c_long;
}


fn rgba(c: &gtk::gdk::RGBA) -> GdkRGBA {
    GdkRGBA { red: c.red(), green: c.green(), blue: c.blue(), alpha: c.alpha() }
}

pub fn user_shell() -> String {
    unsafe {
        let p = vte_get_user_shell();
        if !p.is_null() {
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            glib::ffi::g_free(p as gpointer);
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}

type OnSpawn = Box<dyn FnOnce(Option<i32>)>;

unsafe extern "C" fn spawn_done(_t: *mut VteTerminalC, pid: glib::ffi::GPid,
                                err: *mut glib::ffi::GError, data: gpointer) {
    let cb = Box::from_raw(data as *mut OnSpawn);
    guard("shell spawn", (), move || {
        if !err.is_null() {
            let msg = CStr::from_ptr((*err).message).to_string_lossy();
            log!("cannot start the shell: {msg}");
        }
        cb((pid > 0).then_some(pid));
    })
}

#[derive(Clone)]
pub struct Terminal(gtk::Widget);

impl Default for Terminal {
    fn default() -> Self {
        Terminal::new()
    }
}

impl Terminal {
    pub fn new() -> Terminal {
        let w: gtk::Widget = unsafe { from_glib_none(vte_terminal_new()) };
        Terminal(w)
    }

    pub fn widget(&self) -> &gtk::Widget {
        &self.0
    }

    fn ptr(&self) -> *mut VteTerminalC {
        self.0.as_ptr() as *mut VteTerminalC
    }

    pub fn spawn(&self, dir: &Path, argv: &[String], env: &[String],
                 done: impl FnOnce(Option<i32>) + 'static) {
        let wd = CString::new(dir.as_os_str().as_bytes()).unwrap_or_default();
        let argv_c: Vec<CString> = argv.iter().map(|a| cstr(a)).collect();
        let env_c: Vec<CString> = env.iter().map(|e| cstr(e)).collect();
        let nul_list = |v: &[CString]| -> Vec<*mut c_char> {
            v.iter().map(|c| c.as_ptr() as *mut c_char).chain([ptr::null_mut()]).collect()
        };
        let mut argv = nul_list(&argv_c);
        let mut envv = nul_list(&env_c);
        let data: Box<OnSpawn> = Box::new(Box::new(done));
        unsafe {
            vte_terminal_spawn_async(
                self.ptr(), VTE_PTY_DEFAULT, wd.as_ptr(), argv.as_mut_ptr(), envv.as_mut_ptr(),
                G_SPAWN_SEARCH_PATH, None, ptr::null_mut(), None, -1, ptr::null_mut(),
                Some(spawn_done), Box::into_raw(data) as gpointer);
        }
    }

    pub fn feed(&self, text: &str) {
        unsafe { vte_terminal_feed_child(self.ptr(), text.as_ptr() as *const c_char,
                                         text.len() as isize) }
    }

    pub fn shell_idle(&self, pid: i32) -> bool {
        unsafe {
            let pty = vte_terminal_get_pty(self.ptr());
            if pty.is_null() {
                return false;
            }
            let fd = vte_pty_get_fd(pty);
            fd >= 0 && libc::tcgetpgrp(fd) == pid
        }
    }

    pub fn cwd(&self) -> Option<PathBuf> {
        let p = unsafe { vte_terminal_get_current_directory_uri(self.ptr()) };
        if p.is_null() {
            return None;
        }
        let uri = unsafe { CStr::from_ptr(p) }.to_string_lossy();
        gio::File::for_uri(&uri).path()
    }

    pub fn copy(&self) {
        unsafe { vte_terminal_copy_clipboard_format(self.ptr(), VTE_FORMAT_TEXT) }
    }

    pub fn has_selection(&self) -> bool {
        unsafe { vte_terminal_get_has_selection(self.ptr()) != 0 }
    }

    pub fn char_height(&self) -> i32 {
        unsafe { vte_terminal_get_char_height(self.ptr()) as i32 }
    }

    pub fn select_all(&self) {
        unsafe { vte_terminal_select_all(self.ptr()) }
    }

    pub fn paste(&self) {
        unsafe { vte_terminal_paste_clipboard(self.ptr()) }
    }

    pub fn set_scrollback(&self, lines: i64) {
        unsafe { vte_terminal_set_scrollback_lines(self.ptr(), lines as c_long) }
    }

    pub fn set_colors(&self, fg: &gtk::gdk::RGBA, bg: &gtk::gdk::RGBA, palette: &[gtk::gdk::RGBA],
                      cursor: Option<&gtk::gdk::RGBA>) {
        let (fg, bg) = (rgba(fg), rgba(bg));
        let palette: Vec<GdkRGBA> = palette.iter().map(rgba).collect();
        let p = if palette.is_empty() { ptr::null() } else { palette.as_ptr() };
        let cursor = cursor.map(rgba);
        unsafe {
            vte_terminal_set_colors(self.ptr(), &fg, &bg, p, palette.len());
            vte_terminal_set_color_cursor(self.ptr(), cursor.as_ref().map_or(ptr::null(), |c| c as *const _));
        }
    }

    pub fn set_font(&self, desc: &str) {
        let d = pango::FontDescription::from_string(desc);
        unsafe { vte_terminal_set_font(self.ptr(), d.to_glib_none().0) }
    }

    pub fn connect_child_exited(&self, f: impl Fn() + 'static) {
        self.0.connect_local("child-exited", false, move |_| {
            guard("shell exit", (), &f);
            None
        });
    }

    pub fn connect_contents_changed(&self, f: impl Fn() + 'static) {
        self.0.connect_local("contents-changed", false, move |_| {
            guard("terminal contents", (), &f);
            None
        });
    }

    pub fn connect_cwd_changed(&self, f: impl Fn() + 'static) {
        if glib::subclass::SignalId::lookup("current-directory-uri-changed", self.0.type_()).is_none() {
            log!("signal current-directory-uri-changed missing");
            return;
        }
        self.0.connect_local("current-directory-uri-changed", false, move |_| {
            guard("shell directory", (), &f);
            None
        });
    }
}
