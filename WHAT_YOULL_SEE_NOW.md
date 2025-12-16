# What You'll See Now - Enhanced Input Method Logging

## Overview

With the enhanced logging, you'll now see **much more detail** about input method registration, activation, and keyboard layout changes.

## When You Start an Input Method

### Example: Starting fcitx5

When you run `fcitx5 &`, you should see:

```
[SMITHAY timestamp] ========== INPUT METHOD REGISTRATION ==========
[SMITHAY timestamp] Got client credentials: pid=12345
[SMITHAY timestamp] Attempting to get app_id for pid 12345
[SMITHAY timestamp] Trying to read: /proc/12345/comm
[SMITHAY timestamp] Successfully read from /proc/comm: 'fcitx5'
[SMITHAY timestamp] Extracted app_id from pid 12345: 'fcitx5'
[SMITHAY timestamp] Registering input method instance with app_id: 'fcitx5'
[SMITHAY timestamp] Total input method instances now: 1
[SMITHAY timestamp] Input method added, compositor will be notified via new_input_method callback
[SMITHAY timestamp] ========== END REGISTRATION ==========
[timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[timestamp] Syncing input method for seat after registration of 'fcitx5'
[timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=false
[timestamp] Keyboard currently grabbed: false
```

### If You Don't Have a Mapping File

```
[timestamp] No input method mapping configured - deactivating input method
[SMITHAY timestamp] ========== deactivate_input_method() START ==========
[SMITHAY timestamp] No active input method to deactivate
[SMITHAY timestamp] Checking for keyboard grab to release
[SMITHAY timestamp] No keyboard grab found to release
[SMITHAY timestamp] ========== deactivate_input_method() END ==========
[timestamp] Keyboard grabbed after deactivation: false
```

### If Input Method Tries to Grab Keyboard While Inactive

```
[SMITHAY timestamp] Blocking keyboard grab from inactive input method (id: ObjectId(...)) - creating inert object
[SMITHAY timestamp] Inert keyboard object created, actual grab blocked
```

**Result**: ✅ NO CRASH - Input method runs but cannot access keyboard

## When You Switch Keyboard Layouts

### Example: Switching from "no" to "tw"

```
[timestamp] KEYBOARD LAYOUT CHANGED to 'tw'
[timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=false
[timestamp] Keyboard currently grabbed: false
[timestamp] Current keyboard layout (full): 'tw'
[timestamp] Primary layout: 'tw'
[timestamp] Found mapping: layout 'tw' -> app_id 'fcitx5'
[timestamp] Successfully set 'fcitx5' as active instance
[SMITHAY timestamp] activate_input_method() called
[SMITHAY timestamp] Active input method id: ObjectId(...)
[SMITHAY timestamp] Found instance for activation: app_id=fcitx5
[SMITHAY timestamp] Calling activate() on input method instance
[SMITHAY timestamp] Input method has no popup surface
[SMITHAY timestamp] activate_input_method() completed
[timestamp] Found focused text input, activating input method on surface
[timestamp] Keyboard grabbed after activation: false
[timestamp] ========== END SYNC ==========
```

### If Input Method Grabs Keyboard After Activation

```
[SMITHAY timestamp] Processing keyboard grab request from active input method (id: ObjectId(...))
[SMITHAY timestamp] Keyboard grab successfully installed
```

## When You Have Multiple Layouts (like "no,us,tw")

With layout string `"no,us,tw"`, the system uses the **first** layout:

```
[timestamp] Current keyboard layout (full): 'no,us,tw'
[timestamp] Primary layout: 'no'
```

The mapping file should map `"no"` (not `"no,us,tw"`):

```ron
{
    "no": "none",
    "us": "none", 
    "tw": "fcitx5",
}
```

## Complete Flow Example: Start Input Method → Switch Layout → Switch Back

### 1. Start fcitx5 on "no" layout (unmapped)

```
[SMITHAY timestamp] ========== INPUT METHOD REGISTRATION ==========
[SMITHAY timestamp] Successfully read from /proc/comm: 'fcitx5'
[SMITHAY timestamp] Registering input method instance with app_id: 'fcitx5'
[SMITHAY timestamp] ========== END REGISTRATION ==========
[timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=false
[timestamp] No mapping found for layout 'no' - deactivating input method
[SMITHAY timestamp] Blocking keyboard grab from inactive input method - creating inert object
[SMITHAY timestamp] Inert keyboard object created, actual grab blocked
```

### 2. Switch to "tw" layout (mapped to fcitx5)

```
[timestamp] KEYBOARD LAYOUT CHANGED to 'tw'
[timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=false
[timestamp] Current keyboard layout (full): 'tw'
[timestamp] Primary layout: 'tw'
[timestamp] Found mapping: layout 'tw' -> app_id 'fcitx5'
[timestamp] Successfully set 'fcitx5' as active instance
[SMITHAY timestamp] activate_input_method() called
[SMITHAY timestamp] Calling activate() on input method instance
[SMITHAY timestamp] Processing keyboard grab request from active input method
[SMITHAY timestamp] Keyboard grab successfully installed
```

### 3. Switch back to "no" layout (unmapped)

