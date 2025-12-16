# Troubleshooting: Input Methods Not Showing Up

## Problem

You're not seeing:
1. Input method app_ids in the logs
2. "NEW INPUT METHOD REGISTERED" messages
3. Keyboard layout changes in the logs

## Step-by-Step Diagnosis

### Step 1: Verify cosmic-comp is Running with New Build

```bash
# Kill old cosmic-comp
killall cosmic-comp

# Verify it's gone
ps aux | grep cosmic-comp

# Clear the log
> /tmp/cosmic-comp-input-method-debug.log

# Start the new cosmic-comp (from your build directory)
cd /home/eivind/Public/code/cosmic-epoch/cosmic-comp
./target/release/cosmic-comp &

# Check log immediately
cat /tmp/cosmic-comp-input-method-debug.log
```

**Expected to see**:
```
[SMITHAY timestamp] ========== InputMethodManagerState::new() CALLED ==========
[SMITHAY timestamp] Initializing input method manager
[SMITHAY timestamp] Input method manager global created with id: GlobalId(...)
[SMITHAY timestamp] Input method manager initialized successfully
[SMITHAY timestamp] Waiting for input method clients to connect...
[SMITHAY timestamp] ========== END InputMethodManagerState::new() ==========
```

**If you DON'T see this**: cosmic-comp isn't using the new build or the log file isn't writable.

**Solutions**:
- Make sure you're running the newly built binary: `/home/eivind/Public/code/cosmic-epoch/cosmic-comp/target/release/cosmic-comp`
- Check log file permissions: `touch /tmp/cosmic-comp-input-method-debug.log && ls -la /tmp/cosmic-comp-input-method-debug.log`
- Run with explicit path: `./target/release/cosmic-comp` not just `cosmic-comp`

### Step 2: Check if Input Method Protocol is Advertised

```bash
# List Wayland globals
WAYLAND_DISPLAY=wayland-1 wayland-info | grep -i input_method
```

**Expected to see**:
```
interface: 'zwp_input_method_manager_v2', version: 1, name: XX
```

**If you DON'T see this**:
- Input method protocol not advertised by compositor
- Wrong WAYLAND_DISPLAY (check with `echo $WAYLAND_DISPLAY`)
- cosmic-comp not running or crashed

### Step 3: Start Input Method and Watch Logs

```bash
# Watch log in one terminal
tail -f /tmp/cosmic-comp-input-method-debug.log

# In another terminal, start your input method
fcitx5 &
# OR
ibus-daemon -drx
# OR
kime &
```

**Expected to see in log**:
```
[SMITHAY timestamp] Input method manager binding for client: ClientId(...)
[SMITHAY timestamp] Input method manager bound successfully
[SMITHAY timestamp] ========== GetInputMethod REQUEST ==========
[SMITHAY timestamp] Client ClientId(...) requesting input method
[SMITHAY timestamp] Seat: "seat-0"
[SMITHAY timestamp] Calling add_instance to register input method
[SMITHAY timestamp] ========== INPUT METHOD REGISTRATION ==========
[SMITHAY timestamp] Got client credentials: pid=12345
[SMITHAY timestamp] Attempting to get app_id for pid 12345
[SMITHAY timestamp] Successfully read from /proc/comm: 'fcitx5'
[SMITHAY timestamp] Registering input method instance with app_id: 'fcitx5'
[SMITHAY timestamp] Total input method instances now: 1
[SMITHAY timestamp] ========== END REGISTRATION ==========
[cosmic timestamp] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
```

**If you see NOTHING**:

#### Possibility 1: Input Method Not Connecting

Check if input method is running:
```bash
ps aux | grep fcitx5
# OR
ps aux | grep ibus
```

Check if input method sees the protocol:
```bash
# Set debug environment and restart input method
killall fcitx5
WAYLAND_DEBUG=1 fcitx5 2>&1 | grep -i input_method
```

Look for messages about `zwp_input_method_manager_v2` or `get_input_method`.

