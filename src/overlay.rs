use std::ffi::c_char;

unsafe extern "C" {
    fn wintab_overlay_show(labels: *const *const c_char, count: usize, selected: isize) -> i32;
    fn wintab_overlay_select(selected: isize);
    fn wintab_overlay_hide();
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
