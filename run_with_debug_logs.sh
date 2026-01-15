#!/bin/bash

# Script to run cosmic-comp with comprehensive debug logging
# This helps identify deadlocks related to IME and popup handling

set -e

# Set the log level to info to capture all our custom logging
export RUST_LOG="cosmic_comp=info,smithay=info,warn"

# Optional: uncomment to save logs to a file with timestamp
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOGFILE="cosmic_comp_debug_${TIMESTAMP}.log"

echo "Starting cosmic-comp with debug logging..."
echo "Logs will be saved to: $LOGFILE"
echo ""
echo "To reproduce the deadlock:"
echo "1. Make sure IME (chewingwl) is configured (input_method_keyboard_map exists)"
echo "2. Click on a panel item like volume control"
echo "3. Watch for the freeze and check where logging stops"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# Run cosmic-comp and tee output to both console and file
exec ./target/release/cosmic-comp 2>&1 | tee "$LOGFILE"
