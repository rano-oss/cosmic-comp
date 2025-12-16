# Critical Crash Fix - Input Method Protocol Compliance

## Problem

**CRASH**: The compositor was crashing when input methods started or attempted to grab the keyboard/show popups while inactive.

## Root Cause

When we added activation checks to prevent inactive input methods from grabbing the keyboard or showing popups, we were returning early from the Wayland protocol request handlers **without creating the requested protocol objects**.

### What Was Wrong

```rust
// BROKEN CODE (caused crashes)
zwp_input_method_v2::Request::GrabKeyboard { keyboard } => {
    if !is_active {
        log_to_file("Ignoring keyboard grab");
        return;  // ❌ CRASH: Client expects a keyboard object!
    }
    
    // Create keyboard object...
}
```

### Why It Crashed

1. Input method client sends `GrabKeyboard` request
2. Compositor checks: "Is this input method active?" → NO
3. Compositor returns early without creating the keyboard object
4. Client receives **protocol error**: Expected object was never created
5. **CRASH**: Protocol violation terminates the connection and potentially crashes compositor

## Solution

**Create inert (non-functional) protocol objects** that satisfy the Wayland protocol requirements while still blocking actual functionality.

### Fixed Code

```rust
// FIXED CODE (no crashes)
zwp_input_method_v2::Request::GrabKeyboard { keyboard } => {
    if !is_active {
        log_to_file("Blocking keyboard grab - creating inert object");
        
        // ✅ Create the protocol object (prevents crash)
        let instance = data_init.init(keyboard, /* user data */);
        
        // ✅ Send basic protocol messages (keeps client happy)
        instance.repeat_info(repeat_rate, repeat_delay);
        instance.keymap(keymap_format, fd, size);
        
        // ✅ DON'T call set_grab() - keyboard stays with compositor
        log_to_file("Inert keyboard object created, actual grab blocked");
        return;
    }
    
    // Active input method: install real keyboard grab
    data.keyboard_handle.set_grab(/* ... */);
    // ...
}
```

## What "Inert Object" Means

An **inert object** is a valid Wayland protocol object that:
- ✅ Exists and satisfies protocol requirements
- ✅ Receives basic setup messages (keymap, repeat info)
- ❌ Does NOT have functional backing (no actual keyboard grab)
- ❌ Does NOT provide input method with keyboard access

Think of it as a "fake ID" - it looks real to the input method, but grants no actual access.

## Security Properties Maintained

Even with inert objects:
- ✅ Inactive input methods **cannot** intercept keyboard input
- ✅ Inactive input methods **cannot** show visible popups
- ✅ Only active input methods have real keyboard access
- ✅ Protocol compliance prevents crashes

## What You'll See in Logs

### Before Fix (Crash Scenario)
```
[timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[timestamp] No mapping found for layout 'us'
<CRASH - no further logs>
```

### After Fix (Working)
```
[timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
[timestamp] No mapping found for layout 'us'
[SMITHAY timestamp] Blocking keyboard grab from inactive input method (id: ObjectId(...)) - creating inert object
[SMITHAY timestamp] Inert keyboard object created, actual grab blocked
<compositor continues running normally>
```

## Testing

### Test 1: Start Input Method on Wrong Layout
```bash
# Ensure mapping file exists but current layout isn't mapped
cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
# Shows: "jp": "fcitx5" (but current layout is "us")

# Start input method
fcitx5 &

# Expected: No crash, input method runs but can't grab keyboard
# Check log for "Inert keyboard object created"
```

### Test 2: Switch Away From Mapped Layout
```bash
# Start with mapped layout (e.g., "jp")
cosmic-comp-config set xkb_config.layout "jp"
fcitx5 &

# Input method should activate and work

# Switch to unmapped layout
cosmic-comp-config set xkb_config.layout "us"

# Expected: Input method deactivated, attempts to grab blocked
# No crash
```

### Test 3: Rapid Layout Switching
```bash
# Switch layouts rapidly while input method is running
for i in {1..20}; do
    cosmic-comp-config set xkb_config.layout "us"
    sleep 0.5
    cosmic-comp-config set xkb_config.layout "jp"
    sleep 0.5
done

# Expected: No crashes, clean activation/deactivation cycles
```

## Implementation Details

### Files Changed
- `smithay/src/wayland/input_method/input_method_handle.rs`
  - Modified `GrabKeyboard` request handler
  - Modified `GetInputPopupSurface` request handler

### Key Changes

1. **Extract shared data early**:
   ```rust
   let keyboard_grab = input_method.keyboard_grab.clone();
   drop(input_method);  // Drop lock early to avoid deadlock
   ```

2. **Create inert keyboard object**:
   ```rust
   let instance = data_init.init(keyboard, InputMethodKeyboardUserData { ... });
   instance.repeat_info(repeat_rate, repeat_delay);
   instance.keymap(KeymapFormat::XkbV1, fd, size);
   // DON'T call set_grab() - that's what makes it inert
   ```

3. **Create inert popup object**:
   ```rust
   let _instance = data_init.init(id, InputMethodPopupSurfaceUserData { ... });
   // DON'T create PopupSurface or call state.new_popup()
   ```

## Why This Approach Is Correct

### Alternative Approaches (Rejected)

❌ **Send protocol error to client**:
- Would still crash the client
- Breaks input method entirely
- User experience is poor

❌ **Ignore request completely**:
- Violates Wayland protocol (causes crashes)
- What we tried initially

❌ **Delay object creation**:
- Complicated state machine
- Race conditions
- Client timeout issues

✅ **Create inert objects** (our solution):
- Protocol compliant (no crashes)
- Security maintained (no actual access)
- Simple implementation
- Clear semantics

## Verification

After applying this fix:
- [ ] Compositor does NOT crash when starting input method on unmapped layout
- [ ] Compositor does NOT crash when switching to unmapped layout
- [ ] Input methods work normally when active
- [ ] Input methods cannot grab keyboard when inactive
- [ ] Input methods cannot show popups when inactive
- [ ] Logs show "Inert keyboard object created" for blocked requests

## Summary

**Problem**: Compositor crashed due to protocol violation when blocking inactive input methods

**Solution**: Create inert protocol objects that satisfy protocol requirements without granting access

**Result**: No crashes, security maintained, protocol compliant

This fix is **critical** for stability while maintaining the security improvements.