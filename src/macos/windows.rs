use super::*;

pub(super) struct WindowList {
    pub(super) app: OwnedCf,
    pub(super) windows: OwnedCf,
}

#[derive(Clone)]
pub(super) struct CandidateRow {
    pub(super) owner: usize,
    pub(super) window: usize,
    pub(super) pid: i32,
    pub(super) app_name: String,
    pub(super) title: String,
}

pub(super) struct CandidateSnapshot {
    pub(super) owners: Vec<WindowList>,
    pub(super) rows: Vec<CandidateRow>,
}

fn string_value(value: CFTypeRef) -> Option<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    let mut buffer = [0i8; 2048];
    if unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr(),
            buffer.len() as CFIndex,
            K_CFSTRING_ENCODING_UTF8,
        )
    } == 0
    {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn dictionary_value(dictionary: CFTypeRef, key: CFTypeRef) -> Option<CFTypeRef> {
    if dictionary.is_null()
        || unsafe { CFGetTypeID(dictionary) } != unsafe { CFDictionaryGetTypeID() }
    {
        return None;
    }
    let value = unsafe { CFDictionaryGetValue(dictionary, key) };
    (!value.is_null()).then_some(value)
}

fn number_i32(value: CFTypeRef) -> Option<i32> {
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }
    let mut result = 0i32;
    let ok = unsafe {
        CFNumberGetValue(
            value,
            K_CFNUMBER_SINT32_TYPE,
            (&mut result as *mut i32).cast(),
        )
    };
    (ok != 0).then_some(result)
}

type OwnerRecord = (i32, i32, Option<String>);

fn cg_owner_records(
    options: u32,
    keys: (CFStringRef, CFStringRef, CFStringRef),
) -> Result<(Vec<OwnerRecord>, usize), String> {
    let info = unsafe { CGWindowListCopyWindowInfo(options, 0) };
    if info.is_null() {
        return Err("CGWindowListCopyWindowInfo returned null".into());
    }
    let info = OwnedCf(info);
    if unsafe { CFGetTypeID(info.0) } != unsafe { CFArrayGetTypeID() } {
        return Err("CGWindowListCopyWindowInfo did not return an array".into());
    }
    let count = usize::try_from(unsafe { CFArrayGetCount(info.0) })
        .map_err(|_| "CGWindowListCopyWindowInfo returned a negative count")?;
    let mut records = Vec::with_capacity(count);
    let mut bad = 0;
    for index in 0..count {
        let row = unsafe { CFArrayGetValueAtIndex(info.0, index as CFIndex) };
        let pid = dictionary_value(row, keys.0).and_then(number_i32);
        let layer = dictionary_value(row, keys.1).and_then(number_i32);
        let (Some(pid), Some(layer)) = (pid, layer) else {
            bad += 1;
            continue;
        };
        let name = dictionary_value(row, keys.2).and_then(string_value);
        records.push((pid, layer, name));
    }
    Ok((records, bad))
}

