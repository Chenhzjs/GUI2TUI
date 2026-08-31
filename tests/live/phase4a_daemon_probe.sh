#!/usr/bin/env bash
set -euo pipefail
export DISPLAY="${DISPLAY_NUMBER:-:96}" XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0
Xvfb "$DISPLAY" -screen 0 800x600x24 >/tmp/gui2tui-p4a-daemon-xvfb.log 2>&1 &
xvfb_pid=$!
trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.a11y.Bus.GetAddress
gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
  --method org.freedesktop.DBus.GetNameOwner org.a11y.Bus
ps -eo pid,ppid,stat,comm,args | grep -E 'at-spi|a11y' | grep -v grep
