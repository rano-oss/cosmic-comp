# Viewing Cosmic Compositor Logs

## Important: Rebuilding After Changes

**After making code changes, you must rebuild and restart cosmic-comp to see the new logs:**

```bash
# Build cosmic-comp (from the cosmic-comp directory)
just build-release
# or
cargo build --release

# Restart cosmic-comp
# Option 1: Log out and log back in
# Option 2: Kill the current process and start the new one
pkill cosmic-comp
# Then start from a terminal to see live output:
./target/release/cosmic-comp
```

## Where to Find Logs

Cosmic-comp uses the `tracing` logging framework with journald integration. Logs are sent to the systemd journal.

### View Current Session Logs

To view logs from the current cosmic-comp session:

```bash
# If running as a systemd service:
journalctl --user -u cosmic-comp.service -f

# If running from tty/terminal (more common):
journalctl _COMM=cosmic-comp -f
```

The `-f` flag follows the log in real-time (like `tail -f`).

### View Recent Logs

To view the last 100 lines of logs:

```bash
# For systemd service:
journalctl --user -u cosmic-comp.service -n 100

# For tty/terminal run:
journalctl _COMM=cosmic-comp -n 100
```

### View Logs Since Boot

```bash
# For systemd service:
journalctl --user -u cosmic-comp.service -b

# For tty/terminal run:
journalctl _COMM=cosmic-comp -b
```

### View Logs for a Specific Time Period

```bash
# For systemd service:
journalctl --user -u cosmic-comp.service --since "1 hour ago"

# For tty/terminal run:
journalctl _COMM=cosmic-comp --since "1 hour ago"
journalctl _COMM=cosmic-comp --since "2024-01-01" --until "2024-01-02"
```

### Search Logs

To search for specific keywords:

```bash
# For systemd service:
journalctl --user -u cosmic-comp.service | grep "keyboard"

# For tty/terminal run:
journalctl _COMM=cosmic-comp | grep "keyboard"
journalctl _COMM=cosmic-comp | grep "input method"
```

### If cosmic-comp is Running in a Terminal

If you're running cosmic-comp directly in a terminal, logs will appear in that terminal **and** in journald. You can see real-time output in the terminal, which is useful for debugging.

## Log Levels

Cosmic-comp uses different log levels:
- **ERROR**: Critical errors that need attention
- **WARN**: Warnings about potential issues
- **INFO**: Important informational messages (keyboard layout changes, config loading)
- **DEBUG**: Detailed debugging information (only in debug builds)

### Controlling Log Level

Set the `RUST_LOG` environment variable before starting cosmic-comp:

```bash
# Show all info and above
RUST_LOG=info cosmic-comp

# Show debug logs for cosmic-comp specifically
RUST_LOG=cosmic_comp=debug cosmic-comp

# Show debug for smithay and cosmic-comp
RUST_LOG=smithay=debug,cosmic_comp=debug cosmic-comp
```

## Quick Command Reference

```bash
# Follow logs in real-time (most useful)
journalctl _COMM=cosmic-comp -f

# Filter for keyboard/input method events
journalctl _COMM=cosmic-comp | grep -E "keyboard|input method"

# See logs from the last 5 minutes
journalctl _COMM=cosmic-comp --since "5 minutes ago"
```

## What Gets Logged

The following operations are now logged at INFO level:

1. **Keyboard Layout Changes**: When you switch keyboard layouts
   ```
   Keyboard layout changed to 'us'
   ```

2. **Input Method Config Loading**: When the input method keyboard map is loaded
   ```
   Loaded input method keyboard map from "..." with 2 entries
     Layout 'jp' -> Input method 'fcitx5'
     Layout 'kr' -> Input method 'kime'
   ```
   
   Or if no config exists:
   ```
   No input method keyboard map found at "..."
   ```

## Configuration File Location

Input method keyboard mapping configuration:
```
~/.config/cosmic/com.system76.CosmicComp/v1/input_method_keyboard_map
```

Example configuration file content (RON format):
```ron
{
    "jp": "fcitx5",
    "kr": "kime",
}
```
