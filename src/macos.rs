#![allow(non_camel_case_types)]

use std::{ffi::c_void, ptr};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFIndex = isize;
type AXError = i32;
type CGEventMask = u64;
type CGEventType = u32;
type CGEventTapProxy = *mut c_void;
type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFDictionaryRef = *const c_void;
type CFArrayRef = *const c_void;

const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_AX_ERROR_SUCCESS: AXError = 0;
const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const K_CG_EVENT_KEY_DOWN: CGEventType = 10;
const K_CG_EVENT_KEY_UP: CGEventType = 11;
const K_CG_EVENT_FLAGS_CHANGED: CGEventType = 12;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = u32::MAX - 1;
const K_CG_EVENT_TAP_DISABLED_BY_USER: CGEventType = u32::MAX;
const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 1 << 20;
const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
const K_CG_KEYBOARD_EVENT_AUTOREPEAT: u32 = 8;
const K_CG_WINDOW_LIST_OPTION_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
const K_CFNUMBER_SINT32_TYPE: CFIndex = 3;
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCreateSystemWide() -> CFTypeRef;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementGetPid(element: CFTypeRef, pid: *mut i32) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: CFTypeRef,
        attribute: CFStringRef,
        settable: *mut u8,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> AXError;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: CGEventMask,
        callback: extern "C" fn(
            CGEventTapProxy,
            CGEventType,
            CGEventRef,
            *mut c_void,
        ) -> CGEventRef,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative_to_window: u32) -> CFArrayRef;
    static kCGWindowOwnerPID: CFStringRef;
    static kCGWindowLayer: CFStringRef;
    static kCGWindowOwnerName: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        text: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFGetTypeID(value: CFTypeRef) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: CFIndex) -> CFTypeRef;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFDictionaryGetValue(dictionary: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetCString(value: CFStringRef, buffer: *mut i8, size: CFIndex, encoding: u32) -> u8;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: CFTypeRef) -> u8;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(number: CFTypeRef, number_type: CFIndex, value: *mut c_void) -> u8;
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFRetain(value: CFTypeRef) -> CFTypeRef;
    fn CFRelease(value: CFTypeRef);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(loop_ref: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRemoveSource(loop_ref: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFMachPortInvalidate(port: CFMachPortRef);
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source: bool) -> i32;
    #[allow(non_upper_case_globals)]
    static kCFRunLoopDefaultMode: CFStringRef;
}

struct OwnedCf(CFTypeRef);

impl Drop for OwnedCf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

fn request_ax() -> Result<(), String> {
    let keys = [unsafe { kAXTrustedCheckOptionPrompt }];
    let values = [unsafe { kCFBooleanTrue }];
    let options = unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            ptr::null(),
            ptr::null(),
        )
    };
    if options.is_null() {
        return Err("Could not create Accessibility prompt options".into());
    }
    let options = OwnedCf(options);
    // The prompt is asynchronous; startup checks the current process status afterward.
    unsafe { AXIsProcessTrustedWithOptions(options.0) };
    Ok(())
}

pub fn startup_permissions() -> Result<(bool, bool), String> {
    let mut ax = trusted();
    let mut input = unsafe { CGPreflightListenEventAccess() };
    if ax && input {
        println!("Accessibility: granted");
        println!("Input Monitoring: granted");
        return Ok((ax, input));
    }

    let path = std::env::current_exe().map_err(|e| format!("Could not determine app path: {e}"))?;
    eprintln!(
        "Requesting missing permissions. Executable: {}",
        path.display()
    );
    eprintln!(
        "Enable wintab-rs in System Settings > Privacy & Security > Accessibility and/or Input Monitoring, then relaunch this app."
    );
    if !ax {
        request_ax()?;
    }
    if !input {
        unsafe { CGRequestListenEventAccess() };
    }
    ax = trusted();
    input = unsafe { CGPreflightListenEventAccess() };
    println!(
        "Accessibility: {}",
        if ax {
            "granted"
        } else {
            "not granted to this process; permission requested; grant in System Settings, then relaunch"
        }
    );
    println!(
        "Input Monitoring: {}",
        if input {
            "granted"
        } else {
            "not granted to this process; permission requested; grant in System Settings, then relaunch"
        }
    );
    if !ax || !input {
        eprintln!(
            "If already enabled, remove and re-add this app in System Settings (an updated ad-hoc app identity or launch path can be stale), then relaunch: {}",
            path.display()
        );
    }
    Ok((ax, input))
}

mod input;
mod windows;

pub use input::{capture, list_candidates, listen, resident, switch_global, switch_pid};
pub use windows::{list_windows, raise_window, windows_count};