#### Possibility 2: Input Method Using Different Protocol

Some input methods use `text_input` protocol instead of `input_method` protocol.

Check which protocols your input method supports:
- **fcitx5**: Should support input-method-v2
- **ibus**: Might use text-input-v3
- **kime**: Should support input-method-v2

Try a different input method:
```bash
# Try fcitx5 specifically
sudo apt install fcitx5-frontend-wayland  # Debian/Ubuntu
fcitx5 &
```

#### Possibility 3: Permission/Privilege Issue

Input method protocol might require privileged clients.

Check cosmic-comp initialization:
```bash
grep "client_is_privileged" /home/eivind/Public/code/cosmic-epoch/cosmic-comp/src/state.rs
```

The line should be:
```rust
InputMethodManagerState::new::<Self, _>(dh, client_is_privileged);
```

If it's `|_| false`, input methods are blocked from connecting.

### Step 4: Test Keyboard Layout Changes

```bash
# In terminal with log watching
tail -f /tmp/cosmic-comp-input-method-debug.log

# In another terminal, change layout
cosmic-comp-config set xkb_config.layout "us"
# Wait 2 seconds
cosmic-comp-config set xkb_config.layout "jp"
# Wait 2 seconds
cosmic-comp-config set xkb_config.layout "tw"
```

**Expected to see**:
```
[cosmic timestamp] KEYBOARD LAYOUT CHANGED to 'us'
[cosmic timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
...
[cosmic timestamp] KEYBOARD LAYOUT CHANGED to 'jp'
[cosmic timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
...
[cosmic timestamp] KEYBOARD LAYOUT CHANGED to 'tw'
[cosmic timestamp] ========== SYNC INPUT METHOD WITH LAYOUT ==========
```

**If you see NOTHING**:

#### Check if Config Changes Work

```bash
# Check current layout
cosmic-comp-config get xkb_config.layout

# Change it
cosmic-comp-config set xkb_config.layout "de"

# Verify change
cosmic-comp-config get xkb_config.layout
```

If the value doesn't change, cosmic-config might not be working.

#### Check if cosmic-comp Detects Config Changes

Look for ANY config change logs:
```bash
grep "config_changed\|xkb_config" /tmp/cosmic-comp-input-method-debug.log
```

If nothing appears, the config watcher might not be active.

### Step 5: Check Multiple Layout Format

Your layout is "no,us,tw" (Norwegian, US, Traditional Chinese).

The system extracts the PRIMARY layout (first one):
```
"no,us,tw" → primary = "no"
```

**Test with single layout**:
```bash
# Set single layout
cosmic-comp-config set xkb_config.layout "tw"

# Should see
# KEYBOARD LAYOUT CHANGED to 'tw'
# Primary layout: 'tw'
```

**Test with multiple layouts**:
```bash
# Set multiple
cosmic-comp-config set xkb_config.layout "no,us,tw"

# Should see
# KEYBOARD LAYOUT CHANGED to 'no,us,tw'
# Primary layout: 'no'
```

Your mapping file should map the PRIMARY layout:
```ron
{
    "no": "none",     // First in "no,us,tw"
    "us": "none",     // If "us" is first
    "tw": "fcitx5",   // If "tw" is first
}
```

## Complete Diagnostic Script

Run this script to gather all information:

