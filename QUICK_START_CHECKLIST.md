# Quick Start Checklist - Debugging IME Popup Deadlock

## Prerequisites
- [ ] IME is configured (input_method_keyboard_map file exists)
- [ ] You can reproduce the freeze by clicking panel popups
- [ ] You have built the instrumented version: `cargo build --release`

## Step-by-Step Process

### 1. Run the Debug Build
```bash
cd cosmic-comp
./run_with_debug_logs.sh
```

- [ ] Cosmic-comp starts successfully
- [ ] Logs are being written to console and file
- [ ] You see "Starting cosmic-comp with debug logging..." message

### 2. Reproduce the Freeze
- [ ] Click on volume control in the panel
- [ ] Watch the logs in real-time
- [ ] Note when logging stops (compositor freezes)

### 3. Find the Last Log Message
Look at the console or log file and find the **last message** before the freeze.

Mark which section it stopped in:

- [ ] **Section A**: In `XdgShellHandler::grab` before calling `Shell::set_focus`
- [ ] **Section B**: In `Shell::set_focus` trying to acquire `shell.write()` lock
- [ ] **Section C**: In `update_focus_state` calling `keyboard.set_focus`
- [ ] **Section D**: In `KeyboardHandle::set_focus` with internal lock acquired
- [ ] **Section E**: In `PopupKeyboardGrab::set_focus`
- [ ] **Section F**: Somewhere else (note the exact message): _______________

### 4. Identify the Deadlock Pattern

Based on where it stopped, check the pattern:

#### Pattern 1: Lock Inversion (ABBA Deadlock)
- [ ] Logs show "shell.read() lock ACQUIRED" but never "DROPPED"
- [ ] Then logs show "about to acquire shell.write() lock" and stops
- [ ] **→ This is a read-to-write lock upgrade deadlock**
- [ ] **→ Apply Fix 1 from POTENTIAL_FIXES.md**

#### Pattern 2: Keyboard Lock + Shell Lock
- [ ] Logs show "KeyboardHandle::set_focus - internal lock ACQUIRED"
- [ ] Then stops inside `PopupKeyboardGrab::set_focus` or similar
- [ ] **→ Keyboard lock is held while trying to acquire shell lock**
- [ ] **→ Apply Fix 2 or Fix 3 from POTENTIAL_FIXES.md**

#### Pattern 3: Grab Operation Deadlock
- [ ] Logs show "calling keyboard.set_grab" but never "completed"
- [ ] Or "PopupKeyboardGrab::set_focus - ENTRY" but never "EXIT"
- [ ] **→ Grab operation is waiting on something**
- [ ] **→ Apply Fix 4 or Fix 5 from POTENTIAL_FIXES.md**

### 5. Copy the Last 50 Lines of Logs

```bash
# Get the last 50 lines from the log file
tail -50 cosmic_comp_debug_*.log
```

Copy and save these lines - they contain the crucial information.

Paste them here:
```
[PASTE YOUR LOG LINES HERE]
```

### 6. Apply the Appropriate Fix

Go to `POTENTIAL_FIXES.md` and apply the fix corresponding to your pattern:

- [ ] Read the fix description
- [ ] Understand what it does
- [ ] Apply the code changes
- [ ] Rebuild: `cargo build --release`
- [ ] Test again with `./run_with_debug_logs.sh`

### 7. Verify the Fix

After applying the fix:

- [ ] Logs show complete sequence from ENTRY to EXIT
- [ ] No freeze when clicking panel popups
- [ ] Popups open and close normally
- [ ] Works with IME configured
- [ ] Works with IME running (if applicable)

### 8. If Still Freezing

- [ ] Check if logs progress further than before (partial success)
- [ ] Try the next relevant fix from `POTENTIAL_FIXES.md`
- [ ] Use gdb to get thread backtraces (see below)
- [ ] Consult `DEBUGGING_IME_POPUP_DEADLOCK.md` for advanced techniques

## Advanced Debugging (If Needed)

