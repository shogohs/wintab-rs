use std::{
    ffi::{c_char, c_void},
    os::raw::c_int,
};

pub(super) type MenuAction = extern "C" fn(c_int, *mut c_void) -> c_int;
pub(super) type DeferredAction = extern "C" fn(*mut c_void, c_int);

unsafe extern "C" {
    fn wintab_overlay_show(labels: *const *const c_char, count: usize, selected: isize) -> i32;
    fn wintab_overlay_select(selected: isize);
    fn wintab_overlay_hide();
    fn wintab_status_run(action: MenuAction, context: *mut c_void, enabled: c_int) -> i32;
    fn wintab_status_prepare();
    fn wintab_open_privacy_settings();
    fn wintab_defer_switch(action: DeferredAction, context: *mut c_void, reverse: c_int);
}

pub fn show(labels: &[*const c_char], selected: Option<usize>) -> bool {
    unsafe {
        wintab_overlay_show(
            labels.as_ptr(),
            labels.len(),
            selected.map_or(-1, |index| index as isize),
        ) != 0
    }
}

pub fn select(selected: Option<usize>) {
    unsafe { wintab_overlay_select(selected.map_or(-1, |index| index as isize)) }
}

pub fn hide() {
    unsafe { wintab_overlay_hide() }
}

pub(super) fn status_run(action: MenuAction, context: *mut c_void, enabled: bool) -> bool {
    unsafe { wintab_status_run(action, context, enabled as c_int) != 0 }
}

pub(super) fn status_prepare() {
    unsafe { wintab_status_prepare() }
}

pub(super) fn open_privacy_settings() {
    unsafe { wintab_open_privacy_settings() }
}

pub(super) fn defer_switch(action: DeferredAction, context: *mut c_void, reverse: bool) {
    unsafe { wintab_defer_switch(action, context, reverse as c_int) }
}
