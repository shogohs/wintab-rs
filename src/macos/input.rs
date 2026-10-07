use super::windows::{
    candidate_snapshot, cf_string, copy_attribute, focused_index, raise_from_list, title,
    window_at, window_list, CandidateRow, CandidateSnapshot,
};
use super::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

static RESIDENT_QUIT: AtomicBool = AtomicBool::new(false);
const MAX_MRU_WINDOWS: usize = 128;
thread_local! {
    static MRU_WINDOWS: RefCell<Vec<(i32, OwnedCf)>> = const { RefCell::new(Vec::new()) };
}

struct ResidentContext {
    tap: CFMachPortRef,
    source: CFRunLoopSourceRef,
    enabled: bool,
    busy: bool,
    permitted: bool,
}

#[no_mangle]
extern "C" fn wintab_resident_space_changed(user_info: *mut c_void) {
    if user_info.is_null() {
        return;
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
    if context.enabled && !context.busy && !context.tap.is_null() {
        unsafe { CGEventTapEnable(context.tap, true) };
    }
}

fn install_resident_tap(context: &mut ResidentContext) -> bool {
    if !context.tap.is_null() {
        unsafe { CGEventTapEnable(context.tap, true) };
        return true;
    }
    let tap = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            0,
            1_u64 << K_CG_EVENT_KEY_DOWN,
            resident_event,
            (context as *mut ResidentContext).cast(),
        )
    };
    if tap.is_null() {
        eprintln!("wintab-rs: event tap creation failed; check Input Monitoring permission");
        return false;
    }
    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
    if source.is_null() {
        unsafe { CFRelease(tap) };
        eprintln!("wintab-rs: could not create resident event tap run-loop source");
        return false;
    }
    context.tap = tap;
    context.source = source;
    unsafe {
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
    }
    true
}

extern "C" fn resident_event(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if user_info.is_null() {
        return event;
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
    if kind == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        if context.enabled && !context.busy {
            unsafe { CGEventTapEnable(context.tap, true) };
        }
        return event;
    }
    if kind == K_CG_EVENT_TAP_DISABLED_BY_USER {
        if !context.busy {
            context.enabled = false;
        }
        return event;
    }
    if kind != K_CG_EVENT_KEY_DOWN || event.is_null() {
        return event;
    }
    if !context.enabled || context.busy {
        return event;
    }
    let flags = unsafe { CGEventGetFlags(event) };
    let key = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
    if key != 48
        || flags & K_CG_EVENT_FLAG_MASK_COMMAND == 0
        || flags & ((1 << 18) | (1 << 19)) != 0
    {
        return event;
    }
    context.busy = true;
    unsafe { CGEventTapEnable(context.tap, false) };
    crate::overlay::defer_switch(begin_resident_switch, user_info, flags & (1 << 17) != 0);
    ptr::null_mut()
}