pub(super) fn candidate_snapshot() -> Result<CandidateSnapshot, String> {
    let exclude = K_CG_WINDOW_LIST_OPTION_EXCLUDE_DESKTOP_ELEMENTS;
    let keys = unsafe { (kCGWindowOwnerPID, kCGWindowLayer, kCGWindowOwnerName) };
    let (visible, bad_visible) =
        cg_owner_records(K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | exclude, keys)?;
    let (all, bad_all) = cg_owner_records(exclude, keys)?;
    let mut raw: Vec<(i32, i32)> = visible
        .iter()
        .map(|(pid, layer, _)| (*pid, *layer))
        .collect();
    raw.extend(all.iter().map(|(pid, layer, _)| (*pid, *layer)));
    let mut names = Vec::<(i32, String)>::new();
    for (pid, _, name) in visible.iter().chain(all.iter()) {
        if let Some(name) = name {
            if !names.iter().any(|entry| entry.0 == *pid) {
                names.push((*pid, name.clone()));
            }
        }
    }
    let self_pid = std::process::id() as i32;
    let pids = crate::input::candidate_owner_pids(&raw, self_pid);
    let invalid = raw.iter().filter(|entry| entry.0 <= 0).count();
    let self_windows = raw.iter().filter(|entry| entry.0 == self_pid).count();
    let layered = raw
        .iter()
        .filter(|entry| entry.0 > 0 && entry.0 != self_pid && entry.1 != 0)
        .count();
    let mut seen = Vec::new();
    let duplicates = raw
        .iter()
        .filter(|entry| {
            let (pid, layer) = **entry;
            pid > 0 && pid != self_pid && layer == 0 && {
                let duplicate = seen.contains(&pid);
                if !duplicate {
                    seen.push(pid);
                }
                duplicate
            }
        })
        .count();
    if bad_visible + bad_all > 0 {
        eprintln!(
            "Skipped {} CG window rows with invalid dictionary/PID/layer metadata.",
            bad_visible + bad_all
        );
    }
    println!("CG owner rows: visible {}, all {}, invalid metadata {}; excluded invalid PID: {invalid}, self: {self_windows}, nonzero layer: {layered}, duplicate owners: {duplicates}; unique candidate PIDs: {}.", visible.len(), all.len(), bad_visible + bad_all, pids.len());
    println!("Visible owner order groups apps front-to-back; the supplemental all-windows order is unspecified. Neither order is per-window AX z-order or MRU. CGWindowList can omit apps and Spaces; candidate completeness across apps/Spaces is unverified.");

    let mut owners = Vec::new();
    let mut rows = Vec::new();
    for pid in pids {
        let app_name = names
            .iter()
            .find(|(known, _)| *known == pid)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("PID {pid}"));
        let list = match window_list(pid) {
            Ok(list) => list,
            Err(error) => {
                eprintln!(
                    "Skipped candidate {app_name:?} (PID {pid}): AX enumeration failed: {error}"
                );
                continue;
            }
        };
        let owner = owners.len();
        let count = match usize::try_from(unsafe { CFArrayGetCount(list.windows.0) }) {
            Ok(count) => count,
            Err(_) => {
                eprintln!("Skipped candidate {app_name:?} (PID {pid}): AXWindows returned a negative count.");
                continue;
            }
        };
        let (
            mut invalid_element,
            mut role_skip,
            mut subrole_skip,
            mut modal_skip,
            mut modal_unreadable,
            mut read_skip,
        ) = (0, 0, 0, 0, 0, 0);
        for window_index in 0..count {
            let window = match window_at(&list, window_index) {
                Ok(window) => window,
                Err(_) => {
                    invalid_element += 1;
                    continue;
                }
            };
            let timeout = unsafe { AXUIElementSetMessagingTimeout(window, 1.0) };
            if timeout != K_AX_ERROR_SUCCESS {
                read_skip += 1;
                continue;
            }
            let role = match read_string_attribute(window, c"AXRole") {
                Ok(role) => role,
                Err(_) => {
                    read_skip += 1;
                    continue;
                }
            };
            if role != "AXWindow" {
                role_skip += 1;
                continue;
            }
            let subrole = match read_string_attribute(window, c"AXSubrole") {
                Ok(subrole) => subrole,
                Err(_) => {
                    read_skip += 1;
                    continue;
                }
            };
            if subrole != "AXStandardWindow" {
                subrole_skip += 1;
                continue;
            }
            match read_boolean_attribute(window, c"AXModal") {
                Ok(false) => {}
                Ok(true) => {
                    modal_skip += 1;
                    continue;
                }
                Err(_) => {
                    modal_unreadable += 1;
                    continue;
                }
            }
            rows.push(CandidateRow {
                owner,
                window: window_index,
                pid,
                app_name: app_name.clone(),
                title: title(window),
            });
        }
        println!("{app_name:?} PID {pid}: {} standard windows; skipped invalid elements {invalid_element}, role {role_skip}, subrole {subrole_skip}, modal {modal_skip}, unreadable/unsupported AXModal {modal_unreadable}, other unreadable {read_skip}.", rows.iter().filter(|row| row.owner == owner).count());
        owners.push(list);
    }
    println!("Accepted AX candidates: {}", rows.len());
    Ok(CandidateSnapshot { owners, rows })
}

pub(super) fn window_list(pid: i32) -> Result<WindowList, String> {
    let element = unsafe { AXUIElementCreateApplication(pid) };
    if element.is_null() {
        return Err("AXUIElementCreateApplication returned null".into());
    }
    let app = OwnedCf(element);
    let timeout_result = unsafe { AXUIElementSetMessagingTimeout(app.0, 1.0) };
    if timeout_result != K_AX_ERROR_SUCCESS {
        return Err(format!(
            "Could not set AX timeout (AXError {timeout_result})"
        ));
    }

    let attribute = cf_string(c"AXWindows")?;
    let windows = copy_attribute(app.0, attribute.0, "AXWindows")?;
    if unsafe { CFGetTypeID(windows.0) } != unsafe { CFArrayGetTypeID() } {
        return Err("AXWindows did not return an array".into());
    }
    Ok(WindowList { app, windows })
}

