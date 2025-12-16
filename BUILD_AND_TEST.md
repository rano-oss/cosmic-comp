# Build and Test Instructions - Input Method Security Changes

## Overview

This guide will help you build and test the input method security improvements in cosmic-comp and smithay.

## Prerequisites

```bash
# Ensure you have Rust toolchain installed
rustc --version
cargo --version

# Install build dependencies (Ubuntu/Debian)
sudo apt install build-essential libwayland-dev libxkbcommon-dev libudev-dev libinput-dev libgbm-dev libseat-dev

# Or for Fedora
sudo dnf install gcc wayland-devel libxkbcommon-devel systemd-devel libinput-devel mesa-libgbm-devel libseat-devel
```

## Build Steps

### 1. Build Smithay (Library)

```bash
cd /home/eivind/Public/code/smithay
cargo build --release
```

**Expected Result**: Successful build with 1 warning about deprecated method (unrelated to our changes)

### 2. Build cosmic-comp (Compositor)

```bash
cd /home/eivind/Public/code/cosmic-epoch/cosmic-comp
cargo build --release
```

**Expected Result**: Successful build, using the locally modified smithay

### 3. Verify Changes Were Applied

```bash
# Check that smithay has our security changes
grep -n "Ignoring keyboard grab request from inactive" \
    /home/eivind/Public/code/smithay/src/wayland/input_method/input_method_handle.rs

# Should output line numbers showing our security check
```

## Configuration Setup

### 1. Create Input Method Mapping File

```bash
# Create directory if it doesn't exist
mkdir -p ~/.config/cosmic/com.system76.CosmicComp/v1/

# Create the mapping file
cat > ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map << 'EOF'
{
    "us": "none",
    "jp": "fcitx5",
    "zh": "fcitx5",
    "ko": "kime",
}
EOF
```

**Note**: Adjust the mappings based on your installed input methods.

### 2. Verify Configuration

```bash
cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
```

## Testing

### Test 1: Clean Slate

```bash
# 1. Stop any running input methods
killall fcitx5 ibus-daemon kime 2>/dev/null

# 2. Clear the debug log
> /tmp/cosmic-comp-input-method-debug.log

# 3. Start cosmic-comp (if not already running)
# In a separate terminal or session:
cosmic-comp

# 4. Monitor the log
tail -f /tmp/cosmic-comp-input-method-debug.log
```

### Test 2: Input Method Registration

```bash
# Start your input method
fcitx5 &

# Check the log - should see:
# - "NEW INPUT METHOD REGISTERED: app_id='fcitx5'"
# - "Syncing input method for seat after registration"
```

**Expected**: Input method registers but doesn't activate (if current layout is 'us')

### Test 3: Activation via Layout Switch

```bash
# Switch keyboard layout to one with a mapping (e.g., 'jp')
# You can do this via COSMIC settings or:
cosmic-comp-config set xkb_config.layout "jp"

# Check the log - should see:
# - "KEYBOARD LAYOUT CHANGED to 'jp'"
# - "Found mapping: layout 'jp' -> app_id 'fcitx5'"
# - "Successfully set 'fcitx5' as active instance"
# - "activate_input_method() called"
```

**Expected**: Input method activates and can now grab keyboard

### Test 4: Keyboard Grab (Active State)

```bash
# 1. Open a text editor (e.g., cosmic-edit)
# 2. Click in a text field
# 3. Start typing

# Check the log - should see:
# - "Processing keyboard grab request from active input method"
# - "Keyboard grab successfully installed"
```

**Expected**: Input method can grab keyboard and intercept keys

### Test 5: Popup Display (Active State)

```bash
# With input method active and text field focused:
# 1. Type characters that trigger input method UI
# 2. For example, type Japanese hiragana

# Check the log - should see:
# - "Input method has a popup surface, updating parent"
# - "InputMethodHandler::new_popup() called"
# - "Successfully tracked input method popup"
```

**Expected**: Input method popup appears on screen

### Test 6: Security Check - Inactive Blocking

```bash
# Switch to a layout without mapping
cosmic-comp-config set xkb_config.layout "us"

# Check the log - should see:
# - "No mapping found for layout 'us' - deactivating input method"
# - "deactivate_input_method() START"
# - "Keyboard grab released successfully"
# - "deactivate_input_method() END"

# If input method tries to grab keyboard after this:
# - "Ignoring keyboard grab request from inactive input method"

# If input method tries to show popup after this:
# - "Ignoring popup request from inactive input method"
```

**Expected**: 
- Keyboard grab is released
- Popup is dismissed
- Input method cannot grab keyboard or show popups
- User can type normally

