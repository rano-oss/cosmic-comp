# Input Method Security and Activation Control

## Overview

This document describes the security improvements made to input method handling in cosmic-comp and smithay to prevent inactive input methods from intercepting user input or showing unwanted UI elements.

## Problem Statement

Previously, input methods could:
1. **Grab the keyboard** even when not active, intercepting all keyboard input
2. **Show popup windows** even when not active, displaying UI elements inappropriately
3. These behaviors occurred because the Wayland protocol handlers didn't check activation state before processing requests

This created security and usability issues where:
- An inactive input method could steal keyboard focus
- Multiple input methods could show conflicting UI elements
- Users couldn't type because inactive input methods were grabbing input

## Solution

### Activation-Based Access Control

Input methods now **must be active** to:
- Grab the keyboard via `zwp_input_method_v2.grab_keyboard`
- Show popup surfaces via `zwp_input_method_v2.get_input_popup_surface`

### Implementation Details

#### 1. Keyboard Grab Protection (smithay)

**Location**: `smithay/src/wayland/input_method/input_method_handle.rs`

The `GrabKeyboard` request handler now checks if the requesting input method is active:

```rust
zwp_input_method_v2::Request::GrabKeyboard { keyboard } => {
    let input_method = data.handle.inner.lock().unwrap();
    let requesting_id = seat.id();
    let is_active = input_method.active_input_method_id.as_ref() == Some(&requesting_id);

    if !is_active {
        log_to_file(&format!(
            "Ignoring keyboard grab request from inactive input method (id: {:?})",
            requesting_id
        ));
        drop(input_method);
        return;  // Inactive input methods cannot grab keyboard
    }

    // ... proceed with keyboard grab setup
}
```

**Behavior**:
- Active input method: Keyboard grab is installed normally
- Inactive input method: Request is silently ignored, no grab occurs

#### 2. Popup Protection (smithay)

**Location**: `smithay/src/wayland/input_method/input_method_handle.rs`

The `GetInputPopupSurface` request handler now checks activation state:

```rust
zwp_input_method_v2::Request::GetInputPopupSurface { id, surface } => {
    // ... role checking ...

    let mut input_method = data.handle.inner.lock().unwrap();
    let requesting_id = seat.id();
    let is_active = input_method.active_input_method_id.as_ref() == Some(&requesting_id);

    if !is_active {
        log_to_file(&format!(
            "Ignoring popup request from inactive input method (id: {:?})",
            requesting_id
        ));
        drop(input_method);
        return;  // Inactive input methods cannot show popups
    }

    // ... proceed with popup creation
}
```

**Behavior**:
- Active input method: Popup surface is created and tracked normally
- Inactive input method: Request is silently ignored, no popup appears

#### 3. Cleanup on Deactivation

When an input method is deactivated, all its resources are cleaned up:

```rust
pub fn deactivate_input_method<D: SeatHandler + 'static>(&self, state: &mut D) {
    // 1. Call deactivate() on the input method protocol object
    instance.object.deactivate();
    instance.done();

    // 2. Dismiss any existing popup
    if let Some(popup) = im.popup_handle.surface.as_mut() {
        if popup.get_parent().is_some() {
            (data.dismiss_popup)(state, popup.clone());
        }
        popup.set_parent(None);
    }

    // 3. Release keyboard grab
    if let Some(grab_object) = keyboard_grab.grab.take() {
        data.keyboard_handle.unset_grab(state);
    }
}
```

## Activation Flow

### 1. Input Method Registration

When an input method client connects:

```
1. Client creates zwp_input_method_v2 object
2. Smithay stores it as an "instance" (not yet active)
3. cosmic-comp's new_input_method() callback is invoked
4. cosmic-comp checks if the layout matches this input method
5. If matched, activate it; otherwise, leave it inactive
```

### 2. Layout Change Triggers Activation

When the keyboard layout changes:

```
1. Config system detects xkb_config change
2. Calls sync_input_method_with_layout()
3. Loads InputMethodKeyboardMap from config file
4. Checks if current layout has a mapping
5. If mapped:
   - set_active_instance(app_id)
   - activate_input_method(surface) if text input has focus
6. If not mapped:
   - deactivate_input_method()
```

### 3. Activation Requirements

For an input method to become active, ALL must be true:
- ✅ Input method is registered (zwp_input_method_v2 object exists)
- ✅ Current keyboard layout matches the input method in config
- ✅ set_active_instance() has been called with the correct app_id

Only then can the input method:
- Receive activate() events
- Grab the keyboard
- Show popup surfaces

## Configuration

### Input Method Keyboard Map

**Location**: `~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`

**Format**: RON (Rusty Object Notation)

