#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
app=${1:-chrome}
display=${DISPLAY_NUMBER:-:97}

cd "$project_root"
CARGO_TARGET_DIR="$target_dir" cargo build --bins >/dev/null

PROJECT_ROOT="$project_root" TARGET_DIR="$target_dir" TEST_APP="$app" TEST_DISPLAY="$display" \
    dbus-run-session -- bash -euo pipefail -c '
export DISPLAY="$TEST_DISPLAY"
export XDG_SESSION_TYPE=x11
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export NO_AT_BRIDGE=0
Xvfb "$DISPLAY" -screen 0 1280x800x24 >/tmp/gui2tui-p3f-xvfb.log 2>&1 &
xvfb_pid=$!
app_pid=
cleanup() {
    [[ -n "$app_pid" ]] && kill "$app_pid" 2>/dev/null || true
    kill "$xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE \
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON NO_AT_BRIDGE
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set \
    org.a11y.Status IsEnabled "<true>" >/dev/null

case "$TEST_APP" in
chrome)
    rm -rf /tmp/gui2tui-p3f-chrome-profile
    google-chrome --no-sandbox --disable-gpu \
        --disable-dev-shm-usage --no-first-run --no-default-browser-check \
        --disable-background-networking \
        --force-renderer-accessibility=complete \
        --user-data-dir=/tmp/gui2tui-p3f-chrome-profile \
        "file://$PROJECT_ROOT/tests/fixtures/browser_fixture.html" \
        >/tmp/gui2tui-p3f-chrome.log 2>&1 &
    app_pid=$!
    selector="Google Chrome"
    sentinel=browser-phase-secret
    ;;
chrome-large)
    rm -rf /tmp/gui2tui-p3f-chrome-large-profile
    google-chrome --no-sandbox --disable-gpu --disable-dev-shm-usage \
        --no-first-run --no-default-browser-check --disable-background-networking \
        --force-renderer-accessibility=complete \
        --user-data-dir=/tmp/gui2tui-p3f-chrome-large-profile \
        "file://$PROJECT_ROOT/tests/fixtures/browser_large_fixture.html?count=${LARGE_COUNT:-700}" \
        >/tmp/gui2tui-p3f-chrome-large.log 2>&1 &
    app_pid=$!
    selector="Google Chrome"
    sentinel=browser-phase-secret
    ;;
gtk)
    python3 "$PROJECT_ROOT/tests/fixtures/gtk4_live_fixture.py" \
        >/tmp/gui2tui-p3f-gtk.log 2>&1 &
    app_pid=$!
    selector=gui2tui-live-fixture
    sentinel=phase-zero-secret
    ;;
qt)
    python3 "$PROJECT_ROOT/tests/fixtures/qt6_live_fixture.py" \
        >/tmp/gui2tui-p3f-qt.log 2>&1 &
    app_pid=$!
    selector=gui2tui-qt-fixture
    sentinel=phase-two-secret
    ;;
qt-rich)
    python3 "$PROJECT_ROOT/tests/fixtures/qt6_rich_text_fixture.py" \
        >/tmp/gui2tui-p3f-qt-rich.log 2>&1 &
    app_pid=$!
    selector=gui2tui-qt-rich-text-fixture
    sentinel=phase-two-secret
    ;;
firefox)
    rm -rf /tmp/gui2tui-p3f-firefox-profile
    mkdir -p /tmp/gui2tui-p3f-firefox-profile
    cp "$PROJECT_ROOT/tests/fixtures/firefox-user.js" \
        /tmp/gui2tui-p3f-firefox-profile/user.js
    "${FIREFOX_BIN:-firefox}" --no-remote --profile /tmp/gui2tui-p3f-firefox-profile \
        "file://$PROJECT_ROOT/tests/fixtures/browser_fixture.html" \
        >/tmp/gui2tui-p3f-firefox.log 2>&1 &
    app_pid=$!
    selector=Firefox
    sentinel=browser-phase-secret
    ;;
libreoffice)
    rm -rf /tmp/gui2tui-p3f-lo-profile
    libreoffice --nologo --nodefault --norestore \
        -env:UserInstallation=file:///tmp/gui2tui-p3f-lo-profile \
        "$PROJECT_ROOT/tests/fixtures/libreoffice_content_fixture.fodt" \
        >/tmp/gui2tui-p3f-libreoffice.log 2>&1 &
    app_pid=$!
    selector=soffice
    sentinel=gui2tui-no-password-sentinel
    ;;
*)
    echo "unsupported fixture: $TEST_APP" >&2
    exit 2
    ;;
