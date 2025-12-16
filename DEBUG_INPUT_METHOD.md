# Input Method Debugging Quick Reference

## Quick Start

### 1. Watch the Log
```bash
tail -f /tmp/cosmic-comp-input-method-debug.log
```

### 2. Clear the Log (optional)
```bash
> /tmp/cosmic-comp-input-method-debug.log
```

### 3. Test Scenarios

#### Scenario A: Input Method Registers
**Action**: Start your input method (e.g., `fcitx5`, `ibus-daemon`, `kime`)

**Expected Logs**:
```
[SMITHAY 1234567890] add_instance called with app_id: 'fcitx5'
[1234567890] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[1234567890] Syncing input method for seat after registration of 'fcitx5'
```

#### Scenario B: Layout Switch Activates Input Method
**Action**: Switch to a keyboard layout that has a mapping (e.g., `jp` -> `fcitx5`)

**Expected Logs**:
```
[1234567890] KEYBOARD LAYOUT CHANGED to 'jp'
[1234567890] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[1234567890] Current keyboard layout: 'jp'
[1234567890] Primary layout: 'jp'
[1234567890] Found mapping: layout 'jp' -> app_id 'fcitx5'
[1234567890] Successfully set 'fcitx5' as active instance
[SMITHAY 1234567890] activate_input_method() called
[SMITHAY 1234567890] Calling activate() on input method instance
```

#### Scenario C: Active Input Method Grabs Keyboard
**Action**: Focus a text input field after activation

**Expected Logs**:
```
[SMITHAY 1234567890] Processing keyboard grab request from active input method (id: ObjectId(...))
[SMITHAY 1234567890] Keyboard grab successfully installed
```

#### Scenario D: Active Input Method Shows Popup
**Action**: Type to trigger input method UI

**Expected Logs**:
```
[SMITHAY 1234567890] Input method has a popup surface, updating parent
[SMITHAY 1234567890] Adding popup with new parent
[1234567890] InputMethodHandler::new_popup() called - tracking input method popup
[1234567890] Successfully tracked input method popup
```

#### Scenario E: Inactive Input Method Blocked
**Action**: Switch to a layout without mapping (e.g., `us`)

**Expected Logs**:
```
[1234567890] KEYBOARD LAYOUT CHANGED to 'us'
[1234567890] No mapping found for layout 'us' - deactivating input method
[SMITHAY 1234567890] ========== deactivate_input_method() START ==========
[SMITHAY 1234567890] Calling deactivate() on input method instance
[SMITHAY 1234567890] Keyboard grab released successfully
[SMITHAY 1234567890] ========== deactivate_input_method() END ==========
```

**If input method tries to grab after deactivation**:
```
[SMITHAY 1234567890] Blocking keyboard grab from inactive input method (id: ObjectId(...)) - creating inert object
[SMITHAY 1234567890] Inert keyboard object created, actual grab blocked
```

**If input method tries to show popup after deactivation**:
```
[SMITHAY 1234567890] Blocking popup from inactive input method (id: ObjectId(...)) - creating inert object
```

#### Scenario F: Input Method Starts on Wrong Layout
**Action**: Start input method when layout doesn't have mapping (e.g., on `us` layout)

**Expected Logs**:
```
[1234567890] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[1234567890] Syncing input method for seat after registration of 'fcitx5'
[1234567890] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[1234567890] No mapping found for layout 'us' - deactivating input method
```

**If input method tries to grab keyboard immediately**:
```
[SMITHAY 1234567890] Blocking keyboard grab from inactive input method (id: ObjectId(...)) - creating inert object
[SMITHAY 1234567890] Inert keyboard object created, actual grab blocked
```

**Result**: 
- Input method runs but cannot grab keyboard or show popups
- Compositor does NOT crash (this is correct behavior)
- User can type normally
- Input method will activate when switching to mapped layout

## Common Issues

### Issue 1: No Input Method Activates
**Symptom**: Input method registers but never activates

**Check**:
1. Does mapping file exist?
   ```bash
   cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
   ```