```bash
#!/bin/bash

echo "===== COSMIC-COMP INPUT METHOD DIAGNOSTICS ====="
echo ""

echo "1. Checking if cosmic-comp is running..."
ps aux | grep cosmic-comp | grep -v grep
echo ""

echo "2. Checking log file..."
if [ -f /tmp/cosmic-comp-input-method-debug.log ]; then
    echo "Log exists, size: $(wc -l /tmp/cosmic-comp-input-method-debug.log | awk '{print $1}') lines"
    echo "Last 10 lines:"
    tail -10 /tmp/cosmic-comp-input-method-debug.log
else
    echo "ERROR: Log file does not exist!"
fi
echo ""

echo "3. Checking for input method manager initialization..."
grep "InputMethodManagerState::new()" /tmp/cosmic-comp-input-method-debug.log
echo ""

echo "4. Checking Wayland display..."
echo "WAYLAND_DISPLAY=$WAYLAND_DISPLAY"
echo ""

echo "5. Checking for input method protocol..."
wayland-info 2>/dev/null | grep -i input_method || echo "wayland-info not found"
echo ""

echo "6. Checking running input methods..."
ps aux | grep -E "fcitx|ibus|kime" | grep -v grep
echo ""

echo "7. Checking current keyboard layout..."
cosmic-comp-config get xkb_config.layout 2>/dev/null || echo "cosmic-comp-config not found"
echo ""

echo "8. Checking mapping file..."
if [ -f ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map ]; then
    echo "Mapping file exists:"
    cat ~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
else
    echo "WARNING: No mapping file found"
fi
echo ""

echo "===== END DIAGNOSTICS ====="
```

Save as `diagnose.sh`, make executable (`chmod +x diagnose.sh`), and run.

## Common Issues and Solutions

### Issue: No log entries at all

**Solution**: cosmic-comp is not using the new build
```bash
killall cosmic-comp
cd /home/eivind/Public/code/cosmic-epoch/cosmic-comp
./target/release/cosmic-comp &
```

### Issue: Log shows initialization but no input methods

**Solution**: Input method not connecting or wrong protocol
```bash
# Try fcitx5 explicitly
killall fcitx5
WAYLAND_DEBUG=1 fcitx5 2>&1 | tee /tmp/fcitx5-debug.log &

# Check if it tries to bind input_method
grep input_method /tmp/fcitx5-debug.log
```

### Issue: Layout changes don't appear in log

**Solution**: Config change detection not working
```bash
# Restart cosmic-comp to reload config watcher
killall cosmic-comp
./target/release/cosmic-comp &

# Try changing layout again
cosmic-comp-config set xkb_config.layout "us"
```

### Issue: Multiple layouts confusing

**Solution**: Use single layout for testing
```bash
# Test with one layout at a time
cosmic-comp-config set xkb_config.layout "tw"

# Verify it works, then try multiple
cosmic-comp-config set xkb_config.layout "no,us,tw"
```

## What to Report

If still not working, collect and share:

1. **Full log output**:
   ```bash
   cat /tmp/cosmic-comp-input-method-debug.log > debug.log
   ```

2. **Diagnostic script output**:
   ```bash
   ./diagnose.sh > diagnostics.txt
   ```

3. **WAYLAND_DEBUG from input method**:
   ```bash
   WAYLAND_DEBUG=1 fcitx5 2>&1 > fcitx5-wayland.log
   ```

4. **cosmic-comp version**:
   ```bash
   ./target/release/cosmic-comp --version
   ```

5. **Steps you took** and exactly what you see (or don't see) in the logs

## Expected Working Flow

When everything works, you should see this sequence:

1. **Cosmic-comp starts**:
   ```
   [SMITHAY] InputMethodManagerState::new() CALLED
   [SMITHAY] Input method manager initialized successfully
   ```

2. **Input method starts**:
   ```
   [SMITHAY] Input method manager binding for client
   [SMITHAY] GetInputMethod REQUEST
   [SMITHAY] INPUT METHOD REGISTRATION
   [SMITHAY] Successfully read from /proc/comm: 'fcitx5'
   [cosmic] NEW INPUT METHOD REGISTERED: app_id='fcitx5'
   ```

3. **Layout changes**:
   ```
   [cosmic] KEYBOARD LAYOUT CHANGED to 'tw'
   [cosmic] Registered input methods: 1 total
   [cosmic]   [0] app_id='fcitx5' serial=0 active=false
   [cosmic] Found mapping: layout 'tw' -> app_id 'fcitx5'
   ```

If you're not seeing this flow, use the steps above to find where it's breaking.