use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gio;
use gtk::glib::{self, prelude::*};
use gtk::prelude::*;

use crate::log;
use crate::util::{guard, type_name, walk};

// some Nautilus properties exist but are write-only: reading them panics
fn readable(obj: &impl IsA<glib::Object>, name: &str) -> bool {
    obj.find_property(name).is_some_and(|p| p.flags().contains(glib::ParamFlags::READABLE))
}

fn object_prop(obj: &impl IsA<glib::Object>, name: &str) -> Option<glib::Object> {
    if !readable(obj, name) {
        return None;
    }
    obj.property_value(name).get::<Option<glib::Object>>().ok().flatten()
}

fn gfile_prop(obj: &glib::Object, name: &str) -> (Option<gio::File>, bool) {
    match obj.find_property(name) {
        Some(spec) if spec.value_type().is_a(gio::File::static_type())
            && spec.flags().contains(glib::ParamFlags::READABLE) => {
            (obj.property_value(name).get::<Option<gio::File>>().ok().flatten(), true)
        }
        _ => (None, false),
    }
}

fn prop_names(obj: &glib::Object) -> Vec<String> {
    let mut v: Vec<String> = obj.list_properties().iter().map(|p| p.name().to_string()).collect();
    v.sort();
    v
}

struct Inner {
    slot: Option<glib::Object>,
    handler: Option<glib::SignalHandlerId>,
    logged: bool,
}

pub struct LocationSource {
    win: gtk::Window,
    on_change: Box<dyn Fn()>,
    inner: RefCell<Inner>,
}

impl LocationSource {
    pub fn new(win: &gtk::Window, on_change: impl Fn() + 'static) -> Rc<LocationSource> {
        let src = Rc::new(LocationSource {
            win: win.clone(),
            on_change: Box::new(on_change),
            inner: RefCell::new(Inner { slot: None, handler: None, logged: false }),
        });
        if win.find_property("active-slot").is_some() {
            let weak = Rc::downgrade(&src);
            win.connect_notify_local(Some("active-slot"), move |_, _| {
                if let Some(s) = weak.upgrade() {
                    guard("active tab", (), || s.rebind(true));
                }
            });
        }
        src.rebind(false);
        src
    }

    fn find_slot(&self) -> Option<(glib::Object, String)> {
        if self.win.find_property("active-slot").is_some() {
            if let Some(slot) = object_prop(&self.win, "active-slot") {
                if gfile_prop(&slot, "location").1 {
                    return Some((slot, "window.active-slot".into()));
                }
            }
        }
        for w in walk(&self.win) {
            if w.find_property("window-slot").is_some() {
                if let Some(slot) = object_prop(&w, "window-slot") {
                    if gfile_prop(&slot, "location").1 {
                        return Some((slot, format!("{}.window-slot", type_name(&w))));
                    }
                }
            }
            if type_name(&w).contains("Slot") && w.is_mapped()
                && gfile_prop(w.upcast_ref(), "location").1
            {
                return Some((w.clone().upcast(), type_name(&w)));
            }
        }
        None
    }

    fn rebind(self: &Rc<Self>, notify: bool) {
        let found = self.find_slot();
        let changed = {
            let mut inner = self.inner.borrow_mut();
            let same = match (&found, &inner.slot) {
                (Some((a, _)), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };
            if !same {
                if let (Some(old), Some(h)) = (inner.slot.take(), inner.handler.take()) {
                    old.disconnect(h);
                }
                if let Some((slot, how)) = &found {
                    let weak = Rc::downgrade(self);
                    inner.handler = Some(slot.connect_notify_local(Some("location"), move |_, _| {
                        if let Some(s) = weak.upgrade() {
                            guard("location", (), || (s.on_change)());
                        }
                    }));
                    inner.slot = Some(slot.clone());
                    log!("location tracked through {how}");
                }
            }
            if found.is_none() && inner.slot.is_none() && !inner.logged {
                inner.logged = true;
                log!("Nautilus slot not found; window properties: {:?}", prop_names(self.win.upcast_ref()));
            }
            !same
        };
        if changed && notify {
            (self.on_change)();
        }
    }

    pub fn slot(self: &Rc<Self>) -> Option<glib::Object> {
        let _ = self.location();
        self.inner.borrow().slot.clone()
    }

    pub fn location(self: &Rc<Self>) -> (Option<PathBuf>, bool) {
        let stale = {
            let inner = self.inner.borrow();
            match &inner.slot {
                None => true,
                Some(s) => s.downcast_ref::<gtk::Widget>()
                    .is_some_and(|w| w.root().map(|r| r.upcast::<gtk::Widget>())
                                 != Some(self.win.clone().upcast())),
            }
        };
        if stale {
            self.rebind(false);
        }
        let inner = self.inner.borrow();
        let Some(slot) = &inner.slot else { return (None, false) };
        match gfile_prop(slot, "location") {
            (f, true) => (f.and_then(|f| f.path()), true),
            _ => (None, false),
        }
    }
}