extern "C" fn resident_menu_action(action: i32, user_info: *mut c_void) -> i32 {
    if user_info.is_null() {
        return 0;
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
    match action {
        0 => {
            if context.enabled {
                context.enabled = false;
                if !context.tap.is_null() {
                    unsafe { CGEventTapEnable(context.tap, false) };
                }
            } else {
                context.permitted = startup_permissions().is_ok_and(|(ax, input)| ax && input);
                if context.permitted {
                    remember_focused_window();
                }
                if context.permitted && install_resident_tap(context) {
                    context.enabled = true;
                }
            }
            context.enabled as i32
        }
        1 => {
            crate::overlay::open_privacy_settings();
            context.enabled as i32
        }
        2 => {
            RESIDENT_QUIT.store(true, Ordering::Relaxed);
            if !context.tap.is_null() {
                unsafe { CGEventTapEnable(context.tap, false) };
            }
            0
        }
        _ => context.enabled as i32,
    }
}

extern "C" fn begin_resident_switch(user_info: *mut c_void, reverse: i32) {
    if user_info.is_null() {
        return;
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
    if context.enabled && context.permitted && !RESIDENT_QUIT.load(Ordering::Relaxed) {
        let result = candidate_snapshot().and_then(|mut snapshot| {
            let focused = focused_candidate(&snapshot);
            let focused = order_candidates(&mut snapshot, focused);
            run_switch_session(
                snapshot,
                focused,
                Some(10.0),
                Some(if reverse == 0 {
                    crate::input::Direction::Forward
                } else {
                    crate::input::Direction::Reverse
                }),
            )
        });
        if let Err(error) = result {
            eprintln!("wintab-rs: {error}");
        }
    }
    context.busy = false;
    unsafe {
        CGEventTapEnable(
            context.tap,
            context.enabled && !RESIDENT_QUIT.load(Ordering::Relaxed),
        )
    };
}

pub fn resident() -> Result<(), String> {
    RESIDENT_QUIT.store(false, Ordering::Relaxed);
    crate::overlay::status_prepare();
    let (ax, input_monitoring) = startup_permissions()?;
    if ax {
        remember_focused_window();
    }
    let mut context = ResidentContext {
        tap: ptr::null_mut(),
        source: ptr::null_mut(),
        enabled: ax && input_monitoring,
        busy: false,
        permitted: ax && input_monitoring,
    };
    if context.permitted && !install_resident_tap(&mut context) {
        context.enabled = false;
    }
    let ran = crate::overlay::status_run(
        resident_menu_action,
        (&mut context as *mut ResidentContext).cast(),
        context.enabled,
    );
    unsafe {
        if !context.source.is_null() {
            CGEventTapEnable(context.tap, false);
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), context.source, kCFRunLoopDefaultMode);
            CFMachPortInvalidate(context.tap);
            CFRelease(context.source);
            CFRelease(context.tap);
        }
    }
    if ran {
        Ok(())
    } else {
        Err("Could not create the wintab-rs menu bar item".into())
    }
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

fn decode_event(kind: CGEventType, event: CGEventRef) -> crate::input::Event {
    if kind == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT || kind == K_CG_EVENT_TAP_DISABLED_BY_USER {
        crate::input::Event::Disabled
    } else if event.is_null() {
        crate::input::Event::Other
    } else if kind == K_CG_EVENT_KEY_DOWN {
        let key = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
        let flags = unsafe { CGEventGetFlags(event) };
        let repeat =
            unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_AUTOREPEAT) } != 0;
        crate::input::Event::KeyDown { key, flags, repeat }
    } else if kind == K_CG_EVENT_KEY_UP {
        let key = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
        crate::input::Event::KeyUp { key }
    } else if kind == K_CG_EVENT_FLAGS_CHANGED {
        crate::input::Event::FlagsChanged {
            flags: unsafe { CGEventGetFlags(event) },
        }
    } else {
        crate::input::Event::Other
    }
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
    let output = state.handle(decode_event(kind, event));
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

struct SwitchContext {
    input: crate::input::CaptureState,
    selection: crate::input::Selection,
    deadline: Option<std::time::Instant>,
}

extern "C" fn overlay_mouse_action(index: isize, commit: i32, user_info: *mut c_void) {
    if user_info.is_null() || index < 0 {
        return;
    }
    let context = unsafe { &mut *user_info.cast::<SwitchContext>() };
    if context.selection.terminal() {
        return;
    }
    context.selection.select(index as usize);
    crate::overlay::select(context.selection.selected());
    if commit != 0 {
        context.selection.finish(crate::input::End::Commit);
    }
}

