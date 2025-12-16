#!/bin/bash

echo "===== IME Diagnostics for cosmic-comp ====="
echo ""

echo "1. Checking for running IME processes..."
echo "-------------------------------------------"
ps aux | grep -E "(fcitx|ibus|fcitx5)" | grep -v grep
echo ""

echo "2. Checking Wayland environment..."
echo "-------------------------------------------"
echo "WAYLAND_DISPLAY: $WAYLAND_DISPLAY"
echo "XDG_SESSION_TYPE: $XDG_SESSION_TYPE"
echo ""

echo "3. Checking for IME-related Wayland protocols..."
echo "-------------------------------------------"
if command -v wayland-scanner &> /dev/null; then
    echo "wayland-scanner found"
else
    echo "wayland-scanner not found (install wayland-protocols)"
fi
echo ""

echo "4. Listing all Wayland globals visible to clients..."
echo "-------------------------------------------"
if command -v weston-info &> /dev/null; then
    weston-info 2>/dev/null | grep -E "(input_method|text_input)" || echo "No input method or text input protocols found"
else
    echo "weston-info not installed (install weston)"
    echo "Cannot list Wayland globals"
fi
echo ""

echo "5. Checking which processes are connected to Wayland socket..."
echo "-------------------------------------------"
if [ -n "$WAYLAND_DISPLAY" ]; then
    WAYLAND_SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$WAYLAND_DISPLAY"
    if [ -S "$WAYLAND_SOCKET_PATH" ]; then
        echo "Wayland socket: $WAYLAND_SOCKET_PATH"
        lsof "$WAYLAND_SOCKET_PATH" 2>/dev/null | grep -E "(fcitx|ibus)" || echo "No IME processes connected to Wayland socket"
    else
        echo "Wayland socket not found at $WAYLAND_SOCKET_PATH"
    fi
else
    echo "WAYLAND_DISPLAY not set"
fi
echo ""

echo "6. Checking IME configuration..."
echo "-------------------------------------------"
echo "GTK_IM_MODULE: ${GTK_IM_MODULE:-not set}"
echo "QT_IM_MODULE: ${QT_IM_MODULE:-not set}"
echo "XMODIFIERS: ${XMODIFIERS:-not set}"
echo ""

echo "7. Testing if IMEs are using XWayland..."
echo "-------------------------------------------"
if command -v xlsclients &> /dev/null; then
    xlsclients 2>/dev/null | grep -E "(fcitx|ibus)" && echo "Found IME running as X11 client!" || echo "No IME X11 clients found"
else
    echo "xlsclients not installed (install x11-utils)"
fi
echo ""

echo "8. Checking fcitx5 specific status..."
echo "-------------------------------------------"
if command -v fcitx5-diagnose &> /dev/null; then
    fcitx5-diagnose 2>&1 | grep -A5 -E "(Wayland|Input Method)" || echo "fcitx5-diagnose found but no relevant info"
elif pgrep -x fcitx5 > /dev/null; then
    echo "fcitx5 is running but fcitx5-diagnose not available"
    echo "Process details:"
    ps -fp $(pgrep -x fcitx5)
else
    echo "fcitx5 not running"
fi
echo ""

echo "9. Checking ibus specific status..."
echo "-------------------------------------------"
if pgrep -x ibus-daemon > /dev/null; then
    echo "ibus-daemon is running"
    echo "Process details:"
    ps -fp $(pgrep -x ibus-daemon)
    echo ""
    if command -v ibus &> /dev/null; then
        echo "IBus version:"
        ibus version 2>/dev/null || echo "Could not get ibus version"
    fi
else
    echo "ibus-daemon not running"
fi
echo ""

echo "10. Checking cosmic-comp input method log..."
echo "-------------------------------------------"
if [ -f /tmp/cosmic-comp-input-method-debug.log ]; then
    echo "Last 20 lines of log:"
    tail -20 /tmp/cosmic-comp-input-method-debug.log
else
    echo "Log file /tmp/cosmic-comp-input-method-debug.log not found"
fi
echo ""

echo "11. Recommendations..."
echo "-------------------------------------------"
if ! pgrep -x fcitx5 > /dev/null && ! pgrep -x ibus-daemon > /dev/null; then
    echo "⚠️  No IME processes detected. Start fcitx5 or ibus-daemon first."
elif [ "$XDG_SESSION_TYPE" != "wayland" ]; then
    echo "⚠️  Not running in a Wayland session (XDG_SESSION_TYPE=$XDG_SESSION_TYPE)"
elif ! lsof "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/$WAYLAND_DISPLAY" 2>/dev/null | grep -qE "(fcitx|ibus)"; then
    echo "⚠️  IME process running but NOT connected to Wayland socket"
    echo "    This suggests the IME is running as X11 only."
    echo "    Make sure fcitx5-wayland or ibus-wayland packages are installed."
else
    echo "✓ IME appears to be running and connected to Wayland"
    echo "  Check the log output above to see if GetInputMethod requests are being made."
fi
echo ""

echo "===== End of diagnostics ====="
