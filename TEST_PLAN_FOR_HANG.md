# Test Plan: Finding the 10-Second Hang with IME

## Prerequisites

- [ ] IME (chewingwl or sctk_input_meth) is installed
- [ ] IME keyboard map is configured (`~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map`)
- [ ] Instrumented cosmic-comp is built: `cargo build --release`

## Test Procedure

### Step 1: Start Instrumented Cosmic-Comp

```bash
cd cosmic-comp
RUST_LOG="cosmic_comp=info,smithay=info,warn" ./target/release/cosmic-comp 2>&1 | tee test_$(date +%Y%m%d_%H%M%S).log
```

### Step 2: Start IME

Make sure the IME is actually **running**, not just configured:

```bash
# Check if IME is running
ps aux | grep -E "chewingwl|sctk_input_meth"

# If not running, start it (method depends on your setup)
# Usually it starts automatically when you focus a text input
```

### Step 3: Trigger the Hang

1. Click on any text input field to activate the IME
2. Click on a panel popup (volume control, network settings, etc.)
3. **Observe**: The compositor should freeze for ~10 seconds

### Step 4: Check the Logs

After the freeze recovers, search the log file for:

```bash
# Look for the warning messages
grep -E "dispatch_clients took|flush_clients took" test_*.log

# Look for time gaps
grep "Shell::set_focus - EXIT" test_*.log | tail -5
grep "Shell::set_focus - ENTRY" test_*.log | tail -5
```

## Expected Results

You should see ONE of these warnings appearing at the time of the freeze:

### Result A: dispatch_clients is slow
```
WARN Wayland dispatch_clients took 10.XXXs - possible hang or slow client
```
**Meaning**: The hang is in processing incoming Wayland messages from clients.

### Result B: flush_clients is slow
```
WARN flush_clients took 10.XXXs - possible hang or slow client response
```
**Meaning**: The hang is in sending messages to clients and waiting for acknowledgment.

### Result C: Neither appears
**Meaning**: The hang is happening outside these two operations - possibly in rendering, backend, or another event source.

## Quick Verification

To quickly verify the hang is related to the event loop, check the timestamps:

```bash
# Get timestamps around a hang
awk '/Shell::set_focus - EXIT/ {prev=$1" "$2} /Shell::set_focus - ENTRY/ {curr=$1" "$2; if(prev) print "Gap between:", prev, "and", curr; prev=""}' test_*.log | tail -10
```

Look for gaps of ~10 seconds.

## Cleanup

After testing, you can stop cosmic-comp with Ctrl+C or by switching to a TTY and killing the process.

## Reporting Results

When reporting, include:

1. Which warning appeared (A, B, or C above)
2. The exact duration from the warning message
3. The last 50 lines before the warning:
   ```bash
   grep -B50 "dispatch_clients took\|flush_clients took" test_*.log
   ```
4. Whether the IME was actively typing or just running in background
5. Which popup was clicked (volume, network, etc.)

## Troubleshooting

**If no hang occurs:**
- Ensure IME is actually running (check `ps aux | grep chewing`)
- Ensure IME is activated (click in a text field first)
- Try different panel popups
- Check that input_method_keyboard_map exists

**If hang occurs but no warnings appear:**
- The 100ms threshold might be too high. Rebuild with lower threshold (10ms):
  Edit `cosmic-comp/src/lib.rs` and change `> 100` to `> 10`
- The hang might be in a different part of the event loop

## Success Criteria

Test is successful when we can confirm:
- [x] The hang is reproducible
- [x] We see a warning message indicating where the hang occurs
- [x] The duration matches the observed freeze (~10 seconds)