extern "C" fn switch_event(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if user_info.is_null() {
        return event;
    }
    let context = unsafe { &mut *user_info.cast::<SwitchContext>() };
    if kind == K_CG_EVENT_LEFT_MOUSE_DOWN {
        if !context.selection.terminal() {
            match crate::overlay::handle_click() {
                -1 => context.selection.cancel(),
                1 => return ptr::null_mut(),
                _ => {}
            }
        }
        return event;
    }
    let decoded = decode_event(kind, event);
    let output = if decoded == crate::input::Event::Disabled {
        let output = context.input.handle(decoded);
        context.selection.disable();
        output
    } else if context
        .deadline
        .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        && (!context.selection.terminal() || context.input.has_owned_keys())
    {
        context.selection.cancel();
        context.input.handle_owned_tail(decoded)
    } else if context.selection.terminal() {
        context.input.handle_owned_tail(decoded)
    } else {
        let output = context.input.handle(decoded);
        if let Some(direction) = output.step {
            context.selection.step(direction);
        }
        if let Some(end) = output.end {
            context.selection.finish(end);
        }
        output
    };
    if output.suppress {
        ptr::null_mut()
    } else {
        event
    }
}

fn focused_window() -> Option<(i32, OwnedCf)> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }
    let system = OwnedCf(system);
    if unsafe { AXUIElementSetMessagingTimeout(system.0, 1.0) } != K_AX_ERROR_SUCCESS {
        return None;
    }
    let app_attr = cf_string(c"AXFocusedApplication").ok()?;
    let app = copy_attribute(system.0, app_attr.0, "AXFocusedApplication").ok()?;
    if unsafe { CFGetTypeID(app.0) } != unsafe { AXUIElementGetTypeID() } {
        return None;
    }
    if unsafe { AXUIElementSetMessagingTimeout(app.0, 1.0) } != K_AX_ERROR_SUCCESS {
        return None;
    }
    let mut pid = 0;
    if unsafe { AXUIElementGetPid(app.0, &mut pid) } != K_AX_ERROR_SUCCESS || pid <= 0 {
        return None;
    }
    let window_attr = cf_string(c"AXFocusedWindow").ok()?;
    let focused = copy_attribute(app.0, window_attr.0, "AXFocusedWindow").ok()?;
    if unsafe { CFGetTypeID(focused.0) } != unsafe { AXUIElementGetTypeID() } {
        return None;
    }
    Some((pid, focused))
}

fn focused_candidate(snapshot: &CandidateSnapshot) -> Option<usize> {
    let (pid, focused) = focused_window()?;
    snapshot.rows.iter().position(|row| {
        row.pid == pid
            && window_at(&snapshot.owners[row.owner], row.window)
                .is_ok_and(|window| unsafe { CFEqual(focused.0, window) != 0 })
    })
}

fn remember_window(pid: i32, window: CFTypeRef) {
    let retained = OwnedCf(unsafe { CFRetain(window) });
    MRU_WINDOWS.with(|history| {
        let mut history = history.borrow_mut();
        history.retain(|(old_pid, old)| *old_pid != pid || unsafe { CFEqual(old.0, window) == 0 });
        history.insert(0, (pid, retained));
        history.truncate(MAX_MRU_WINDOWS);
    });
}

#[no_mangle]
extern "C" fn wintab_record_focus(pid: i32, window: CFTypeRef) {
    if pid > 0 && pid != std::process::id() as i32 && !window.is_null() {
        remember_window(pid, window);
    }
}

fn remember_candidate(snapshot: &CandidateSnapshot, index: usize) {
    if let Some(row) = snapshot.rows.get(index) {
        if let Ok(window) = window_at(&snapshot.owners[row.owner], row.window) {
            remember_window(row.pid, window);
        }
    }
}

fn remember_focused_window() {
    if let Some((pid, window)) = focused_window() {
        remember_window(pid, window.0);
    }
}