pub fn windows_count(pid: i32) -> Result<usize, String> {
    let list = window_list(pid)?;
    usize::try_from(unsafe { CFArrayGetCount(list.windows.0) })
        .map_err(|_| "AXWindows returned a negative count".into())
}

pub(super) fn cf_string(value: &std::ffi::CStr) -> Result<OwnedCf, String> {
    let result =
        unsafe { CFStringCreateWithCString(ptr::null(), value.as_ptr(), K_CFSTRING_ENCODING_UTF8) };
    if result.is_null() {
        Err("Could not create Accessibility attribute name".into())
    } else {
        Ok(OwnedCf(result))
    }
}

pub(super) fn copy_attribute(
    element: CFTypeRef,
    attribute: CFTypeRef,
    name: &str,
) -> Result<OwnedCf, String> {
    let mut value = ptr::null();
    let result = unsafe { AXUIElementCopyAttributeValue(element, attribute, &mut value) };
    if result != K_AX_ERROR_SUCCESS {
        return Err(format!("Could not read {name} (AXError {result})"));
    }
    if value.is_null() {
        return Err(format!("{name} returned no value"));
    }
    Ok(OwnedCf(value))
}

fn read_string_attribute(element: CFTypeRef, name: &std::ffi::CStr) -> Result<String, String> {
    let attribute = cf_string(name)?;
    let value = copy_attribute(element, attribute.0, name.to_str().unwrap_or("attribute"))?;
    string_value(value.0).ok_or_else(|| format!("{name:?} was not a bounded UTF-8 string"))
}

fn read_boolean_attribute(element: CFTypeRef, name: &std::ffi::CStr) -> Result<bool, String> {
    let attribute = cf_string(name)?;
    let value = copy_attribute(element, attribute.0, name.to_str().unwrap_or("attribute"))?;
    if unsafe { CFGetTypeID(value.0) } != unsafe { CFBooleanGetTypeID() } {
        return Err(format!("{name:?} did not return a Boolean"));
    }
    Ok(unsafe { CFBooleanGetValue(value.0) } != 0)
}

pub(super) fn window_at(list: &WindowList, index: usize) -> Result<CFTypeRef, String> {
    let count = unsafe { CFArrayGetCount(list.windows.0) };
    if index >= usize::try_from(count).map_err(|_| "AXWindows returned a negative count")? {
        return Err(format!("Window index {index} is out of range"));
    }
    let window = unsafe { CFArrayGetValueAtIndex(list.windows.0, index as CFIndex) };
    if window.is_null() || unsafe { CFGetTypeID(window) } != unsafe { AXUIElementGetTypeID() } {
        return Err(format!("AXWindows item {index} is not an AXUIElement"));
    }
    Ok(window)
}

