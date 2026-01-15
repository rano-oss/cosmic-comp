# Potential Fixes for IME Popup Deadlock

Based on the instrumentation added, here are the potential fixes for different deadlock scenarios that may be identified.

## Fix 1: Drop shell.read() Lock Before Calling Shell::set_focus

**Scenario**: The grab handler holds `shell.read()` while calling `Shell::set_focus`, which then tries to acquire `shell.write()`, causing a deadlock because RwLock doesn't allow upgrading read to write.

**Location**: `cosmic-comp/src/wayland/handlers/xdg_shell/mod.rs`, line ~158

**Current Code**:
```rust
if let Some(keyboard) = seat.get_keyboard() {
    // ... check if grabbed ...
    
    Shell::set_focus(
        self,
        grab.current_grab().as_ref(),
        &seat,
        Some(serial),
        false,
    );
    keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
}
```

**Problem**: At this point in the code, we've already released the `shell.read()` lock, but if there's any path that still holds it, we need to ensure it's dropped.

**Fix**: Explicitly ensure no shell locks are held:
```rust
if let Some(keyboard) = seat.get_keyboard() {
    // Ensure no shell locks are held before calling set_focus
    // (should already be the case, but make it explicit)
    
    info!("Calling Shell::set_focus with NO locks held");
    Shell::set_focus(
        self,
        grab.current_grab().as_ref(),
        &seat,
        Some(serial),
        false,
    );
    
    keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
}
```

---

## Fix 2: Clone Data Before Locking in update_focus_state

**Scenario**: The `update_focus_state` function acquires `shell.read()` for getting focus geometry, but might still hold it during keyboard operations.

**Location**: `cosmic-comp/src/shell/focus/mod.rs`, line ~357-447

**Current Code**:
```rust
if should_update_cursor && state.common.config.cosmic_conf.cursor_follows_focus && target.is_some() {
    let shell = state.common.shell.read();
    let geometry = shell.focused_geometry(target.unwrap());
    // ... use geometry ...
    mem::drop(shell);
}
```

**Problem**: The lock is properly dropped, but there may be another path where it's not.

**Fix**: Ensure the lock is dropped before any keyboard operations:
```rust
// Get all data we need from shell while holding the lock
let geometry_data = if should_update_cursor 
    && state.common.config.cosmic_conf.cursor_follows_focus 
    && target.is_some() 
{
    let shell = state.common.shell.read();
    let geometry = shell.focused_geometry(target.unwrap());
    let outputs = shell.outputs().cloned().collect::<Vec<_>>();
    mem::drop(shell);
    geometry.map(|g| (g, outputs))
} else {
    None
};

// Process geometry data without holding any locks
if let Some((geometry, outputs)) = geometry_data {
    // ... process cursor movement ...
}

// Now safe to do keyboard operations
keyboard.set_focus(state, target.cloned(), serial);
```

---

## Fix 3: Avoid Re-entrant Calls to Shell Methods

**Scenario**: `PopupKeyboardGrab::set_focus` is called while the keyboard internal lock is held, and it tries to call back into shell methods that acquire locks.

**Location**: `smithay/src/desktop/wayland/popup/grab.rs`, line ~469

**Current Code**:
```rust
pub fn set_focus(
    &mut self,
    data: &mut D,
    handle: &mut KeyboardInnerHandle<'_, D>,
    focus: Option<<D as SeatHandler>::KeyboardFocus>,
    serial: Serial,
) {
    if self.popup_grab.has_ended() {
        handle.set_focus(data, focus, serial);
        handle.unset_grab(self, data, serial, false);
        return;
    }
    
    if self.popup_grab.current_grab() == focus {
        handle.set_focus(data, focus, serial);
    }
}
```

**Problem**: `handle.set_focus` or `handle.unset_grab` might trigger callbacks that try to acquire shell locks.

**Fix**: This is tricky because `KeyboardInnerHandle` operations need to happen while the keyboard lock is held. The real fix might be in the cosmic-comp side to avoid calling into shell methods from keyboard grab callbacks. However, we can add defensive logging:

```rust
pub fn set_focus(
    &mut self,
    data: &mut D,
    handle: &mut KeyboardInnerHandle<'_, D>,
    focus: Option<<D as SeatHandler>::KeyboardFocus>,
    serial: Serial,
) {
    tracing::info!("PopupKeyboardGrab::set_focus - ensuring no re-entrant locks");
    
    if self.popup_grab.has_ended() {
        tracing::info!("PopupKeyboardGrab::set_focus - grab ended, updating focus");
        handle.set_focus(data, focus, serial);
        handle.unset_grab(self, data, serial, false);
        return;
    }
    
    if self.popup_grab.current_grab() == focus {
        tracing::info!("PopupKeyboardGrab::set_focus - focus matches current grab");
        handle.set_focus(data, focus, serial);
    } else {
        tracing::info!("PopupKeyboardGrab::set_focus - ignoring focus change (not current grab)");
    }
}
```

---

## Fix 4: Defer Shell Operations Until After Keyboard Grab

**Scenario**: The sequence of operations in `XdgShellHandler::grab` might need to be reordered to avoid holding locks during keyboard operations.

