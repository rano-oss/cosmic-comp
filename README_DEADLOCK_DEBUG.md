# IME Popup Deadlock Debugging - Summary

## Overview

This document summarizes the comprehensive debugging instrumentation added to diagnose and fix a deadlock that occurs when opening popups (volume, network, etc.) in Cosmic-Comp **only when an Input Method Editor (IME) is configured**.

## The Problem

- **Symptom**: Compositor freezes when clicking panel popups
- **Trigger**: Only happens when IME is configured (input_method_keyboard_map exists)
- **Oddity**: Occurs even when IME (chewingwl) is NOT running
- **Location**: Freeze happens during popup grab setup, somewhere between `grab_popup` call and grab completion

## What's Been Done

### 1. Added Comprehensive Logging

We've instrumented the entire call chain from popup creation to keyboard grab with detailed logging to identify exactly where locks are acquired and where execution stops.

**Modified Files:**
```
cosmic-comp/src/wayland/handlers/xdg_shell/mod.rs  - Popup grab handler
cosmic-comp/src/shell/focus/mod.rs                 - Focus management and lock tracking
smithay/src/input/keyboard/mod.rs                  - Keyboard handle operations
smithay/src/desktop/wayland/popup/grab.rs          - Popup keyboard grab
```

### 2. Created Debug Tools

**run_with_debug_logs.sh**
- Runs cosmic-comp with info-level logging
- Saves logs to timestamped file
- Shows real-time output

**DEBUGGING_IME_POPUP_DEADLOCK.md**
- Complete guide to using the instrumentation
- Expected log sequences
- How to identify different deadlock patterns
- Tools for further investigation (strace, gdb)

**POTENTIAL_FIXES.md**
- 6 different potential fixes for different deadlock scenarios
- Code examples for each fix
- Testing procedures

### 3. Logging Coverage

The instrumentation tracks:

#### XDG Shell Handler
- Entry/exit of grab function
- Shell read lock acquisition and release
- Call to PopupManager::grab_popup
- Call to Shell::set_focus (potential write lock)
- Call to keyboard.set_grab

#### Shell Focus Management
- Shell write lock for append_focus_stack
- Shell write lock for update_active
- Shell read lock for cursor updates
- Shell read lock for output focus
- Keyboard grab operations

#### Keyboard Handle
- Internal keyboard state lock acquisition
- Call to grab.set_focus via with_grab
- Lock release

#### Popup Keyboard Grab
- Whether grab has ended
- Whether focus matches current grab
- Calls to handle.set_focus and handle.unset_grab

## How to Use

### Step 1: Build
```bash
cd cosmic-comp
cargo build --release
```

### Step 2: Run with Logging
```bash
./run_with_debug_logs.sh
```

### Step 3: Reproduce the Freeze
1. Ensure IME is configured (input_method_keyboard_map exists)
2. Click on a panel popup (volume control recommended)
3. Watch where the logs stop

### Step 4: Analyze the Logs

Look for the **last log message** before the freeze. This tells you where execution stopped.

**Example - Lock Inversion Deadlock:**
```
XdgShellHandler::grab - calling Shell::set_focus (THIS MAY TRY TO ACQUIRE shell.write() LOCK)
Shell::set_focus - ENTRY
Shell::set_focus - about to acquire shell.write() lock for append_focus_stack
[FREEZE - no more logs]
```

**Example - Keyboard Grab Deadlock:**
```
KeyboardHandle::set_focus - internal lock ACQUIRED
KeyboardHandle::set_focus - inside with_grab callback, calling grab.set_focus
PopupKeyboardGrab::set_focus - ENTRY
[FREEZE - no more logs]
```

### Step 5: Apply the Appropriate Fix

Refer to `POTENTIAL_FIXES.md` for detailed fixes based on where the freeze occurs.

## Most Likely Scenarios

### Scenario A: RwLock Read-to-Write Upgrade Deadlock

**Symptoms**: Logs stop at `Shell::set_focus - about to acquire shell.write() lock`

**Cause**: RwLock doesn't allow upgrading read lock to write lock. If `shell.read()` is still held somewhere when we try to acquire `shell.write()`, it deadlocks.

**Fix**: Ensure all `shell.read()` locks are dropped before calling `Shell::set_focus`.

### Scenario B: Keyboard Internal Lock + Shell Lock Deadlock

**Symptoms**: Logs stop during `KeyboardHandle::set_focus` or `PopupKeyboardGrab::set_focus`

**Cause**: Keyboard internal lock is held, and a callback tries to acquire shell lock.

**Fix**: Avoid calling shell methods from within keyboard grab callbacks. Clone data first.

### Scenario C: IME Grab Interference

**Symptoms**: Freeze only with IME configured, even though IME is not running

**Cause**: IME grab might be registered but dormant, and interferes with popup grabs.

