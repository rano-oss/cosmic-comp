# IME Auto-Activation Based on Keyboard Layout - Debug Summary

## Problem Description

You're implementing a new feature to automatically activate Input Method Editors (IMEs) based on the current keyboard layout. The IMEs connect and work fine manually, but when `sync_input_method_with_layout()` is called, it reports "Registered input methods: 0 total" even though IMEs are connected and functional.

## Root Cause

The issue was in how `sync_input_method_with_layout()` was retrieving the `InputMethodHandle`:

```rust
// WRONG - returns None if handle was never created
let Some(input_method_handle) = seat.user_data().get::<InputMethodHandle>() else {
    return;
};
```

The `InputMethodHandle` is created lazily in several places:
- When an IME calls `GetInputMethod` request
- When a text input calls `GetTextInput` request  
- When the `InputMethodSeat::input_method()` trait method is called

If `sync_input_method_with_layout()` runs BEFORE any of these events (e.g., at compositor startup or before text inputs are created), the handle won't exist yet, and the function returns early with "No InputMethodHandle found".

## The Fix

Use the `InputMethodSeat` trait which ensures the handle is created if it doesn't exist:

```rust
use smithay::wayland::input_method::{InputMethodHandle, InputMethodSeat};

// CORRECT - creates handle if needed
let input_method_handle = seat.input_method();
```

**Changed in:** `cosmic-comp/src/wayland/handlers/input_method.rs`

## Additional Fixes

### 1. Missing [COSMIC] Log Prefix
The `log_to_file()` functions in cosmic-comp weren't including the `[COSMIC]` prefix.

**Fixed in:**
- `cosmic-comp/src/config/mod.rs` 
- `cosmic-comp/src/wayland/handlers/input_method.rs`

### 2. Enhanced Debug Logging
Added more detailed logging throughout to track:
- Seat name when syncing
- When InputMethodHandle is obtained
- When listing registered methods
- Step-by-step progression through the sync process

## How It Works Now

1. **Keyboard Layout Changes** → Triggers `sync_input_method_with_layout()`
2. **Get InputMethodHandle** → Uses `.input_method()` trait to ensure handle exists
3. **List Registered IMEs** → Queries all connected input method instances
4. **Check Mapping Config** → Loads `~/.config/cosmic/com.system76.CosmicComp/input_method_keyboard_map.ron`
5. **Match Layout** → Finds which IME should be active for current layout
6. **Activate IME** → Calls `set_active_instance()` and `activate_input_method()`

## Expected Log Flow

After rebuilding, you should see:

```
[COSMIC timestamp] KEYBOARD LAYOUT CHANGED to 'no,us,tw'
[COSMIC timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
[COSMIC timestamp] Seat name: seat-0
[COSMIC timestamp] InputMethodHandle obtained (created if needed)
[COSMIC timestamp] TextInputHandle found successfully
[COSMIC timestamp] About to call list_registered_input_methods...
[COSMIC timestamp] Registered input methods: 2 total
[COSMIC timestamp]   [0] app_id='fcitx5' serial=0 active=false
[COSMIC timestamp]   [1] app_id='ibus-daemon' serial=0 active=false
[COSMIC timestamp] Configured keyboard layouts: 'no,us,tw'
[COSMIC timestamp] Using primary layout: 'no'
[COSMIC timestamp] Found mapping: layout 'no' -> app_id 'fcitx5'
[COSMIC timestamp] Successfully set 'fcitx5' as active instance
[COSMIC timestamp] Found focused text input, activating input method on surface
```

## Configuration File Format

Create `~/.config/cosmic/com.system76.CosmicComp/input_method_keyboard_map.ron`:

```ron
{
    "tw": "fcitx5",      // Taiwanese layout uses fcitx5
    "jp": "ibus-daemon",  // Japanese layout uses ibus
    "kr": "fcitx5",      // Korean layout uses fcitx5
    // Add more mappings as needed
}
```

The key is the keyboard layout name from your XKB config, and the value is the app_id of the IME (which comes from `/proc/<pid>/comm` of the IME process).

## Testing Instructions

1. **Rebuild cosmic-comp:**
   ```bash
   cd cosmic-comp
   cargo build --release
   ```

2. **Install the new binary:**
   ```bash
   sudo cp target/release/cosmic-comp /usr/bin/cosmic-comp
   ```

3. **Clear old logs:**
   ```bash
   sudo rm /tmp/cosmic-comp-input-method-debug.log
   ```

4. **Restart COSMIC** (log out and log back in)

5. **Launch your IMEs** (fcitx5, ibus, etc.)

6. **Create the mapping config** file with your desired layout→IME mappings

7. **Switch keyboard layouts** using your keyboard shortcut

8. **Check the logs:**
   ```bash
   cat /tmp/cosmic-comp-input-method-debug.log
   ```

You should now see the registered IMEs and activation happening!

## Notes

- The mapping config file is optional - if it doesn't exist or doesn't have a mapping for the current layout, no IME will be activated
- The "primary layout" is the first layout in the comma-separated list (e.g., 'no' from 'no,us,tw')
- IMEs are identified by their process name (app_id), which is extracted from `/proc/<pid>/comm`
- Only the focused text input will have the IME activated - if no text input is focused, the IME is set as active but not bound to a surface yet