**Example**:
```ron
{
    "us": "none",
    "jp": "fcitx5",
    "zh": "fcitx5",
    "ko": "kime",
}
```

**Behavior**:
- If file doesn't exist: No input method will be activated
- If layout not in map: Current input method is deactivated
- If layout in map: Corresponding input method is activated

## Debugging

### Log File

All input method operations are logged to:
```
/tmp/cosmic-comp-input-method-debug.log
```

### Log Messages

Key events that are logged:
- Input method registration with app_id
- Activation/deactivation calls
- Keyboard grab installation/removal
- Popup creation/dismissal
- Access denial for inactive input methods
- Layout changes and mapping lookups

### Example Log Sequence

```
[1234567890] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[1234567890] Syncing input method for seat after registration of 'fcitx5'
[1234567890] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[1234567890] Current keyboard layout: 'jp'
[1234567890] Found mapping: layout 'jp' -> app_id 'fcitx5'
[1234567890] Successfully set 'fcitx5' as active instance
[1234567890] Found focused text input, activating input method on surface
[1234567890] activate_input_method() called
[1234567890] Active input method id: ObjectId(...)
[1234567890] Calling activate() on input method instance
[1234567890] Processing keyboard grab request from active input method
[1234567890] Keyboard grab successfully installed
```

## Security Properties

### Guaranteed Invariants

1. **No unauthorized keyboard access**: Only the active input method can grab the keyboard
2. **No unauthorized UI**: Only the active input method can show popups
3. **Clean state transitions**: Deactivation always releases grabs and dismisses popups
4. **Single active instance**: Only one input method can be active per seat at a time

### Attack Prevention

This design prevents:
- **Keylogging**: Inactive input methods cannot grab keyboard to spy on input
- **UI spoofing**: Inactive input methods cannot show misleading popup windows
- **Resource exhaustion**: Multiple input methods cannot compete for resources
- **Focus stealing**: Inactive input methods cannot intercept keyboard focus

## Testing

### Manual Testing

1. **Test inactive input method blocking**:
   ```bash
   # Start an input method without a matching layout
   # Verify it cannot show popups or grab keyboard
   ```

2. **Test activation on layout switch**:
   ```bash
   # Switch to a layout with mapping
   # Verify correct input method activates
   # Verify it can now grab keyboard and show popups
   ```

3. **Test cleanup on deactivation**:
   ```bash
   # Switch to a layout without mapping
   # Verify keyboard grab is released
   # Verify popups are dismissed
   ```

### Check Debug Logs

```bash
tail -f /tmp/cosmic-comp-input-method-debug.log
```

Look for:
- ✅ "Ignoring keyboard grab request from inactive input method"
- ✅ "Ignoring popup request from inactive input method"
- ✅ "Keyboard grab released successfully"
- ✅ "Successfully tracked input method popup"

## Migration Guide

### For Input Method Developers

**No changes required** - Input methods work the same, but:
- Your requests may be ignored if you're not active
- Don't assume your input method is always active
- Test with keyboard layout switching

### For Compositor Integrators

**Required changes**:
1. Implement activation logic based on keyboard layout (see cosmic-comp example)
2. Call `sync_input_method_with_layout()` when layout changes
3. Provide configuration for layout-to-input-method mapping

## Related Files

### Smithay Changes
- `smithay/src/wayland/input_method/input_method_handle.rs`
  - Added activation checks to `GrabKeyboard` handler
  - Added activation checks to `GetInputPopupSurface` handler
  - Enhanced logging in `activate_input_method()`
  - Enhanced logging in `deactivate_input_method()`

### cosmic-comp Changes
- `cosmic-comp/src/wayland/handlers/input_method.rs`
  - Implements `InputMethodHandler` trait
  - Provides `sync_input_method_with_layout()` function
  - Loads `InputMethodKeyboardMap` configuration
  - Enhanced logging in popup handlers

- `cosmic-comp/src/config/mod.rs`
  - Calls `sync_input_method_with_layout()` on layout change

## Future Enhancements

Possible improvements:
1. **Per-application input methods**: Allow different apps to use different input methods
2. **Input method profiles**: Save/restore input method state per workspace
3. **Explicit activation requests**: Allow users to manually switch input methods
4. **Protocol error reporting**: Send protocol errors for invalid requests instead of silently ignoring

## References

- [Wayland input-method-unstable-v2 protocol](https://wayland.app/protocols/input-method-unstable-v2)
- [Smithay input method implementation](https://github.com/Smithay/smithay/tree/master/src/wayland/input_method)
- [cosmic-comp input method configuration](https://github.com/pop-os/cosmic-comp)