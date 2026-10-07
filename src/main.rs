#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod input;

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
    type CFDictionaryRef = *const c_void;

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
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
        fn CGPreflightListenEventAccess() -> bool;
        fn CGRequestListenEventAccess() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
        fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> AXError;
        fn AXUIElementCopyAttributeValue(
            element: CFTypeRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementGetTypeID() -> usize;
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
        fn CFStringGetTypeID() -> usize;
        fn CFStringGetCString(
            value: CFStringRef,
            buffer: *mut i8,
            size: CFIndex,
            encoding: u32,
        ) -> u8;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(value: CFTypeRef) -> u8;
        fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
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

        let path =
            std::env::current_exe().map_err(|e| format!("Could not determine app path: {e}"))?;
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

    struct WindowList {
        app: OwnedCf,
        windows: OwnedCf,
    }

    fn window_list(pid: i32) -> Result<WindowList, String> {
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

    fn cf_string(value: &std::ffi::CStr) -> Result<OwnedCf, String> {
        let result = unsafe {
            CFStringCreateWithCString(ptr::null(), value.as_ptr(), K_CFSTRING_ENCODING_UTF8)
        };
        if result.is_null() {
            Err("Could not create Accessibility attribute name".into())
        } else {
            Ok(OwnedCf(result))
        }
    }

    fn copy_attribute(
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

    fn window_at(list: &WindowList, index: usize) -> Result<CFTypeRef, String> {
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

    fn title(window: CFTypeRef) -> String {
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

    unsafe fn set_true_if_supported(
        element: CFTypeRef,
        name: &std::ffi::CStr,
    ) -> Result<bool, String> {
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
                "Could not create listen-only event tap. Check Input Monitoring permission and other event-tap requirements.".into(),
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

    extern "C" fn capture_event(
        _proxy: CGEventTapProxy,
        kind: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef {
        if user_info.is_null() {
            return event;
        }
        let state = unsafe { &mut *user_info.cast::<crate::input::CaptureState>() };
        let input = if kind == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT
            || kind == K_CG_EVENT_TAP_DISABLED_BY_USER
        {
            crate::input::Event::Disabled
        } else if event.is_null() {
            crate::input::Event::Other
        } else if kind == K_CG_EVENT_KEY_DOWN {
            let key =
                unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
            let flags = unsafe { CGEventGetFlags(event) };
            let repeat =
                unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_AUTOREPEAT) } != 0;
            crate::input::Event::KeyDown { key, flags, repeat }
        } else if kind == K_CG_EVENT_KEY_UP {
            let key =
                unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
            crate::input::Event::KeyUp { key }
        } else if kind == K_CG_EVENT_FLAGS_CHANGED {
            crate::input::Event::FlagsChanged {
                flags: unsafe { CGEventGetFlags(event) },
            }
        } else {
            crate::input::Event::Other
        };
        let output = state.handle(input);
        if output.suppress {
            ptr::null_mut()
        } else {
            event
        }
    }

    pub fn capture(seconds: f64) -> Result<(), String> {
        let mask = (1_u64 << K_CG_EVENT_KEY_DOWN)
            | (1_u64 << K_CG_EVENT_KEY_UP)
            | (1_u64 << K_CG_EVENT_FLAGS_CHANGED);
        let mut state = crate::input::CaptureState::default();
        let tap = unsafe {
            CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                0,
                mask,
                capture_event,
                (&mut state as *mut crate::input::CaptureState).cast(),
            )
        };
        if tap.is_null() {
            return Err("Could not create active event tap; check Input Monitoring permission and event-tap requirements.".into());
        }
        let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
        if source.is_null() {
            unsafe { CFRelease(tap) };
            return Err("Could not create active event tap run-loop source".into());
        }
        let started = std::time::Instant::now();
        let deadline = started + std::time::Duration::from_secs_f64(seconds);
        unsafe {
            let current = CFRunLoopGetCurrent();
            let mode = kCFRunLoopDefaultMode;
            CFRunLoopAddSource(current, source, mode);
            CGEventTapEnable(tap, true);
            while state.end != Some(crate::input::End::Disabled) {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                CFRunLoopRunInMode(mode, remaining.as_secs_f64().min(0.1), false);
            }
            CGEventTapEnable(tap, false);
            CFRunLoopRemoveSource(current, source, mode);
            CFMachPortInvalidate(tap);
            CFRelease(source);
            CFRelease(tap);
        }
        println!("Capture ended: forward steps {}, reverse steps {}, committed sessions {}, cancelled sessions {}; no window was switched.", state.forward, state.reverse, state.committed, state.cancelled);
        if state.end == Some(crate::input::End::Disabled) {
            return Err("The active event tap was disabled; capture state was reset and the tap was not re-enabled.".into());
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
        enum Command {
            Status,
            Pid(i32),
            Tap(f64),
            Capture(f64),
            ListWindows(i32),
            RaiseWindow(i32, usize),
        }
        let command = match args.as_slice() {
            [] => Command::Status,
            [flag, pid] if flag == "--pid" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 {
                    return Err("PID must be a positive integer".into());
                }
                Command::Pid(pid)
            }
            [flag, duration] if flag == "--tap-seconds" => Command::Tap(parse_seconds(duration)?),
            [flag, duration] if flag == "--capture-seconds" => Command::Capture(parse_seconds(duration)?),
            [flag, pid] if flag == "--list-windows" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 { return Err("PID must be a positive integer".into()); }
                Command::ListWindows(pid)
            }
            [flag, pid, index] if flag == "--raise-window" => {
                let pid: i32 = pid.parse().map_err(|_| "PID must be a positive integer")?;
                if pid <= 0 { return Err("PID must be a positive integer".into()); }
                let index: usize = index.parse().map_err(|_| "window index must be a non-negative integer")?;
                Command::RaiseWindow(pid, index)
            }
            _ => return Err("Usage: wintab-rs [--pid PID | --list-windows PID | --raise-window PID INDEX | --tap-seconds SECONDS | --capture-seconds SECONDS]".into()),
        };
        let (ax, input_monitoring) = macos::startup_permissions()?;
        match command {
            Command::Status => {
                println!("wintab-rs phase 1 diagnostic PoC");
                println!(
                    "Use --list-windows PID to list AX windows, --raise-window PID INDEX to test public AX raise/focus APIs, --tap-seconds N for listen-only input, or --capture-seconds N for bounded Command+Tab suppression diagnostics (no window switching)."
                );
                Ok(())
            }
            Command::Pid(pid) => {
                if !ax {
                    return Ok(());
                }
                let count = macos::windows_count(pid)?;
                println!("AX windows for PID {pid}: {count}");
                Ok(())
            }
            Command::Tap(seconds) => {
                if !input_monitoring {
                    return Ok(());
                }
                println!(
                    "Listening for key-down events for {seconds:.1}s; all events pass through unchanged."
                );
                macos::listen(seconds)
            }
            Command::Capture(seconds) => {
                if !ax || !input_monitoring {
                    return Ok(());
                }
                eprintln!("Capture suppresses Command+Tab for at most {seconds:.1}s and never switches a window. Escape cancels the current session; capture stops at the deadline.");
                macos::capture(seconds)
            }
            Command::ListWindows(pid) => {
                if !ax {
                    return Ok(());
                }
                macos::list_windows(pid)
            }
            Command::RaiseWindow(pid, index) => {
                if !ax {
                    return Ok(());
                }
                macos::raise_window(pid, index)
            }
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