fn order_candidates(snapshot: &mut CandidateSnapshot, focused: Option<usize>) -> Option<usize> {
    if let Some(index) = focused {
        remember_candidate(snapshot, index);
    }
    let recent = MRU_WINDOWS.with(|history| {
        let history = history.borrow();
        // ponytail: compare the small AX candidate list linearly; index caching is only needed if this becomes slow.
        history
            .iter()
            .filter_map(|(pid, window)| {
                snapshot.rows.iter().position(|row| {
                    row.pid == *pid
                        && window_at(&snapshot.owners[row.owner], row.window)
                            .is_ok_and(|candidate| unsafe { CFEqual(candidate, window.0) != 0 })
                })
            })
            .collect::<Vec<_>>()
    });
    // Some apps expose their focused window to AXFocusedApplication but do not
    // match that element against AXWindows. Their latest activation is still
    // at the head of the observed MRU history, so use it as the cursor to skip.
    let has_history = !recent.is_empty();
    let order = crate::input::mru_order(snapshot.rows.len(), &recent);
    let old_rows = std::mem::take(&mut snapshot.rows);
    snapshot.rows = order.iter().map(|&index| old_rows[index].clone()).collect();
    MRU_WINDOWS.with(|history| {
        let mut next = Vec::with_capacity(snapshot.rows.len());
        for row in &snapshot.rows {
            if let Ok(window) = window_at(&snapshot.owners[row.owner], row.window) {
                next.push((row.pid, OwnedCf(unsafe { CFRetain(window) })));
            }
        }
        *history.borrow_mut() = next;
    });
    crate::input::mru_start_index(focused.is_some(), has_history)
}

pub fn list_candidates() -> Result<(), String> {
    let snapshot = candidate_snapshot()?;
    for (index, row) in snapshot.rows.iter().enumerate() {
        println!(
            "{index}: {:?} — {:?} (PID {})",
            row.app_name, row.title, row.pid
        );
    }
    Ok(())
}

pub fn switch_global(seconds: f64) -> Result<(), String> {
    let mut snapshot = candidate_snapshot()?;
    let focused = focused_candidate(&snapshot);
    let focused = order_candidates(&mut snapshot, focused);
    run_switch(snapshot, focused, seconds)
}

pub fn switch_pid(pid: i32, seconds: f64) -> Result<(), String> {
    let list = window_list(pid)?;
    let count = usize::try_from(unsafe { CFArrayGetCount(list.windows.0) })
        .map_err(|_| "AXWindows returned a negative count")?;
    if count == 0 {
        return Err(format!("PID {pid} has no AX windows to switch to"));
    }
    let focused = focused_index(&list, count);
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        let window = window_at(&list, index)?;
        let timeout = unsafe { AXUIElementSetMessagingTimeout(window, 1.0) };
        if timeout != K_AX_ERROR_SUCCESS {
            return Err(format!(
                "Could not set AX timeout for window {index} (AXError {timeout})"
            ));
        }
        rows.push(CandidateRow {
            owner: 0,
            window: index,
            pid,
            app_name: format!("PID {pid}"),
            title: title(window),
        });
    }
    let mut snapshot = CandidateSnapshot {
        owners: vec![list],
        rows,
    };
    let focused = order_candidates(&mut snapshot, focused);
    run_switch(snapshot, focused, seconds)
}

fn run_switch(
    snapshot: CandidateSnapshot,
    focused: Option<usize>,
    seconds: f64,
) -> Result<(), String> {
    run_switch_session(snapshot, focused, Some(seconds), None)
}

