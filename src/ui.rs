use std::{
    ffi::{c_char, c_void},
    os::raw::c_int,
};

pub(super) type MenuAction = extern "C" fn(c_int, *mut c_void) -> c_int;
pub(super) type DeferredAction = extern "C" fn(*mut c_void, c_int);
pub(super) type MouseAction = extern "C" fn(isize, c_int, *mut c_void);

unsafe extern "C" {
    fn wintab_overlay_show(
        labels: *const *const c_char,
        pids: *const i32,
        count: usize,
        selected: isize,
        action: MouseAction,
        context: *mut c_void,
    ) -> i32;
    fn wintab_overlay_handle_click() -> i32;
    fn wintab_overlay_select(selected: isize);
    fn wintab_overlay_hide();
    fn wintab_status_run(
        action: MenuAction,
        context: *mut c_void,
        enabled: c_int,
        accessibility: c_int,
        input_monitoring: c_int,
    ) -> i32;
    fn wintab_status_prepare();
    fn wintab_open_privacy_settings();
    fn wintab_defer_switch(action: DeferredAction, context: *mut c_void, reverse: c_int);
}

pub fn show(
    labels: &[*const c_char],
    pids: &[i32],
    selected: Option<usize>,
    action: MouseAction,
    context: *mut c_void,
) -> bool {
    unsafe {
        wintab_overlay_show(
            labels.as_ptr(),
            pids.as_ptr(),
            labels.len(),
            selected.map_or(-1, |index| index as isize),
            action,
            context,
        ) != 0
    }
}

pub fn handle_click() -> i32 {
    unsafe { wintab_overlay_handle_click() }
}

pub fn select(selected: Option<usize>) {
    unsafe { wintab_overlay_select(selected.map_or(-1, |index| index as isize)) }
}

pub fn hide() {
    unsafe { wintab_overlay_hide() }
}

pub(super) fn status_run(
    action: MenuAction,
    context: *mut c_void,
    enabled: bool,
    accessibility: bool,
    input_monitoring: bool,
) -> bool {
    unsafe {
        wintab_status_run(
            action,
            context,
            enabled as c_int,
            accessibility as c_int,
            input_monitoring as c_int,
        ) != 0
    }
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