esac
sleep 8
inspect="$TARGET_DIR/debug/gui2tui-inspect"
"$inspect" --list
if [[ "${PRODUCT_BENCHMARK:-0}" == 1 ]]; then
    APP_SELECTOR="$selector" python3 tests/live/phase4b_bootstrap_bench.py
    exit 0
fi
if [[ "${HANDOFF_ONLY:-0}" == 1 ]]; then
    APP_SELECTOR="$selector" GUI="$TARGET_DIR/debug/gui2tui" \
        python3 - <<"PY"
import os
import time
import pexpect
import pyte

child = pexpect.spawn(
    os.environ["GUI"], ["--app", os.environ["APP_SELECTOR"]],
    env=os.environ.copy(), encoding=None, dimensions=(38, 120),
)
time.sleep(1.5)
child.send(b"\r")
time.sleep(.2)
child.send(b"/Replace article paragraph")
time.sleep(.2)
child.send(b"\r")
time.sleep(.2)
child.send(b"\r")
time.sleep(.7)
capture = b""
deadline = time.monotonic() + .7
while time.monotonic() < deadline:
    try:
        capture += child.read_nonblocking(100000, timeout=.1)
    except (pexpect.TIMEOUT, pexpect.EOF):
        break
screen = pyte.Screen(120, 38)
pyte.Stream(screen).feed(capture.decode("utf-8", "ignore"))
assert "Reader task focused" in "\n".join(screen.display)
print("READER_TASK_HANDOFF_LIVE_PASS")
child.send(b"\x1b")
time.sleep(.7)
deadline = time.monotonic() + .7
while time.monotonic() < deadline:
    try:
        capture += child.read_nonblocking(100000, timeout=.1)
    except (pexpect.TIMEOUT, pexpect.EOF):
        break
screen = pyte.Screen(120, 38)
pyte.Stream(screen).feed(capture.decode("utf-8", "ignore"))
restored = "\n".join(screen.display)
assert "Reader" in restored and "Replace article paragraph" in restored
print("READER_POSITION_RESTORE_LIVE_PASS")
child.terminate(force=True)
PY
    exit 0
fi
if [[ "${FAST_BENCHMARK:-0}" == 1 ]]; then
    for run in $(seq 1 "${BENCHMARK_RUNS:-3}"); do
        APP_SELECTOR="$selector" GUI="$TARGET_DIR/debug/gui2tui" OUTPUT="/tmp/gui2tui-p3f-benchmark-$run.bin" \
            python3 - <<"PY"
import os
import time
import pexpect

child = pexpect.spawn(
    os.environ["GUI"], ["--app", os.environ["APP_SELECTOR"]],
    env=os.environ.copy(), encoding=None, dimensions=(38, 120),
)
time.sleep(1.5)
with open(f"/proc/{child.pid}/status", encoding="utf-8") as status:
    for line in status:
        if line.startswith("VmRSS:"):
            print(line.strip())
            break
child.send(b"q")
child.expect(pexpect.EOF, timeout=8)
with open(os.environ["OUTPUT"], "wb") as output:
    output.write(child.before)
PY
        strings "/tmp/gui2tui-p3f-benchmark-$run.bin" \
            | grep -Eo "Loaded [0-9,]+ semantic nodes via [^|]+" | tail -1
    done
    exit 0
fi
"$inspect" --app "$selector" --bootstrap walk --dump-content >"/tmp/gui2tui-p3f-$TEST_APP-content.txt"
"$inspect" --app "$selector" --bootstrap walk --dump-virtual-collections >"/tmp/gui2tui-p3f-$TEST_APP-virtual.txt"
"$inspect" --app "$selector" --bootstrap walk --probe-tables >"/tmp/gui2tui-p3f-$TEST_APP-table.txt"
"$inspect" --app "$selector" --probe-collection >"/tmp/gui2tui-p3f-$TEST_APP-collection.txt"
grep -E "Content root|VirtualCollection|realized=|Table locator|Table interface|Collection nodes|matches-|active-descendant" \
    "/tmp/gui2tui-p3f-$TEST_APP-content.txt" \
    "/tmp/gui2tui-p3f-$TEST_APP-virtual.txt" \
    "/tmp/gui2tui-p3f-$TEST_APP-table.txt" \
    "/tmp/gui2tui-p3f-$TEST_APP-collection.txt" || true
if grep -q "$sentinel" "/tmp/gui2tui-p3f-$TEST_APP-content.txt"; then
    echo "password sentinel leaked" >&2
    exit 44
fi