```
[timestamp] KEYBOARD LAYOUT CHANGED to 'no'
[timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=true
[timestamp] No mapping found for layout 'no' - deactivating input method
[SMITHAY timestamp] ========== deactivate_input_method() START ==========
[SMITHAY timestamp] Deactivating input method with id: ObjectId(...)
[SMITHAY timestamp] Found instance to deactivate: app_id=fcitx5
[SMITHAY timestamp] Calling deactivate() on input method instance
[SMITHAY timestamp] Calling done() on input method instance
[SMITHAY timestamp] No popup to dismiss
[SMITHAY timestamp] Checking for keyboard grab to release
[SMITHAY timestamp] Found keyboard grab, releasing it
[SMITHAY timestamp] Calling unset_grab on keyboard handle
[SMITHAY timestamp] Keyboard grab released successfully
[SMITHAY timestamp] ========== deactivate_input_method() END ==========
```

## Troubleshooting Guide

### Problem: No app_id showing

**Symptom**: You see `app_id='unknown-12345'` instead of `app_id='fcitx5'`

**Check logs for**:
```
[SMITHAY timestamp] Failed to read /proc/comm, trying cmdline...
[SMITHAY timestamp] Failed to read cmdline
[SMITHAY timestamp] Using final fallback: 'unknown-12345'
```

**Possible causes**:
- Input method doesn't set process name
- Permission issues reading /proc
- Input method running in container/sandbox

**Solution**: Use the PID-based name in your mapping:
```ron
{
    "tw": "unknown-12345",
}
```

### Problem: Not seeing "NEW INPUT METHOD REGISTERED"

**Symptom**: Input method starts but no registration log

**Check**:
1. Is the log file being written? `ls -la /tmp/cosmic-comp-input-method-debug.log`
2. Is cosmic-comp running? `ps aux | grep cosmic-comp`
3. Is input method actually connecting? `ps aux | grep fcitx5`

**Try**:
```bash
# Clear log and restart
> /tmp/cosmic-comp-input-method-debug.log
killall fcitx5
fcitx5 &
tail -f /tmp/cosmic-comp-input-method-debug.log
```

### Problem: No "KEYBOARD LAYOUT CHANGED" when switching

**Symptom**: You switch layouts but see no log entry

**Possible causes**:
1. Layout change isn't actually happening (check COSMIC settings)
2. cosmic-comp config not updating
3. You're switching physical keyboards, not layouts

**Verify**:
```bash
# Check current layout
cosmic-comp-config get xkb_config.layout

# Force a layout change
cosmic-comp-config set xkb_config.layout "tw"

# Should see log entry immediately
```

### Problem: Input method registered but shows as inactive

**Symptom**: 
```
[timestamp] Registered input methods: 1 total
[timestamp]   [0] app_id='fcitx5' serial=0 active=false
```

**This is CORRECT if**:
- Current layout doesn't have a mapping
- Mapping file doesn't exist
- Layout name doesn't match mapping

**Check**:
1. Does mapping file exist?
   ```bash
   cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
   ```

2. What's the current layout?
   ```bash
   cosmic-comp-config get xkb_config.layout
   ```

3. Do they match?
   - If layout is `"no,us,tw"`, primary is `"no"`
   - Mapping file should have entry for `"no"`

## Quick Reference: Log Message Meanings

| Log Message | What It Means |
|-------------|---------------|
| `========== INPUT METHOD REGISTRATION ==========` | New input method is registering |
| `Successfully read from /proc/comm: 'fcitx5'` | Found input method name |
| `NEW INPUT METHOD REGISTERED: app_id='fcitx5'` | Registration complete, compositor notified |
| `Registered input methods: N total` | Shows all registered input methods |
| `app_id='fcitx5' serial=0 active=false` | Input method details: not active |
| `app_id='fcitx5' serial=0 active=true` | Input method details: currently active |
| `KEYBOARD LAYOUT CHANGED to 'XX'` | Layout switch detected |
| `Primary layout: 'XX'` | First layout from comma-separated list |
| `Found mapping: layout 'XX' -> app_id 'YY'` | Mapping exists, will activate |
| `No mapping found for layout 'XX'` | No mapping, will deactivate |
| `Successfully set 'YY' as active instance` | Input method activated |
| `activate_input_method() called` | Activation sequence starting |
| `Processing keyboard grab request from active` | Allowing keyboard grab |
| `Blocking keyboard grab from inactive` | Preventing keyboard grab (security) |
| `Inert keyboard object created` | Created fake object to prevent crash |
| `deactivate_input_method() START` | Deactivation sequence starting |
| `Keyboard grab released successfully` | Keyboard returned to compositor |

## Expected Behavior Summary

✅ **Input method starts**: Logs show registration with app_id
✅ **If no mapping**: Input method stays inactive, can't grab keyboard
✅ **If tries to grab**: Inert object created, no crash
✅ **Layout switch to mapped**: Input method activates
✅ **Keyboard grab works**: When active
✅ **Layout switch away**: Input method deactivates, grab released
✅ **All operations logged**: Every step visible in log file

## Testing Checklist

- [ ] Start input method → See registration logs with app_id
- [ ] Check registered list → See input method listed
- [ ] Switch to mapped layout → See activation logs
- [ ] Input method grabs keyboard → See "Processing keyboard grab"
- [ ] Switch to unmapped layout → See deactivation logs
- [ ] See "Keyboard grab released successfully"
- [ ] Input method tries to grab again → See "Blocking keyboard grab"
- [ ] See "Inert keyboard object created"
- [ ] No crashes throughout process

---

**All the detailed logging is now in place to help you debug input method behavior!**