**Fix**: Explicitly check for and handle IME grabs before setting popup grab.

## Expected Normal Log Sequence

When working correctly, you should see this complete sequence:

```
XdgShellHandler::grab - ENTRY
XdgShellHandler::grab - acquiring shell.read() lock NOW
XdgShellHandler::grab - shell.read() lock ACQUIRED
XdgShellHandler::grab - shell.read() lock DROPPED
XdgShellHandler::grab - about to call grab_popup (no locks held)
XdgShellHandler::grab - grab_popup returned
XdgShellHandler::grab - calling Shell::set_focus
Shell::set_focus - ENTRY
Shell::set_focus - about to acquire shell.write() lock for append_focus_stack
Shell::set_focus - shell.write() lock released after append_focus_stack
Shell::set_focus - calling update_focus_state
update_focus_state - ENTRY
update_focus_state - calling keyboard.set_focus
KeyboardHandle::set_focus - ENTRY
KeyboardHandle::set_focus - internal lock ACQUIRED
KeyboardHandle::set_focus - calling with_grab
KeyboardHandle::set_focus - inside with_grab callback, calling grab.set_focus
PopupKeyboardGrab::set_focus - ENTRY
PopupKeyboardGrab::set_focus - focus matches current grab, calling handle.set_focus
PopupKeyboardGrab::set_focus - EXIT
KeyboardHandle::set_focus - grab.set_focus completed
KeyboardHandle::set_focus - EXIT
update_focus_state - keyboard.set_focus completed
update_focus_state - EXIT
Shell::set_focus - update_focus_state completed
Shell::set_focus - about to acquire shell.write() lock for update_active
Shell::set_focus - EXIT
XdgShellHandler::grab - Shell::set_focus completed
XdgShellHandler::grab - about to call keyboard.set_grab
XdgShellHandler::grab - keyboard.set_grab completed successfully
XdgShellHandler::grab - EXIT
```

## Quick Reference: Lock Hierarchy

To avoid deadlocks, locks should be acquired in this order:

1. Shell locks (shell.read() or shell.write())
2. Keyboard internal state lock
3. Individual surface/window locks

**Never**:
- Hold shell.read() while trying to acquire shell.write()
- Hold keyboard internal lock while trying to acquire shell lock
- Make synchronous Wayland protocol calls while holding locks

## Additional Debugging Tools

### Get Thread Backtrace When Frozen

```bash
# Find PID
ps aux | grep cosmic-comp

# Attach gdb
sudo gdb -p <PID>

# Get all thread backtraces
(gdb) thread apply all bt

# Look for threads waiting on locks
(gdb) thread apply all bt | grep -A 5 -B 5 "lock\|mutex\|rwlock"
```

### Use strace to See System Calls

```bash
sudo strace -p <PID> -o strace.log -f -tt
# Trigger freeze
# Analyze strace.log for last syscalls before freeze
```

### Check for Lock Contention

```bash
# Use perf to see where time is spent
sudo perf record -p <PID> -g
# Trigger freeze, wait 10 seconds, Ctrl+C
sudo perf report
```

## Files Reference

| File | Purpose |
|------|---------|
| `run_with_debug_logs.sh` | Script to run cosmic-comp with debug logging |
| `DEBUGGING_IME_POPUP_DEADLOCK.md` | Detailed debugging guide |
| `POTENTIAL_FIXES.md` | Fixes for different deadlock scenarios |
| `README_DEADLOCK_DEBUG.md` | This file - quick start guide |

## Next Steps

1. **Run the debug build** and reproduce the freeze
2. **Identify where logs stop** - this is your deadlock location
3. **Consult POTENTIAL_FIXES.md** for the appropriate fix
4. **Apply the fix** and test again
5. **Report findings** to Smithay/Cosmic-Comp maintainers

## Key Insights from Previous Investigation

- The original IME keyboard grab deadlock (holding locks during protocol calls) has been fixed in Smithay
- This is a **new deadlock** triggered by IME configuration presence, not IME activity
- The freeze occurs in the Wayland event loop or compositor code, not in the IME implementation itself
- The issue is likely a lock ordering problem or re-entrant lock acquisition

## Success Criteria

After fixing:
- Popups open and close normally with IME configured
- No freeze when clicking panel items
- Logs show complete sequence from ENTRY to EXIT
- Works both with and without IME actually running

## Contact

If the provided instrumentation and fixes don't resolve the issue:
1. Share the complete log output (especially the last 100 lines before freeze)
2. Share thread backtraces from gdb
3. Consider filing a detailed bug report with:
   - Log files
   - Steps to reproduce
   - System configuration (IME setup, Wayland version, etc.)
   - Thread dumps

---

**Generated**: This instrumentation was added to debug a persistent deadlock issue in Cosmic-Comp when IME is configured. The goal is to identify the exact lock acquisition sequence that causes the freeze and apply the appropriate fix.