2. Does current layout have a mapping?
   ```bash
   # Check log for:
   # "No input method mapping configured"
   # OR
   # "No mapping found for layout 'XX'"
   ```

**Solution**: Create/update mapping file with correct layout

### Issue 2: Keyboard Grabs Not Released
**Symptom**: Can't type after switching layouts

**Check Logs**:
```bash
# Look for:
grep "keyboard grab" /tmp/cosmic-comp-input-method-debug.log
```

**Expected**: Should see "Keyboard grab released successfully" after deactivation

**If missing**: Input method may not be properly deactivating

### Issue 3: Popup Won't Dismiss
**Symptom**: Input method popup stays visible

**Check Logs**:
```bash
# Look for:
grep "dismiss" /tmp/cosmic-comp-input-method-debug.log
```

**Expected**: Should see "Dismissing old popup" or "calling dismiss_popup"

### Issue 4: Input Method App ID Unknown
**Symptom**: Logs show "app_id='unknown-XXXX'"

**Cause**: Input method client doesn't set proper app_id

**Solution**: Configure mapping using the detected PID-based name, or contact input method developer

## Log Message Reference

### Smithay Messages (prefixed with `[SMITHAY timestamp]`)

| Message | Meaning |
|---------|---------|
| `activate_input_method() called` | Activation sequence started |
| `Calling activate() on input method instance` | Sending activation to input method |
| `Processing keyboard grab request from active input method` | Allowing keyboard grab |
| `Blocking keyboard grab from inactive input method` | Blocking keyboard grab (SECURITY) |
| `Inert keyboard object created, actual grab blocked` | Created non-functional grab object to prevent crash |
| `Blocking popup from inactive input method` | Blocking popup (SECURITY) |
| `deactivate_input_method() START` | Deactivation sequence started |
| `Keyboard grab released successfully` | Grab cleanup completed |
| `deactivate_input_method() END` | Deactivation complete |

### cosmic-comp Messages (prefixed with `[timestamp]`)

| Message | Meaning |
|---------|---------|
| `NEW INPUT METHOD REGISTERED: app_id='...'` | Input method connected |
| `KEYBOARD LAYOUT CHANGED to '...'` | Layout switch detected |
| `Found mapping: layout '...' -> app_id '...'` | Activation will occur |
| `No mapping found for layout '...'` | No input method will activate |
| `Successfully set '...' as active instance` | Input method is now active |
| `Found focused text input, activating input method` | Activating on text field |
| `InputMethodHandler::new_popup() called` | Popup being created |
| `Successfully tracked input method popup` | Popup registered in compositor |

## Configuration

### Mapping File Location
```
~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
```

### Example Configuration
```ron
{
    "us": "none",
    "jp": "fcitx5",
    "zh": "fcitx5",
    "ko": "kime",
    "ru": "ibus",
}
```

### Reload Configuration
```bash
# Configuration is reloaded on cosmic-comp restart
# OR when the config file is modified (cosmic-config watches it)
```

## Useful Commands

### Find Input Method Process
```bash
ps aux | grep -E "fcitx|ibus|kime|chewing"
```

### Kill and Restart Input Method
```bash
killall fcitx5
fcitx5 &
```

### Monitor Wayland Protocol
```bash
# Set environment variable before starting cosmic-comp
WAYLAND_DEBUG=1 cosmic-comp 2>&1 | grep input_method
```

### Check Current Keyboard Layout
```bash
# The layout cosmic-comp sees is from its config
cosmic-comp-config get xkb_config
```

## Emergency Reset

### Clear All Input Method State
```bash
# 1. Kill all input methods
killall fcitx5 ibus-daemon kime

# 2. Restart cosmic-comp
systemctl --user restart cosmic-comp

# 3. Verify clean state
tail /tmp/cosmic-comp-input-method-debug.log
```

### Remove Configuration
```bash
# Backup first
cp ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map \
   ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map.backup

# Remove to disable all input methods
rm ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
```

## Contact

If you see unexpected behavior:
1. Capture full log: `cat /tmp/cosmic-comp-input-method-debug.log > input-method-debug.txt`
2. Note your configuration
3. Describe the steps to reproduce
4. Report with log file attached