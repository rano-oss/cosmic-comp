# Input Method Security and Activation Control - Complete Guide

## What Changed?

This update reorganizes input method code to prevent security issues where inactive input methods could:
- **Grab the keyboard** and intercept all user input
- **Show popup windows** when they shouldn't be visible
- **Interfere with user typing** by competing for input focus

## The Solution

Input methods now operate under **activation-based access control**:
- ✅ Only **active** input methods can grab the keyboard
- ✅ Only **active** input methods can show popup surfaces
- ✅ Activation is controlled by **keyboard layout mapping**
- ✅ Deactivation automatically **releases all resources**
- ✅ **No crashes**: Inactive input methods receive inert protocol objects instead of being rejected

## Files Changed

### Smithay (Wayland Compositor Library)
- `smithay/src/wayland/input_method/input_method_handle.rs`
  - Added activation checks to `GrabKeyboard` request handler
  - Added activation checks to `GetInputPopupSurface` request handler
  - Enhanced logging throughout activation/deactivation flow
  - Improved resource cleanup in `deactivate_input_method()`

### cosmic-comp (COSMIC Desktop Compositor)
- `cosmic-comp/src/wayland/handlers/input_method.rs`
  - Added comprehensive logging to popup handlers
  - Already had activation logic via `sync_input_method_with_layout()`

## How It Works

### 1. Input Method Lifecycle

```
┌─────────────────┐
│ Input Method    │
│ Starts          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Registers with  │
│ Compositor      │ ← Input method connects but is INACTIVE
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Keyboard Layout │
│ Matches Mapping │ ← User switches to mapped layout (e.g., "jp")
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ ACTIVATED       │ ← Now can grab keyboard and show popups
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Layout Changes  │
│ to Unmapped     │ ← User switches to "us" (no mapping)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ DEACTIVATED     │ ← Resources released, grabs removed
└─────────────────┘
```

### 2. Security Enforcement

When an **inactive** input method tries to:

**Grab Keyboard**:
```
Input Method → GrabKeyboard request → Smithay
                                          ↓
                                   Check: Is active?
                                          ↓
                                      NO → Create inert keyboard object
                                          ↓
                                      Send keymap & repeat info
                                          ↓
                                      DON'T install actual grab
                                          ↓
                                   Log: "Blocking keyboard grab from
                                         inactive input method"
                                   Log: "Inert keyboard object created"
```

**Show Popup**:
```
Input Method → GetInputPopupSurface → Smithay
                                          ↓
                                   Check: Is active?
                                          ↓
                                      NO → Create inert popup object
                                          ↓
                                      DON'T track or display popup
                                          ↓
                                   Log: "Blocking popup from
                                         inactive input method"
```

**Why Create Inert Objects?**

The Wayland protocol requires that when a client requests an object, the compositor must create it. If we simply returned early without creating the object, the client would receive a protocol error and the compositor would crash. By creating "inert" (non-functional) objects, we:
- Satisfy the protocol requirements (no crashes)
- Block actual functionality (security maintained)
- Keep the input method client happy (thinks it has access)
- Log the security block for debugging

## Configuration

### Keyboard Layout Mapping

**File**: `~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`

**Format**: RON (Rusty Object Notation)

**Example**:
```ron
{
    "us": "none",
    "jp": "fcitx5",
    "zh": "fcitx5",
    "ko": "kime",
    "ru": "ibus",
}
```

### Layout → Input Method Rules

- **"us": "none"** - US layout uses no input method
- **"jp": "fcitx5"** - Japanese layout activates fcitx5
- **"zh": "fcitx5"** - Chinese layout activates fcitx5
- **"ko": "kime"** - Korean layout activates kime

### Configuration Behavior

| Scenario | Behavior |
|----------|----------|
| File doesn't exist | No input method will activate (safe default) |
| Layout not in map | Current input method is deactivated |
| Layout in map | Corresponding input method is activated |
| Input method not running | Activation waits until it starts |

## Quick Start

### 1. Build

```bash
# Build smithay
cd /home/eivind/Public/code/smithay
cargo build --release

# Build cosmic-comp
cd /home/eivind/Public/code/cosmic-epoch/cosmic-comp
cargo build --release
```

### 2. Configure

```bash
# Create config directory
mkdir -p ~/.config/cosmic/com.system76.CosmicComp/v1/

# Create mapping file (adjust for your input methods)
cat > ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map << 'EOF'
{
    "us": "none",
    "jp": "fcitx5",
    "zh": "fcitx5",
}
EOF
```

### 3. Test

```bash
# Clear debug log
> /tmp/cosmic-comp-input-method-debug.log

# Start cosmic-comp (or restart if already running)

# Start your input method
fcitx5 &

# Watch the log
tail -f /tmp/cosmic-comp-input-method-debug.log

# Switch keyboard layouts and observe activation/deactivation
```

## Debugging

### Log File Location
```
/tmp/cosmic-comp-input-method-debug.log
```

### Key Log Messages

**Input Method Registers**:
```
[timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
```

**Activation**:
```
[SMITHAY timestamp] activate_input_method() called
[SMITHAY timestamp] Processing keyboard grab request from active input method
```

**Security Block** (This is good - it means protection is working):
```
[SMITHAY timestamp] Blocking keyboard grab from inactive input method
[SMITHAY timestamp] Inert keyboard object created, actual grab blocked
[SMITHAY timestamp] Blocking popup from inactive input method
```