if [[ "$TEST_APP" != qt-rich ]]; then
    APP_SELECTOR="$selector" GUI="$TARGET_DIR/debug/gui2tui" OUTPUT="/tmp/gui2tui-p3f-$TEST_APP-tui.bin" \
        python3 - <<"PY"
import os
import time
import pexpect

child = pexpect.spawn(
    os.environ["GUI"],
    ["--app", os.environ["APP_SELECTOR"]],
    env=os.environ.copy(),
    encoding=None,
    dimensions=(38, 120),
)
time.sleep(1.5)
child.send(b"\r/semantic\x06")
time.sleep(4 if os.environ.get("TEST_APP") == "firefox" else 2)
for _ in range(6):
    child.send(b"\x1b")
    time.sleep(.35)
child.send(b"q")
child.expect(pexpect.EOF, timeout=8)
with open(os.environ["OUTPUT"], "wb") as output:
    output.write(child.before)
PY
    strings "/tmp/gui2tui-p3f-$TEST_APP-tui.bin" \
        | grep -E "Full search|blocks|matches|Reader|quarantined" | tail -12 || true

    if [[ "$TEST_APP" == chrome ]]; then
        APP_SELECTOR="$selector" GUI="$TARGET_DIR/debug/gui2tui" OUTPUT=/tmp/gui2tui-p3f-chrome-table-tui.bin \
            python3 - <<"PY"
import os
import time
import pexpect

child = pexpect.spawn(
    os.environ["GUI"], ["--app", os.environ["APP_SELECTOR"]],
    env=os.environ.copy(), encoding=None, dimensions=(38, 120),
)
time.sleep(1.5)
# Open Reader, use the bounded index to find the real HTML table anchor,
# choose it, then enter terminal-native table navigation and move one cell.
child.send(b"\r/Evaluation scores\r\r")
time.sleep(.8)
child.send(b"l")
child.expect(b"Navigate semantic cells", timeout=4)
time.sleep(.5)
frame_data = child.before + child.after
try:
    frame_data += child.read_nonblocking(100000, timeout=.2)
except (pexpect.TIMEOUT, pexpect.EOF):
    pass
with open(os.environ["OUTPUT"] + ".frame", "wb") as frame:
    frame.write(frame_data)
for _ in range(5):
    child.send(b"\x1b")
    time.sleep(.35)
child.send(b"q")
child.expect(pexpect.EOF, timeout=8)
with open(os.environ["OUTPUT"], "wb") as output:
    output.write(child.before)
PY
        strings /tmp/gui2tui-p3f-chrome-table-tui.bin \
            | grep -E "Table|Evaluation scores|semantic row/column|Row [0-9]" | tail -12 || true
        python3 - <<"PY"
import pyte
screen = pyte.Screen(120, 38)
stream = pyte.Stream(screen)
with open("/tmp/gui2tui-p3f-chrome-table-tui.bin.frame", "rb") as capture:
    stream.feed(capture.read().decode("utf-8", "ignore"))
for line in screen.display:
    if " r" in line or "Table" in line or "semantic cells" in line:
        print(line.rstrip())
PY
    fi
else
    set +e
    APP_SELECTOR="$selector" GUI="$TARGET_DIR/debug/gui2tui" \
        timeout 10 python3 - <<"PY" >/tmp/gui2tui-p3f-qt-rich-tui.txt 2>&1
import os
import time
import pexpect

child = pexpect.spawn(
    os.environ["GUI"],
    ["--app", os.environ["APP_SELECTOR"]],
    env=os.environ.copy(),
    encoding=None,
    dimensions=(30, 100),
)
time.sleep(1)
child.send(b"\r")
time.sleep(3)
child.terminate(force=True)
PY
    tui_status=$?
    set -e
    sleep 1
    if kill -0 "$app_pid" 2>/dev/null; then
        echo "Qt rich-text process survived bounded probe; tui_status=$tui_status"
    else
        echo "Qt rich-text process disappeared after bounded probe; tui_status=$tui_status"
    fi
fi

if [[ "$TEST_APP" == firefox ]]; then
    sleep 1
    "$inspect" --app "$selector" --bootstrap walk --dump-content \
        >/tmp/gui2tui-p3f-firefox-content-after-events.txt
    "$inspect" --app "$selector" --bootstrap walk --probe-tables \
        >/tmp/gui2tui-p3f-firefox-table-after-events.txt
    grep -E "Content root|Table locator|Table interface" \
        /tmp/gui2tui-p3f-firefox-content-after-events.txt \
        /tmp/gui2tui-p3f-firefox-table-after-events.txt || true
fi
'