pub(super) fn title(window: CFTypeRef) -> String {
    let Ok(attribute) = cf_string(c"AXTitle") else {
        return String::new();
    };
    let Ok(value) = copy_attribute(window, attribute.0, "AXTitle") else {
        return String::new();
    };
    if unsafe { CFGetTypeID(value.0) } != unsafe { CFStringGetTypeID() } {
        return String::new();
    }
    let mut buffer = [0i8; 2048];
    if unsafe {
        CFStringGetCString(
            value.0,
            buffer.as_mut_ptr(),
            buffer.len() as CFIndex,
            K_CFSTRING_ENCODING_UTF8,
        )
    } == 0
    {
        return "<title unavailable or too long>".into();
    }
    unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

pub fn list_windows(pid: i32) -> Result<(), String> {
    let list = window_list(pid)?;
    let count = unsafe { CFArrayGetCount(list.windows.0) };
    let count = usize::try_from(count).map_err(|_| "AXWindows returned a negative count")?;
    println!("AX windows for PID {pid} (ordinal indexes are diagnostic and may change):");
    for index in 0..count {
        let window = window_at(&list, index)?;
        let timeout = unsafe { AXUIElementSetMessagingTimeout(window, 1.0) };
        if timeout != K_AX_ERROR_SUCCESS {
            return Err(format!(
                "Could not set AX timeout for window {index} (AXError {timeout})"
            ));
        }
        println!("{index}: {:?}", title(window));
    }
    Ok(())
}

unsafe fn is_settable(element: CFTypeRef, name: &std::ffi::CStr) -> Result<bool, String> {
    let attribute = cf_string(name)?;
    let mut settable = 0;
    let result = AXUIElementIsAttributeSettable(element, attribute.0, &mut settable);
    if result == K_AX_ERROR_SUCCESS {
        Ok(settable != 0)
    } else {
        Err(format!("Could not inspect {name:?} (AXError {result})"))
    }
}

unsafe fn set_true_if_supported(element: CFTypeRef, name: &std::ffi::CStr) -> Result<bool, String> {
    if !is_settable(element, name)? {
        return Ok(false);
    }
    let attribute = cf_string(name)?;
    let result = AXUIElementSetAttributeValue(element, attribute.0, kCFBooleanTrue);
    if result == K_AX_ERROR_SUCCESS {
        Ok(true)
    } else {
        Err(format!("Could not set {name:?} (AXError {result})"))
    }
}

pub fn raise_window(pid: i32, index: usize) -> Result<(), String> {
    let list = window_list(pid)?;
    raise_from_list(&list, index)
}

pub(super) fn raise_from_list(list: &WindowList, index: usize) -> Result<(), String> {
    let window = window_at(&list, index)?;
    let timeout = unsafe { AXUIElementSetMessagingTimeout(window, 1.0) };
    if timeout != K_AX_ERROR_SUCCESS {
        return Err(format!(
            "Could not set AX timeout for window {index} (AXError {timeout})"
        ));
    }
    let restored = unsafe { set_false_if_true(window, c"AXMinimized")? };
    let app_hidden = unsafe { set_false_if_true(list.app.0, c"AXHidden")? };
    let frontmost_accepted = unsafe { set_true_if_supported(list.app.0, c"AXFrontmost")? };
    if !frontmost_accepted {
        return Err("AXFrontmost is not settable for this app; app activation requires further investigation".into());
    }
    let action = cf_string(c"AXRaise")?;
    let action_result = unsafe { AXUIElementPerformAction(window, action.0) };
    if action_result != K_AX_ERROR_SUCCESS {
        return Err(format!("AXRaise failed (AXError {action_result})"));
    }
    let _ = unsafe { set_true_if_supported(window, c"AXMain")? };
    let _ = unsafe { set_true_if_supported(window, c"AXFocused")? };
    let focused_attr = cf_string(c"AXFocusedWindow")?;
    let focused = copy_attribute(list.app.0, focused_attr.0, "AXFocusedWindow");
    let focus_verified = focused.is_ok_and(|value| unsafe { CFEqual(value.0, window) != 0 });
    println!("AXRaise accepted: yes; app frontmost accepted: {}; app hidden restored: {}; minimized restored: {}; focused-window verified: {}. This does not prove key input or Space transition.", frontmost_accepted, app_hidden, restored, focus_verified);
    Ok(())
}

pub(super) fn focused_index(list: &WindowList, count: usize) -> Option<usize> {
    let attribute = cf_string(c"AXFocusedWindow").ok()?;
    let focused = copy_attribute(list.app.0, attribute.0, "AXFocusedWindow").ok()?;
    (0..count).find(|index| {
        window_at(list, *index).is_ok_and(|window| unsafe { CFEqual(focused.0, window) != 0 })
    })
}

unsafe fn set_false_if_true(element: CFTypeRef, name: &std::ffi::CStr) -> Result<bool, String> {
    let attribute = cf_string(name)?;
    let value = copy_attribute(element, attribute.0, name.to_str().unwrap_or("attribute"))?;
    if CFGetTypeID(value.0) != CFBooleanGetTypeID() {
        return Err(format!("{name:?} did not return a Boolean"));
    }
    if CFBooleanGetValue(value.0) == 0 {
        return Ok(false);
    }
    let mut settable = 0;
    let result = AXUIElementIsAttributeSettable(element, attribute.0, &mut settable);
    if result != K_AX_ERROR_SUCCESS {
        return Err(format!("Could not inspect {name:?} (AXError {result})"));
    }
    if settable == 0 {
        return Err(format!("{name:?} is true but not settable"));
    }
    let result = AXUIElementSetAttributeValue(element, attribute.0, kCFBooleanFalse);
    if result != K_AX_ERROR_SUCCESS {
        return Err(format!("Could not clear {name:?} (AXError {result})"));
    }
    Ok(true)
}
