use super::*;
use std::ptr;

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
        let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
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
    let output = state.handle(super::input::decode_event(kind, event));
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
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