### Get Thread Backtraces
```bash
# Find cosmic-comp PID
ps aux | grep cosmic-comp | grep -v grep

# Attach gdb (while frozen)
sudo gdb -p <PID>

# In gdb:
(gdb) thread apply all bt

# Save output to file:
(gdb) set logging on
(gdb) set logging file gdb_backtrace.txt
(gdb) thread apply all bt
(gdb) quit
```

- [ ] Collected thread backtraces
- [ ] Saved to file
- [ ] Identified which thread is waiting on which lock

### Use strace
```bash
# Attach strace before triggering freeze
sudo strace -p <PID> -o strace.log -f -tt -e trace=futex,poll,epoll_wait

# Trigger freeze, wait 5 seconds, then Ctrl+C

# Look at the last lines
tail -50 strace.log
```

- [ ] Collected strace output
- [ ] Identified last syscalls before freeze

## Checklist for Reporting

If none of the fixes work, prepare a report with:

- [ ] Complete log file from freeze
- [ ] Last 50-100 lines clearly highlighted
- [ ] Thread backtraces from gdb
- [ ] System information:
  - [ ] OS and version: _______________
  - [ ] Wayland version: _______________
  - [ ] IME being used: _______________
  - [ ] Cosmic-comp commit hash: _______________
  - [ ] Smithay commit hash: _______________

- [ ] Steps to reproduce (detailed)
- [ ] Whether it happens every time or intermittently
- [ ] Any workarounds found

## Quick Reference

| Last Log Message Contains | Likely Cause | Fix Number |
|---------------------------|--------------|------------|
| "shell.read() lock ACQUIRED" (never dropped) | Lock inversion | Fix 1 |
| "about to acquire shell.write()" (stops) | Read-to-write upgrade | Fix 1 |
| "KeyboardHandle::set_focus - internal lock ACQUIRED" (stops in callback) | Re-entrant lock | Fix 2 or 3 |
| "PopupKeyboardGrab::set_focus - ENTRY" (never exits) | Grab callback deadlock | Fix 3 |
| "calling keyboard.set_grab" (never completes) | Grab setup issue | Fix 4 or 5 |

## Success Indicators

You know it's fixed when:
- ✅ All log sequences complete from ENTRY to EXIT
- ✅ No freezes during normal popup operations
- ✅ Can open/close volume control repeatedly
- ✅ Can open/close network settings repeatedly
- ✅ Works with IME configured but not running
- ✅ Works with IME actually running

## Files You'll Need

1. `run_with_debug_logs.sh` - Run cosmic-comp with logging
2. `DEBUGGING_IME_POPUP_DEADLOCK.md` - Full debugging guide
3. `POTENTIAL_FIXES.md` - Fixes for each scenario
4. `README_DEADLOCK_DEBUG.md` - Overview and explanation
5. This file - Quick checklist

## Time Estimates

- First run and log collection: **5 minutes**
- Analyzing logs and identifying pattern: **10 minutes**
- Applying a fix: **15 minutes**
- Testing the fix: **5 minutes**
- **Total for one iteration: ~35 minutes**

If multiple fixes needed, multiply accordingly.

## Common Pitfalls

- ❌ Not waiting long enough to confirm freeze (wait at least 10 seconds)
- ❌ Not saving the log file before restarting
- ❌ Applying multiple fixes at once (can't tell which worked)
- ❌ Not checking if the fix actually compiles
- ❌ Testing without IME configured (won't reproduce issue)

## Emergency Workaround

If you need cosmic-comp working NOW and can't wait for the fix:

```bash
# Remove IME configuration temporarily
mv ~/.config/cosmic/com.system76.CosmicComp/input_method_keyboard_map \
   ~/.config/cosmic/com.system76.CosmicComp/input_method_keyboard_map.disabled

# Restart cosmic-comp
# Popups should work now (but no IME support)
```

- [ ] Applied workaround
- [ ] Verified popups work without IME config
- [ ] Remember to re-enable when fix is found

---

**Last Updated**: When you applied the instrumentation
**Status**: Ready to debug
**Goal**: Identify and fix the IME popup deadlock

Good luck! 🚀