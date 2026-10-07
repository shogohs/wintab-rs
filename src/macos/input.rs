use super::windows::{
    candidate_snapshot, candidate_snapshot_until, cf_string, copy_attribute, focused_index,
    raise_from_list, title, window_at, window_list, CandidateRow, CandidateSnapshot,
};
use super::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

static RESIDENT_QUIT: AtomicBool = AtomicBool::new(false);
static DISCOVERY_RUNNING: AtomicBool = AtomicBool::new(false);
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
    pending: Option<PendingSwitch>,
    tail: Option<crate::input::CaptureState>,
    active: *mut SwitchContext,
}

struct PendingSwitch {
    input: crate::input::PendingInput,
    deadline: std::time::Instant,
}

struct SnapshotForMain(CandidateSnapshot, Option<usize>);
// SAFETY: the worker creates and exclusively owns this CF-backed snapshot, then
// transfers it once through a channel; no CF object is accessed concurrently.
unsafe impl Send for SnapshotForMain {}

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
            (1_u64 << K_CG_EVENT_KEY_DOWN)
                | (1_u64 << K_CG_EVENT_KEY_UP)
                | (1_u64 << K_CG_EVENT_FLAGS_CHANGED)
                | (1_u64 << K_CG_EVENT_LEFT_MOUSE_DOWN),
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
    let active = unsafe { (*user_info.cast::<ResidentContext>()).active };
    if !active.is_null() {
        return switch_event(_proxy, kind, event, active.cast());
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
    if kind == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        if context.pending.is_some() {
            if let Some(pending) = context.pending.as_mut() {
                pending.input.handle(crate::input::Event::Disabled);
            }
        }
        if context.enabled {
            unsafe { CGEventTapEnable(context.tap, true) };
        }
        return event;
    }
    if kind == K_CG_EVENT_TAP_DISABLED_BY_USER {
        context.enabled = false;
        if let Some(pending) = context.pending.as_mut() {
            pending.input.handle(crate::input::Event::Disabled);
        }
        return event;
    }
    if event.is_null() {
        return event;
    }
    let (tail_suppress, clear_tail) = if let Some(tail) = context.tail.as_mut() {
        let output = tail.handle_owned_tail(decode_event(kind, event));
        let drained = !tail.has_owned_keys();
        (output.suppress, drained)
    } else {
        (false, false)
    };
    if clear_tail {
        context.tail = None;
    }
    if tail_suppress {
        return ptr::null_mut();
    }
    if context.busy {
        if let Some(pending) = context.pending.as_mut() {
            let decoded = decode_event(kind, event);
            let output = pending.input.handle(decoded);
            return if output.suppress {
                ptr::null_mut()
            } else {
                event
            };
        }
        return event;
    }
    if context.tail.is_some() {
        return event;
    }
    if kind != K_CG_EVENT_KEY_DOWN || !context.enabled {
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let direction = if flags & (1 << 17) != 0 {
        crate::input::Direction::Reverse
    } else {
        crate::input::Direction::Forward
    };
    if DISCOVERY_RUNNING.load(Ordering::Acquire) {
        context.busy = false;
        return event;
    }
    context.pending = Some(PendingSwitch {
        input: crate::input::PendingInput::begin(direction),
        deadline,
    });
    crate::ui::defer_switch(begin_resident_switch, user_info, flags & (1 << 17) != 0);
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
                let (ax, input) = permission_status();
                context.permitted = ax && input;
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
            crate::ui::open_privacy_settings();
            context.enabled as i32
        }
        3 => {
            let (ax, input) = permission_status();
            context.permitted = ax && input;
            if !context.permitted && context.enabled {
                context.enabled = false;
                if !context.tap.is_null() {
                    unsafe { CGEventTapEnable(context.tap, false) };
                }
            }
            ax as i32 | ((input as i32) << 1) | ((context.enabled as i32) << 2)
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

extern "C" fn begin_resident_switch(user_info: *mut c_void, _reverse: i32) {
    if user_info.is_null() {
        return;
    }
    let (should_run, deadline) = {
        let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
        (
            context.enabled && context.permitted,
            context.pending.as_ref().map(|p| p.deadline),
        )
    };
    let result = if should_run && !RESIDENT_QUIT.load(Ordering::Relaxed) {
        if DISCOVERY_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            match std::thread::Builder::new()
                .name("wintab-ax-discovery".into())
                .spawn(move || {
                    let result = candidate_snapshot_until(deadline).map(|snapshot| {
                        let focused = focused_candidate(&snapshot);
                        SnapshotForMain(snapshot, focused)
                    });
                    let _ = sender.send(result);
                    DISCOVERY_RUNNING.store(false, Ordering::Release);
                }) {
                Ok(_worker) => {
                    let mut result = None;
                    while deadline.is_some_and(|end| std::time::Instant::now() < end)
                        && !RESIDENT_QUIT.load(Ordering::Relaxed)
                    {
                        if let Ok(value) = receiver.try_recv() {
                            result = Some(value);
                            break;
                        }
                        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.025, false) };
                    }
                    result.unwrap_or_else(|| {
                        Err("Resident window discovery expired after 10 seconds".into())
                    })
                }
                Err(error) => {
                    DISCOVERY_RUNNING.store(false, Ordering::Release);
                    Err(format!("Could not start AX discovery: {error}"))
                }
            }
        } else {
            Err("Previous resident window discovery is still finishing".into())
        }
    } else {
        Err("Resident switch cancelled".into())
    };

    let replay = {
        let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
        context.pending.take()
    };
    let mut replay = replay;
    match result {
        Ok(SnapshotForMain(mut snapshot, focused)) => {
            if let Some(pending) = replay.take() {
                if pending.deadline > std::time::Instant::now()
                    && !pending.input.is_disabled()
                    && !RESIDENT_QUIT.load(Ordering::Relaxed)
                {
                    let focused = order_candidates(&mut snapshot, focused);
                    match run_switch_session(
                        snapshot,
                        focused,
                        None,
                        Some(pending),
                        user_info.cast(),
                    ) {
                        Ok(()) => {}
                        Err(error) => eprintln!("wintab-rs: {error}"),
                    }
                } else {
                    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
                    if pending.input.has_owned_keys() {
                        context.tail = Some(pending.input.into_capture());
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("wintab-rs: {error}");
            if let Some(pending) = replay.take() {
                let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
                if pending.input.has_owned_keys() {
                    context.tail = Some(pending.input.into_capture());
                }
            }
        }
    }
    let context = unsafe { &mut *user_info.cast::<ResidentContext>() };
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
    crate::ui::status_prepare();
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
        pending: None,
        tail: None,
        active: ptr::null_mut(),
    };
    if context.permitted && !install_resident_tap(&mut context) {
        context.enabled = false;
    }
    let ran = crate::ui::status_run(
        resident_menu_action,
        (&mut context as *mut ResidentContext).cast(),
        context.enabled,
        ax,
        input_monitoring,
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

pub(super) fn decode_event(kind: CGEventType, event: CGEventRef) -> crate::input::Event {
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
    crate::ui::select(context.selection.selected());
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
    if kind == K_CG_EVENT_LEFT_MOUSE_DOWN {
        let active = {
            let context = unsafe { &mut *user_info.cast::<SwitchContext>() };
            !context.selection.terminal()
        };
        if active {
            let action = crate::ui::handle_click();
            let context = unsafe { &mut *user_info.cast::<SwitchContext>() };
            match action {
                -1 => context.selection.cancel(),
                1 => return ptr::null_mut(),
                _ => {}
            }
        }
        return event;
    }
    let context = unsafe { &mut *user_info.cast::<SwitchContext>() };
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
    run_switch_session(snapshot, focused, Some(seconds), None, ptr::null_mut())
}

fn run_switch_session(
    snapshot: CandidateSnapshot,
    focused: Option<usize>,
    seconds: Option<f64>,
    pending: Option<PendingSwitch>,
    resident: *mut ResidentContext,
) -> Result<(), String> {
    let count = snapshot.rows.len();
    if count == 0 {
        if let Some(pending) = pending {
            if !resident.is_null() && pending.input.has_owned_keys() {
                unsafe {
                    (*resident).tail = Some(pending.input.into_capture());
                }
            }
        }
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
    let deadline = pending
        .as_ref()
        .map(|pending| pending.deadline)
        .or_else(|| {
            seconds.map(|seconds| {
                std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds)
            })
        });
    let (input, selection) = if let Some(pending) = pending {
        pending
            .input
            .into_session(count, focused)
            .ok_or("No supported AX standard windows were found")?
    } else {
        (
            crate::input::CaptureState::default(),
            crate::input::Selection::new(count, focused).unwrap(),
        )
    };
    let mut context = SwitchContext {
        input,
        selection,
        deadline,
    };
    let mask = (1_u64 << K_CG_EVENT_KEY_DOWN)
        | (1_u64 << K_CG_EVENT_KEY_UP)
        | (1_u64 << K_CG_EVENT_FLAGS_CHANGED)
        | (1_u64 << K_CG_EVENT_LEFT_MOUSE_DOWN);
    let context_ptr = ptr::addr_of_mut!(context);
    let resident_mode = !resident.is_null();
    let tap = if resident_mode {
        unsafe { (*resident).tap }
    } else {
        unsafe {
            CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                0,
                mask,
                switch_event,
                context_ptr.cast(),
            )
        }
    };
    if tap.is_null() {
        if !resident.is_null() && context.input.has_owned_keys() {
            unsafe {
                (*resident).tail = Some(std::mem::take(&mut context.input));
            }
        }
        return Err("Could not create active event tap; check Input Monitoring permission and event-tap requirements.".into());
    }
    let source = if resident_mode {
        unsafe { (*resident).source }
    } else {
        unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) }
    };
    if source.is_null() {
        if !resident_mode {
            unsafe { CFRelease(tap) }
        }
        if !resident.is_null() && context.input.has_owned_keys() {
            unsafe {
                (*resident).tail = Some(std::mem::take(&mut context.input));
            }
        }
        return Err("Could not create switch event tap run-loop source".into());
    }
    if resident_mode {
        unsafe {
            (*resident).active = context_ptr;
        }
    }
    let initial_selected = unsafe { (*context_ptr).selection.selected() };
    if !crate::ui::show(
        &label_ptrs,
        &pids,
        initial_selected,
        overlay_mouse_action,
        context_ptr.cast(),
    ) {
        if resident_mode {
            unsafe {
                (*resident).active = ptr::null_mut();
            }
        }
        if !resident_mode {
            unsafe {
                CFRelease(source);
                CFRelease(tap);
            }
        }
        if !resident.is_null() && context.input.has_owned_keys() {
            unsafe {
                (*resident).tail = Some(std::mem::take(&mut context.input));
            }
        }
        return Err("Could not create the window list panel".into());
    }
    unsafe {
        let current = CFRunLoopGetCurrent();
        let mode = kCFRunLoopDefaultMode;
        if !resident_mode {
            CFRunLoopAddSource(current, source, mode);
            CGEventTapEnable(tap, true);
        }
        loop {
            let (done, wait) = {
                let context = &mut *context_ptr;
                let expired =
                    deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline);
                if expired {
                    context.selection.cancel();
                }
                let done = expired
                    || RESIDENT_QUIT.load(Ordering::Relaxed)
                    || (context.selection.terminal() && !context.input.has_owned_keys());
                let wait = if expired {
                    std::time::Duration::ZERO
                } else {
                    deadline
                        .map(|deadline| {
                            deadline.saturating_duration_since(std::time::Instant::now())
                        })
                        .unwrap_or(std::time::Duration::from_millis(100))
                };
                (done, wait)
            };
            if done {
                break;
            }
            CFRunLoopRunInMode(mode, wait.as_secs_f64().min(0.1), false);
            let selected = (*context_ptr).selection.selected();
            crate::ui::select(selected);
        }
        let tail = context
            .input
            .has_owned_keys()
            .then(|| std::mem::take(&mut context.input));
        if !resident.is_null() {
            (*resident).tail = tail;
        }
        if resident_mode {
            (*resident).active = ptr::null_mut();
        } else {
            CGEventTapEnable(tap, false);
            CFRunLoopRemoveSource(current, source, mode);
            CFMachPortInvalidate(tap);
            CFRelease(source);
            CFRelease(tap);
        }
    }
    crate::ui::hide();
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
        println!(
            "Committing frozen candidate {index}: {:?} — {:?} (PID {})",
            row.app_name, row.title, row.pid
        );
        raise_from_list(&snapshot.owners[row.owner], row.window)?;
        if let Some((pid, window)) = focused_window() {
            if pid == row.pid {
                let target = window_at(&snapshot.owners[row.owner], row.window)?;
                if unsafe { CFEqual(window.0, target) != 0 } {
                    remember_window(pid, window.0);
                }
            }
        }
    } else {
        println!("Switch cancelled or expired; no window was raised.");
    }
    Ok(())
}