**Location**: `cosmic-comp/src/wayland/handlers/xdg_shell/mod.rs`, line ~158-170

**Current Code**:
```rust
Shell::set_focus(self, grab.current_grab().as_ref(), &seat, Some(serial), false);
keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
```

**Problem**: If `Shell::set_focus` triggers a keyboard focus change that tries to interact with the grab, we might have issues.

**Fix**: Set the keyboard grab first, then update focus:
```rust
// Set the keyboard grab first
info!("XdgShellHandler::grab - setting keyboard grab BEFORE calling Shell::set_focus");
keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
info!("XdgShellHandler::grab - keyboard grab set, now calling Shell::set_focus");

// Now update focus - the grab is already in place to handle focus changes
Shell::set_focus(
    self,
    grab.current_grab().as_ref(),
    &seat,
    Some(serial),
    false,
);
info!("XdgShellHandler::grab - Shell::set_focus completed");
```

---

## Fix 5: Check for IME Grab and Handle Specially

**Scenario**: An IME keyboard grab might be active (even if IME is not running), and it interferes with popup grabs.

**Location**: `cosmic-comp/src/wayland/handlers/xdg_shell/mod.rs`, line ~135-145

**Current Code**:
```rust
if let Some(keyboard) = seat.get_keyboard() {
    let is_grabbed = keyboard.is_grabbed();
    
    if is_grabbed && !(keyboard.has_grab(serial) || keyboard.has_grab(grab.previous_serial().unwrap_or(serial))) {
        grab.ungrab(PopupUngrabStrategy::All);
        return;
    }
```

**Problem**: This check might not properly detect IME grabs.

**Fix**: Add explicit IME grab detection:
```rust
if let Some(keyboard) = seat.get_keyboard() {
    let is_grabbed = keyboard.is_grabbed();
    info!("XdgShellHandler::grab - keyboard is_grabbed: {}", is_grabbed);
    
    // Check if there's an IME grab active
    if is_grabbed {
        let has_current_serial = keyboard.has_grab(serial);
        let has_previous_serial = keyboard.has_grab(grab.previous_serial().unwrap_or(serial));
        
        info!(
            "XdgShellHandler::grab - has_current_serial: {}, has_previous_serial: {}",
            has_current_serial, has_previous_serial
        );
        
        // If grabbed by something else (possibly IME), ungrab the popup
        if !has_current_serial && !has_previous_serial {
            info!("XdgShellHandler::grab - keyboard grabbed by something else, ungrabbing popup");
            grab.ungrab(PopupUngrabStrategy::All);
            return;
        }
    }
```

---

## Fix 6: Ensure KeyboardInnerHandle Doesn't Call Back Into Shell

**Scenario**: When `KeyboardInnerHandle::set_focus` calls `grab.set_focus`, if that grab's implementation tries to acquire shell locks, we get a deadlock.

**Location**: This is more of an architectural issue than a single-line fix.

**Problem**: The keyboard internal lock is held when calling into grab implementations, and those implementations might try to acquire other locks.

**Solution**: Review all `KeyboardGrab` implementations (especially `InputMethodKeyboardGrab` and `PopupKeyboardGrab`) to ensure they:
1. Never acquire shell locks while being called from within keyboard lock
2. Clone any needed data before the keyboard lock is acquired
3. Use deferred operations (e.g., event loop callbacks) for operations that need shell access

**Implementation**: Add a trait requirement or documentation:
```rust
/// SAFETY: Implementations of this trait MUST NOT:
/// - Acquire any RwLock or Mutex on the Shell/State while being called
/// - Make synchronous Wayland protocol calls that might trigger callbacks
/// - Call back into KeyboardHandle methods (except through KeyboardInnerHandle)
/// 
/// If you need to access shell state or make protocol calls, clone the necessary
/// data first and use an event loop callback or defer the operation.
pub trait KeyboardGrab<D: SeatHandler>: fmt::Debug {
    // ...
}
```

---

## Testing Each Fix

After applying each fix:

1. Run the debug build: `./run_with_debug_logs.sh`
2. Reproduce the freeze by clicking on panel popups
3. Check the logs to see if the sequence completes
4. Verify popups work correctly with IME configured
5. Test with IME actually running (not just configured)

## Expected Outcome

After the correct fix is applied, the log sequence should complete without freezing:

```
XdgShellHandler::grab - EXIT
```

And popups should open/close normally even with IME configuration present.

## If Multiple Fixes Are Needed

It's possible that the deadlock has multiple contributing factors. If one fix doesn't resolve it:

1. Check the logs to see if execution progresses further
2. Apply the next relevant fix
3. Repeat until the issue is resolved

## Contact and Further Investigation

If none of these fixes work, the issue might be:
- In a different code path not covered by the logging
- Related to Wayland protocol timing or event ordering
- A deeper architectural issue requiring more significant refactoring

In that case, consider:
- Using a memory/thread debugger like Valgrind with helgrind
- Adding even more granular logging
- Creating a minimal reproduction case outside cosmic-comp
- Consulting with Smithay maintainers for guidance