**Deactivation**:
```
[SMITHAY timestamp] deactivate_input_method() START
[SMITHAY timestamp] Keyboard grab released successfully
[SMITHAY timestamp] deactivate_input_method() END
```

### Common Issues

**Problem**: Input method registers but never activates
- **Check**: Does your mapping file exist and is it valid?
- **Check**: Does current layout match a mapping?
- **Solution**: Verify configuration and current layout

**Problem**: Can't type after switching layouts
- **Check**: Look for "Keyboard grab released successfully" in log
- **If missing**: Input method may not be properly deactivating
- **Solution**: Restart input method and cosmic-comp

**Problem**: Popup stays visible after layout switch
- **Check**: Look for "dismiss_popup" messages in log
- **If missing**: Popup may not be properly cleaned up
- **Solution**: Check that input method is being deactivated

## Security Properties

### What This Prevents

❌ **Keylogging**: Inactive input methods cannot grab keyboard to spy on typing
❌ **UI Spoofing**: Inactive input methods cannot show fake popup windows
❌ **Focus Stealing**: Inactive input methods cannot intercept keyboard focus
❌ **Resource Competition**: Multiple input methods cannot fight for keyboard access
❌ **Compositor Crashes**: Protocol-compliant handling prevents crashes when blocking access

### Guarantees

✅ **Single Active Instance**: Only one input method active per seat at a time
✅ **Explicit Activation**: Input methods only activate when layout matches
✅ **Automatic Cleanup**: Deactivation always releases grabs and dismisses popups
✅ **Fail-Safe Default**: Without configuration, no input method activates
✅ **Crash-Free**: Inert protocol objects maintain protocol compliance while blocking access

## Documentation Files

1. **INPUT_METHOD_CHANGES_README.md** (this file) - Overview and quick start
2. **INPUT_METHOD_SECURITY.md** - Detailed security architecture
3. **DEBUG_INPUT_METHOD.md** - Debugging quick reference
4. **BUILD_AND_TEST.md** - Comprehensive build and test guide
5. **CHANGES_SUMMARY.md** - Concise summary of changes

## Migration for Input Method Developers

### No Code Changes Required

Your input method will work the same, but:
- You may receive fewer protocol events when inactive
- Your requests may be ignored when you're not the active instance
- Test your input method with keyboard layout switching

### Best Practices

1. **Don't assume you're always active** - Check activation state
2. **Handle activation/deactivation gracefully** - Clean up resources
3. **Test with multiple layouts** - Ensure proper behavior when switching
4. **Respect protocol lifecycle** - Don't grab or show popups unnecessarily

## Migration for Compositor Integrators

### Required Steps

1. **Implement activation logic** based on keyboard layout or other criteria
2. **Call activation functions** when appropriate (see cosmic-comp example)
3. **Provide configuration** for users to map layouts to input methods
4. **Handle new_input_method callback** to check if newly registered input methods should activate

### Example Integration (cosmic-comp)

```rust
// In config change handler
fn config_changed(state: &mut State, key: &str) {
    if key == "xkb_config" {
        // Keyboard layout changed - sync input method
        for seat in seats {
            sync_input_method_with_layout(state, &seat);
        }
    }
}

// In input method handler
impl InputMethodHandler for State {
    fn new_input_method(&mut self, app_id: &str) {
        // New input method registered - check if it should activate
        for seat in seats {
            sync_input_method_with_layout(self, &seat);
        }
    }
}
```

## Testing Checklist

- [ ] Input method registers successfully
- [ ] Activation occurs on correct keyboard layout
- [ ] Active input method can grab keyboard
- [ ] Active input method can show popups
- [ ] Inactive input method CANNOT grab keyboard (security)
- [ ] Inactive input method CANNOT show popups (security)
- [ ] Keyboard grab released on deactivation
- [ ] Popup dismissed on deactivation
- [ ] Layout switching works smoothly
- [ ] No crashes or deadlocks
- [ ] No resource leaks

## Performance

Expected performance characteristics:
- **Activation latency**: < 100ms from layout change to activation
- **Memory usage**: No leaks, stable over time
- **CPU usage**: Negligible overhead for activation checks
- **User experience**: Seamless switching between layouts

## Future Enhancements

Potential improvements:
1. **Per-application input methods** - Different apps use different input methods
2. **Input method profiles** - Save/restore state per workspace
3. **Manual switching** - User can explicitly activate input methods
4. **Protocol errors** - Send errors to clients instead of silently ignoring

## Support

### Getting Help

1. Check debug log: `/tmp/cosmic-comp-input-method-debug.log`
2. Review configuration file syntax
3. Verify keyboard layout matches mapping
4. Consult DEBUG_INPUT_METHOD.md for troubleshooting

### Reporting Issues

Include:
- Debug log: `/tmp/cosmic-comp-input-method-debug.log`
- Configuration: `~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`
- Steps to reproduce
- Expected vs actual behavior

## Summary

These changes make input method handling more secure and predictable by:
- Preventing inactive input methods from accessing sensitive user input
- Ensuring clean resource management during activation/deactivation
- Providing clear configuration for layout-based activation
- Adding comprehensive logging for debugging

The system is backward compatible with existing input methods but adds important security guarantees that protect user privacy and system stability.