fn run_switch_session(
    snapshot: CandidateSnapshot,
    focused: Option<usize>,
    seconds: Option<f64>,
    initial: Option<crate::input::Direction>,
) -> Result<(), String> {
    let count = snapshot.rows.len();
    if count == 0 {
        return Err(
            "No supported AX standard windows were found; no event tap was installed".into(),
        );
    }
    println!("Frozen AX candidate list (indexes can change after AX operations):");
    for (index, row) in snapshot.rows.iter().enumerate() {
        println!(
            "{index}: {:?} — {:?} (PID {})",
            row.app_name, row.title, row.pid
        );
    }
    if let Some(index) = focused {
        println!("Initial focus is window {index}; first Tab selects the next window.");
    } else {
        println!("Initial focus was unavailable in this AX list; first forward Tab selects 0 and first reverse Tab selects the last window.");
    }
    let labels: Vec<_> = snapshot
        .rows
        .iter()
        .map(|row| {
            std::ffi::CString::new(row.title.replace('\0', "�"))
                .expect("replacing NUL yields a valid C string")
        })
        .collect();
    let label_ptrs: Vec<_> = labels.iter().map(|label| label.as_ptr()).collect();
    let pids: Vec<_> = snapshot.rows.iter().map(|row| row.pid).collect();
    let deadline = seconds
        .map(|seconds| std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds));
    let mut selection = crate::input::Selection::new(count, focused).unwrap();
    if let Some(direction) = initial {
        selection.step(direction);
    }
    let mut context = SwitchContext {
        input: if initial.is_some() {
            crate::input::CaptureState::begin_with_tab_down()
        } else {
            crate::input::CaptureState::default()
        },
        selection,
        deadline,
    };
    let mask = (1_u64 << K_CG_EVENT_KEY_DOWN)
        | (1_u64 << K_CG_EVENT_KEY_UP)
        | (1_u64 << K_CG_EVENT_FLAGS_CHANGED)
        | (1_u64 << K_CG_EVENT_LEFT_MOUSE_DOWN);
    let tap = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            0,
            mask,
            switch_event,
            (&mut context as *mut SwitchContext).cast(),
        )
    };
    if tap.is_null() {
        return Err("Could not create active event tap; check Input Monitoring permission and event-tap requirements.".into());
    }
    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
    if source.is_null() {
        unsafe { CFRelease(tap) };
        return Err("Could not create switch event tap run-loop source".into());
    }
    if !crate::overlay::show(
        &label_ptrs,
        &pids,
        context.selection.selected(),
        overlay_mouse_action,
        (&mut context as *mut SwitchContext).cast(),
    ) {
        unsafe {
            CFRelease(source);
            CFRelease(tap);
        }
        return Err("Could not create the window list panel".into());
    }
    unsafe {
        let current = CFRunLoopGetCurrent();
        let mode = kCFRunLoopDefaultMode;
        CFRunLoopAddSource(current, source, mode);
        CGEventTapEnable(tap, true);
        while !RESIDENT_QUIT.load(Ordering::Relaxed)
            && (!context.selection.terminal() || context.input.has_owned_keys())
        {
            let wait = deadline
                .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap_or(std::time::Duration::from_millis(100));
            if wait.is_zero() {
                context.selection.cancel();
                break;
            }
            CFRunLoopRunInMode(mode, wait.as_secs_f64().min(0.1), false);
            crate::overlay::select(context.selection.selected());
        }
        CGEventTapEnable(tap, false);
        CFRunLoopRemoveSource(current, source, mode);
        CFMachPortInvalidate(tap);
        CFRelease(source);
        CFRelease(tap);
    }
    crate::overlay::hide();
    if RESIDENT_QUIT.load(Ordering::Relaxed)
        || deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline)
    {
        context.selection.cancel();
    }
    let selected = context.selection.take_commit();
    let disabled = context.input.end == Some(crate::input::End::Disabled);
    drop(context);
    if disabled {
        return Err(
            "The switch event tap was disabled; the selected window was not raised.".into(),
        );
    }
    if let Some(index) = selected {
        let row = &snapshot.rows[index];
        remember_candidate(&snapshot, index);
        println!(
            "Committing frozen candidate {index}: {:?} — {:?} (PID {})",
            row.app_name, row.title, row.pid
        );
        raise_from_list(&snapshot.owners[row.owner], row.window)?;
    } else {
        println!("Switch cancelled or expired; no window was raised.");
    }
    Ok(())
}
