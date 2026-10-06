#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
mod macos {
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

    const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_AX_ERROR_SUCCESS: AXError = 0;
    const K_CG_SESSION_EVENT_TAP: u32 = 1;
    const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
    const K_CG_EVENT_KEY_DOWN: CGEventType = 10;
    const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = u32::MAX - 1;
    const K_CG_EVENT_TAP_DISABLED_BY_USER: CGEventType = u32::MAX;
    const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 1 << 20;
    const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> AXError;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
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
        fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        fn CGEventGetFlags(event: CGEventRef) -> u64;
        fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            text: *const i8,
            encoding: u32,
        ) -> CFStringRef;
        fn CFGetTypeID(value: CFTypeRef) -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
        fn CFRelease(value: CFTypeRef);
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: CFIndex,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(
            loop_ref: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        fn CFRunLoopRemoveSource(
            loop_ref: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
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
        unsafe { AXIsProcessTrusted() }
    }

    pub fn windows_count(pid: i32) -> Result<usize, String> {
        let element = unsafe { AXUIElementCreateApplication(pid) };
        if element.is_null() {
            return Err("AXUIElementCreateApplication returned null".into());
        }
        let element = OwnedCf(element);
        let timeout_result = unsafe { AXUIElementSetMessagingTimeout(element.0, 1.0) };
        if timeout_result != K_AX_ERROR_SUCCESS {
            return Err(format!("Could not set AX timeout (AXError {timeout_result})"));
        }

        let attribute = unsafe {
            CFStringCreateWithCString(ptr::null(), c"AXWindows".as_ptr(), K_CFSTRING_ENCODING_UTF8)
        };
        if attribute.is_null() {
            return Err("Could not create AXWindows attribute name".into());
        }
        let attribute = OwnedCf(attribute);
        let mut value = ptr::null();
        let result = unsafe { AXUIElementCopyAttributeValue(element.0, attribute.0, &mut value) };
        if result != K_AX_ERROR_SUCCESS {
            return Err(format!("Could not read AXWindows (AXError {result})"));
        }
        if value.is_null() {
            return Err("AXWindows returned no value".into());
        }
        let value = OwnedCf(value);
        if unsafe { CFGetTypeID(value.0) } != unsafe { CFArrayGetTypeID() } {
            return Err("AXWindows did not return an array".into());
        }
        let count = unsafe { CFArrayGetCount(value.0) };
        usize::try_from(count).map_err(|_| "AXWindows returned a negative count".into())
    }

    #[derive(Default)]
    struct TapStats {
        command_tab: u32,
        disabled: bool,
    }

    extern "C" fn observe_event(
        _proxy: CGEventTapProxy,
        kind: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef {
        if user_info.is_null() {
            return event;
        }
        let stats = unsafe { &mut *user_info.cast::<TapStats>() };
        if kind == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT || kind == K_CG_EVENT_TAP_DISABLED_BY_USER {
            stats.disabled = true;
        } else if kind == K_CG_EVENT_KEY_DOWN && !event.is_null() {
            let flags = unsafe { CGEventGetFlags(event) };
            let keycode =
                unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
            if flags & K_CG_EVENT_FLAG_MASK_COMMAND != 0 && keycode == 48 {
                stats.command_tab = stats.command_tab.saturating_add(1);
            }
        }
        // No event is modified or swallowed; only a count is kept in memory.
        event
    }

    pub fn listen(seconds: f64) -> Result<(), String> {
        let mask: CGEventMask = 1_u64 << K_CG_EVENT_KEY_DOWN;
        let mut stats = TapStats::default();
        let tap = unsafe {
            CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                mask,
                observe_event,
                (&mut stats as *mut TapStats).cast(),
            )
        };
        if tap.is_null() {
            return Err(
                "Could not create listen-only event tap. Check Input Monitoring permission.".into(),
            );
        }
        let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
        if source.is_null() {
            unsafe { CFRelease(tap) };
            return Err("Could not create event tap run-loop source".into());
        }
        let started = std::time::Instant::now();
        let deadline = started + std::time::Duration::from_secs_f64(seconds);
        unsafe {
            let current = CFRunLoopGetCurrent();
            let mode = kCFRunLoopDefaultMode;
            CFRunLoopAddSource(current, source, mode);
            CGEventTapEnable(tap, true);
            while !stats.disabled {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                CFRunLoopRunInMode(mode, remaining.as_secs_f64(), false);
            }
            CGEventTapEnable(tap, false);
            CFRunLoopRemoveSource(current, source, mode);
            CFMachPortInvalidate(tap);
            CFRelease(source);
            CFRelease(tap);
        }
        println!("Command+Tab key-downs observed: {}", stats.command_tab);
        if stats.disabled {
            return Err("The event tap was disabled during the diagnostic".into());
        }
        Ok(())
    }
}

fn parse_seconds(raw: &str) -> Result<f64, String> {
    let seconds: f64 = raw
        .parse()
        .map_err(|_| "duration must be a number between 0 and 10 seconds")?;
    if seconds.is_finite() && (0.0..=10.0).contains(&seconds) && seconds > 0.0 {
        Ok(seconds)
    } else {
        Err("duration must be greater than 0 and at most 10 seconds".into())
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        return Err("wintab-rs currently supports macOS only".into());
    }
    #[cfg(target_os = "macos")]
    {
        match args.as_slice() {
            [] => {
                println!("wintab-rs phase 1 diagnostic PoC");
                if macos::trusted() {
                    println!("Accessibility: granted");
                } else {
                    println!(
                        "Accessibility: not granted. Enable wintab-rs in System Settings > Privacy & Security > Accessibility, then relaunch it."
                    );
                }
                println!(
                    "Input Monitoring: if --tap-seconds cannot create an event tap, enable wintab-rs in System Settings > Privacy & Security > Input Monitoring, then relaunch it."
                );
                println!(
                    "Use --pid PID to count an application's AX windows, or --tap-seconds N for a short listen-only input-monitoring check."
                );
                Ok(())
            }
            [flag, pid] if flag == "--pid" => {
                let pid: i32 = pid
                    .parse()
                    .map_err(|_| "PID must be a positive integer".to_string())?;
                if pid <= 0 {
                    return Err("PID must be a positive integer".into());
                }
                if !macos::trusted() {
                    return Err(
                        "Accessibility is not granted. Enable wintab-rs in System Settings > Privacy & Security > Accessibility, then relaunch it."
                            .into(),
                    );
                }
                let count = macos::windows_count(pid)?;
                println!("AX windows for PID {pid}: {count}");
                Ok(())
            }
            [flag, duration] if flag == "--tap-seconds" => {
                let seconds = parse_seconds(duration)?;
                println!(
                    "Listening for key-down events for {seconds:.1}s; all events pass through unchanged."
                );
                macos::listen(seconds)
            }
            _ => Err("Usage: wintab-rs [--pid PID | --tap-seconds SECONDS]".into()),
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("wintab-rs: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_seconds;

    #[test]
    fn duration_is_bounded_to_ten_seconds() {
        assert_eq!(parse_seconds("0.5").unwrap(), 0.5);
        assert!(parse_seconds("0").is_err());
        assert!(parse_seconds("10.1").is_err());
        assert!(parse_seconds("NaN").is_err());
    }
}
