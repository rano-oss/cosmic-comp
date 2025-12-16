#!/bin/bash

# Debug script for Input Method (IME) issues in cosmic-comp
# This script helps capture and filter logs related to IME activation/deactivation

set -e

LOG_FILE="/tmp/cosmic-comp-ime-debug.log"
FILTERED_LOG="/tmp/cosmic-comp-ime-filtered.log"

echo "=== Cosmic Compositor IME Debug Helper ==="
echo ""
echo "This script will help you debug input method issues."
echo ""
echo "Instructions:"
echo "1. Make sure cosmic-comp is rebuilt with the latest changes"
echo "2. Save your work - you'll need to restart the compositor"
echo "3. Switch to TTY2 (Ctrl+Alt+F2)"
echo "4. Login and run this script"
echo "5. Switch back to TTY1 (Ctrl+Alt+F1) to test"
echo ""
echo "When ready, the script will:"
echo "  - Kill the current cosmic-comp"
echo "  - Start a new one with logging enabled"
echo "  - Filter logs to show only IME-related events"
echo ""

read -p "Press Enter to continue or Ctrl+C to cancel..."

echo ""
echo "Killing existing cosmic-comp..."
pkill cosmic-comp || true
sleep 1

echo "Starting cosmic-comp with logging..."
echo "Logs will be saved to: $LOG_FILE"
echo "Filtered logs will be saved to: $FILTERED_LOG"
echo ""
echo "========================================"
echo "Test sequence to follow:"
echo "1. Switch to TW layout (should activate chewingwl)"
echo "2. Type some characters in a text field"
echo "3. Switch to Norwegian layout (should deactivate)"
echo "4. Type some characters again"
echo "5. Press Ctrl+C here to stop and analyze logs"
echo "========================================"
echo ""

# Start cosmic-comp with logging, filter in real-time
RUST_LOG=cosmic_comp=info,smithay=info cosmic-comp 2>&1 | tee "$LOG_FILE" | grep -E "(sync_input_method|InputMethod|active_input_method|clear_active|set_active|keyboard_grab|KeyboardGrab|layout changed)" --line-buffered --color=always

echo ""
echo "cosmic-comp stopped. Analyzing logs..."
echo ""

# Create filtered log
grep -E "(sync_input_method|InputMethod|active_input_method|clear_active|set_active|keyboard_grab|KeyboardGrab|layout changed)" "$LOG_FILE" > "$FILTERED_LOG" || true

echo "=== Log Analysis ==="
echo ""
echo "Full log saved to: $LOG_FILE"
echo "Filtered log saved to: $FILTERED_LOG"
echo ""
echo "Key events to look for:"
echo "  - 'sync_input_method_with_layout: ENTRY' - Layout sync started"
echo "  - 'set_active_instance' - Input method selected"
echo "  - 'activate_input_method - CALLED' - Activation triggered"
echo "  - 'clear_active_instance - START' - Clearing started"
echo "  - 'active_input_method_id: Some' vs 'None' - Current state"
echo "  - 'InputMethodKeyboardGrab: Checking active state' - Event routing"
echo "  - 'No active input method, forwarding to keyboard' - Events bypassing IME"
echo "  - 'Forwarding key to input method' - Events going to IME"
echo ""
echo "To view the filtered log:"
echo "  less $FILTERED_LOG"
echo ""
echo "To view the full log:"
echo "  less $LOG_FILE"
echo ""