### Test 7: Multiple Layout Switches

```bash
# Rapidly switch between layouts
for i in {1..5}; do
    cosmic-comp-config set xkb_config.layout "us"
    sleep 1
    cosmic-comp-config set xkb_config.layout "jp"
    sleep 1
done

# Monitor log for proper activation/deactivation cycles
```

**Expected**: Clean activation/deactivation on each switch, no errors

## Verification Checklist

- [ ] Smithay builds without errors
- [ ] cosmic-comp builds without errors
- [ ] Configuration file created
- [ ] Input method registers successfully
- [ ] Input method activates on correct layout
- [ ] Active input method can grab keyboard
- [ ] Active input method can show popups
- [ ] Inactive input method CANNOT grab keyboard (security check)
- [ ] Inactive input method CANNOT show popups (security check)
- [ ] Layout switch properly activates/deactivates
- [ ] No keyboard grab stuck after deactivation
- [ ] No popup stuck after deactivation

## Troubleshooting

### Issue: Smithay build fails

```bash
# Ensure you're in the right directory
cd /home/eivind/Public/code/smithay
pwd

# Clean and rebuild
cargo clean
cargo build --release
```

### Issue: cosmic-comp doesn't use local smithay

Check `cosmic-comp/Cargo.toml`:
```toml
[dependencies]
smithay = { path = "../../smithay" }
```

Or check `cosmic-comp/.cargo/config.toml` for path overrides.

### Issue: Input method doesn't register

```bash
# Check input method is running
ps aux | grep fcitx5

# Check Wayland display
echo $WAYLAND_DISPLAY

# Try starting with debug output
WAYLAND_DEBUG=1 fcitx5 2>&1 | grep input_method
```

### Issue: No activation despite correct layout

```bash
# Verify mapping file syntax
cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map

# Check current layout
cosmic-comp-config get xkb_config.layout

# Check log for why activation failed
grep "No mapping\|not registered" /tmp/cosmic-comp-input-method-debug.log
```

### Issue: Log file not created

```bash
# Check permissions
touch /tmp/cosmic-comp-input-method-debug.log
ls -la /tmp/cosmic-comp-input-method-debug.log

# cosmic-comp might not be running with our changes
# Verify you're running the newly built version
which cosmic-comp
```

## Performance Testing

### Memory Leak Check

```bash
# Monitor cosmic-comp memory usage over time
watch -n 5 'ps aux | grep cosmic-comp | grep -v grep | awk "{print \$6}"'

# Repeatedly switch layouts for 10 minutes
# Memory should remain stable
```

### Activation Latency

```bash
# Measure time from layout switch to activation
time cosmic-comp-config set xkb_config.layout "jp"

# Check log timestamps for activation delay
grep "KEYBOARD LAYOUT CHANGED\|activate_input_method" /tmp/cosmic-comp-input-method-debug.log | tail -10
```

## Success Criteria

✅ **Security**: Inactive input methods cannot grab keyboard or show popups
✅ **Functionality**: Active input methods work normally
✅ **Stability**: No crashes or deadlocks during layout switching
✅ **Performance**: Activation latency < 100ms
✅ **Cleanup**: No resource leaks (keyboard grabs, popups)

## Debug Tips

### Enable Verbose Logging

All operations are already logged to `/tmp/cosmic-comp-input-method-debug.log`

### Analyze Log Patterns

```bash
# Count activations
grep -c "activate_input_method() called" /tmp/cosmic-comp-input-method-debug.log

# Count deactivations
grep -c "deactivate_input_method() START" /tmp/cosmic-comp-input-method-debug.log

# Find security blocks
grep "Ignoring.*request from inactive" /tmp/cosmic-comp-input-method-debug.log

# Check for errors
grep -i "error\|failed\|warning" /tmp/cosmic-comp-input-method-debug.log
```

### Compare Before/After

```bash
# Save log before a test
cp /tmp/cosmic-comp-input-method-debug.log before.log

# Run test

# Save log after test
cp /tmp/cosmic-comp-input-method-debug.log after.log

# Compare
diff before.log after.log
```

## Reporting Issues

If you encounter problems, collect:

1. **Build output**
   ```bash
   cargo build --release 2>&1 | tee build.log
   ```

2. **Debug log**
   ```bash
   cp /tmp/cosmic-comp-input-method-debug.log debug.log
   ```

3. **Configuration**
   ```bash
   cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map > config.txt
   ```

4. **System info**
   ```bash
   uname -a > sysinfo.txt
   cosmic-comp --version >> sysinfo.txt
   ```

5. **Steps to reproduce** (written description)

Then share these files when reporting the issue.