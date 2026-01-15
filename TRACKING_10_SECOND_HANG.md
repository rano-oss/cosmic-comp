# Tracking the 10-Second Hang When IME is Running

## Problem Summary

When an Input Method Editor (IME) like chewingwl is **running** (not just configured), clicking on panel popups causes a ~10-second freeze/hang in the compositor. This is NOT a deadlock - the system recovers after exactly ~10 seconds.

## Key Findings from Log Analysis

1. **All instrumented code works perfectly** - All locks are acquired and released correctly
2. **The hang is NOT in cosmic-comp or smithay code** - It's in the Wayland protocol layer
3. **The hang is exactly ~10 seconds** - This suggests a timeout, not a deadlock
4. **Pattern**: The hang occurs between completion of one operation and the start of the next
5. **Only happens when IME is actively running** - Not just when configured

### Evidence from Logs

```
2026-01-14T17:38:56.569908Z  INFO Shell::set_focus - EXIT
[10.35 SECOND GAP - NO LOGS]
2026-01-14T17:39:06.918983Z  INFO Shell::set_focus - ENTRY
```

Multiple such gaps were found:
- 10 seconds at 17:43:03 → 17:43:13
- 10 seconds at 17:43:16 → 17:43:26
- 10 seconds at line 7027

## Root Cause Hypothesis

The 10-second hang is likely caused by:

1. **Wayland protocol blocking** - The compositor is waiting for a client (IME or panel) to respond
2. **Message timeout** - A Wayland message is sent to the IME client, but the client doesn't respond within 10 seconds
3. **Synchronous IPC** - Something is doing synchronous communication with the IME process

## New Instrumentation Added

### 1. Timing Measurement in Event Loop Dispatch

**File**: `cosmic-comp/src/lib.rs`

**Location**: Wayland event loop dispatch handler (around line 275)

**What it does**: Times how long `dispatch_clients()` takes and logs a warning if it exceeds 100ms.

```rust
let start = std::time::Instant::now();
let result = unsafe { display.get_mut().dispatch_clients(state) };
let elapsed = start.elapsed();

if elapsed.as_millis() > 100 {
    tracing::warn!(
        "Wayland dispatch_clients took {:?} - possible hang or slow client",
        elapsed
    );
}
```

### 2. Timing Measurement in Client Flushing

**File**: `cosmic-comp/src/lib.rs`

**Location**: Main event loop where clients are flushed (around line 194)

**What it does**: Times how long `flush_clients()` takes and logs a warning if it exceeds 100ms.

```rust
let flush_start = std::time::Instant::now();
let _ = state.common.display_handle.flush_clients();
let flush_elapsed = flush_start.elapsed();

if flush_elapsed.as_millis() > 100 {
    tracing::warn!(
        "flush_clients took {:?} - possible hang or slow client response",
        flush_elapsed
    );
}
```

## How to Test

1. **Build the instrumented version**:
   ```bash
   cd cosmic-comp
   cargo build --release
   ```

2. **Run with logging**:
   ```bash
   RUST_LOG="cosmic_comp=info,smithay=info,warn" ./target/release/cosmic-comp 2>&1 | tee cosmic_hang_debug.log
   ```

3. **Reproduce the hang**:
   - Ensure IME (chewingwl) is **running** (not just configured)
   - Click on a panel popup (volume, network, etc.)
   - Wait for the 10-second freeze

4. **Check the logs**:
   Look for these warning messages:
   ```
   WARN Wayland dispatch_clients took 10.XXXs - possible hang or slow client
   ```
   OR
   ```
   WARN flush_clients took 10.XXXs - possible hang or slow client response
   ```

## What the Logs Will Tell Us

### If dispatch_clients is slow:
The hang is in **processing incoming messages from clients**. The IME or panel client sent a message that triggered a slow operation in the compositor.

### If flush_clients is slow:
The hang is in **sending messages to clients**. The compositor sent a message to the IME client and is waiting for the client to acknowledge/respond, but the client is blocked or slow.

### If neither shows up:
The hang is happening elsewhere - possibly in:
- Graphics/rendering pipeline
- Backend operations (DRM, input devices)
- Another event source in the event loop

## Expected Result

Based on the symptoms (only happens with IME running, exact 10-second timeout), we expect to see:

```
WARN flush_clients took 10.XXXs - possible hang or slow client response
```

This would indicate the compositor is waiting for the IME client to process a message, and hitting a timeout.

## Next Steps After Confirmation

Once we identify which operation is slow:

1. **If it's dispatch_clients**: Add more granular logging inside the dispatch handler to track which client and which message type is slow

2. **If it's flush_clients**: Check what messages were sent to clients before the hang. Likely candidates:
   - Popup configuration messages
   - Keyboard grab notifications
   - Focus change notifications to IME

3. **Investigate the IME client (chewingwl)**: The client might be:
   - Doing expensive computation on the main thread
   - Waiting for I/O (file, network, DBus)
   - Deadlocked internally
   - Missing a message handler

## Workaround

Until this is fixed, users can:

1. **Disable IME when not needed**: Stop the IME process (chewingwl) when not actively typing
2. **Remove IME configuration**: Temporarily remove the input_method_keyboard_map file
3. **Use a different IME**: Try an alternative IME implementation

## Related Files

- `cosmic-comp/src/lib.rs` - Main event loop with instrumentation
- `cosmic-comp/DEBUGGING_IME_POPUP_DEADLOCK.md` - Previous investigation (focus/keyboard/grab)
- `smithay/src/wayland/input_method/input_method_handle.rs` - IME handler (already instrumented)

## Technical Notes

- The 10-second timeout is likely hardcoded in the Wayland server or client library
- This is different from the original IME keyboard grab deadlock which was fixed by releasing locks before protocol calls
- The hang is synchronous - the entire compositor freezes, suggesting a blocking operation on the main thread