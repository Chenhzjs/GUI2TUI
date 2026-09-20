#!/usr/bin/env python3
"""Exact-process control for the disposable v0.7D Wayland container."""

import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import time


HOME = pathlib.Path("/home/gui2tui")
PREFIX = HOME / ".local"
RUNTIME = PREFIX / "run"
EVIDENCE = HOME / "evidence"
SESSION = EVIDENCE / "session.json"
GUI = PREFIX / "bin/gui2tui"
INSPECT = PREFIX / "libexec/gui2tui/gui2tui-inspect"
WESTON_SOCKET = "wayland-v07d"
FIXTURES = {
    "native-selection": (
        "v05a_gtk_selection_fixture.py",
        "gui2tui-v05a-gtk-selection",
        "native-gtk",
    ),
    "native-live": ("gtk4_live_fixture.py", "gui2tui-live-fixture", "native-gtk"),
    "native-qt": ("qt6_live_fixture.py", "gui2tui-qt-fixture", "native-qt"),
    "xwayland-selection": (
        "v05a_gtk_selection_fixture.py",
        "gui2tui-v05a-gtk-selection",
        "xwayland-gtk",
    ),
    "xwayland-live": ("gtk4_live_fixture.py", "gui2tui-live-fixture", "xwayland-gtk"),
}


def base_environment() -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "HOME": str(HOME),
            "XDG_CONFIG_HOME": str(HOME / ".config"),
            "XDG_STATE_HOME": str(HOME / ".local/state"),
            "XDG_RUNTIME_DIR": str(RUNTIME),
            "LANG": "C.UTF-8",
            "TERM": "xterm-256color",
            "NO_AT_BRIDGE": "0",
            "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "1",
        }
    )
    return env


def require_user() -> None:
    if os.geteuid() == 0:
        raise SystemExit("container control must run as the unprivileged gui2tui user")


def read_session() -> dict[str, str]:
    data = json.loads(SESSION.read_text(encoding="utf-8"))
    required = {"DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY", "DISPLAY"}
    if set(data) != required:
        raise SystemExit("invalid private Wayland session metadata")
    return data


def product_environment() -> dict[str, str]:
    env = base_environment()
    env.update(read_session())
    env["XDG_SESSION_TYPE"] = "wayland"
    return env


def install() -> None:
    subprocess.run(
        ["/opt/gui2tui-bundle/install-user.sh", "--prefix", str(PREFIX)],
        check=True,
        env=base_environment(),
    )
    subprocess.run([GUI, "--version"], check=True, env=base_environment())


def configure_vim() -> None:
    config_dir = HOME / ".config/gui2tui"
    config_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    config = config_dir / "config.toml"
    config.write_text(
        "version = 1\n"
        "[interaction.complex_text]\n"
        'program = "vim"\n'
        'args = ["-f", "-n", "-i", "NONE", "--cmd", '
        '"set nobackup nowritebackup backupcopy=yes", "{file}"]\n',
        encoding="utf-8",
    )
    config.chmod(0o600)


def write_pid(name: str, process: subprocess.Popen[bytes]) -> None:
    path = EVIDENCE / f"{name}.pid"
    path.write_text(f"{process.pid}\n", encoding="ascii")
    path.chmod(0o600)


def owned_pid(name: str, expected: str) -> int | None:
    path = EVIDENCE / f"{name}.pid"
    try:
        pid = int(path.read_text(encoding="ascii").strip())
        command = pathlib.Path(f"/proc/{pid}/cmdline").read_bytes()
    except (FileNotFoundError, ValueError):
        path.unlink(missing_ok=True)
        return None
    if expected.encode() not in command:
        raise SystemExit(f"refusing reused or unowned pid {pid} for {name}")
    return pid


def wait_path(path: pathlib.Path, timeout: float = 15) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.05)
    raise SystemExit(f"timed out waiting for {path.name}")


def start_session() -> None:
    if SESSION.exists():
        raise SystemExit("private Wayland session is already recorded")
    RUNTIME.mkdir(mode=0o700, parents=True, exist_ok=True)
    EVIDENCE.mkdir(mode=0o700, parents=True, exist_ok=True)
    if RUNTIME.stat().st_mode & 0o077 or RUNTIME.stat().st_uid != os.geteuid():
        raise SystemExit("unsafe XDG_RUNTIME_DIR ownership or mode")

    bus_socket = RUNTIME / "v07d-session-bus"
    bus_address = f"unix:path={bus_socket}"
    bus_log = (EVIDENCE / "dbus.log").open("ab", buffering=0)
    bus = subprocess.Popen(
        ["dbus-daemon", "--session", "--nofork", f"--address={bus_address}"],
        stdin=subprocess.DEVNULL,
        stdout=bus_log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        env=base_environment(),
    )
    write_pid("dbus", bus)
    wait_path(bus_socket)

    env = base_environment()
    env["DBUS_SESSION_BUS_ADDRESS"] = bus_address
    weston_log = EVIDENCE / "weston.log"
    weston_out = (EVIDENCE / "weston.stdout").open("ab", buffering=0)
    weston = subprocess.Popen(
        [
            "weston",
            "--backend=headless",
            "--renderer=pixman",
            "--shell=desktop",
            f"--socket={WESTON_SOCKET}",
            "--width=1280",
            "--height=720",
            "--idle-time=0",
            "--xwayland",
            "--no-config",
            f"--log={weston_log}",
        ],
        stdin=subprocess.DEVNULL,
        stdout=weston_out,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        env=env,
    )
    write_pid("weston", weston)
    wait_path(RUNTIME / WESTON_SOCKET)

    deadline = time.monotonic() + 15
    display = None
    while time.monotonic() < deadline:
        if weston.poll() is not None:
            raise SystemExit("Weston exited during startup")
        text = weston_log.read_text(encoding="utf-8", errors="replace")
        match = re.search(r"xserver listening on display (:[0-9]+)", text)
        if match:
            display = match.group(1)
            break
        time.sleep(0.05)
    if display is None:
        raise SystemExit("Weston did not publish an XWayland display")

    session = {
        "DBUS_SESSION_BUS_ADDRESS": bus_address,
        "WAYLAND_DISPLAY": WESTON_SOCKET,
        "DISPLAY": display,
    }
    SESSION.write_text(json.dumps(session), encoding="utf-8")
    SESSION.chmod(0o600)
    env.update(session)
    env["XDG_SESSION_TYPE"] = "wayland"
    subprocess.run(
        ["gsettings", "set", "org.gnome.desktop.interface", "toolkit-accessibility", "true"],
        check=True,
        env=env,
    )
    subprocess.run(
        [
            "dbus-send",
            "--session",
            "--print-reply",
            "--dest=org.a11y.Bus",
            "/org/a11y/bus",
            "org.a11y.Bus.GetAddress",
        ],
        check=True,
        env=env,
        stdout=(EVIDENCE / "a11y-address-redacted.log").open("wb"),
    )
    # Never retain the private address returned above as evidence.
    (EVIDENCE / "a11y-address-redacted.log").write_text(
        "org.a11y.Bus.GetAddress=AVAILABLE\n", encoding="utf-8"
    )
    subprocess.run(
        ["wayland-info"],
        check=True,
        env=env,
        stdout=(EVIDENCE / "wayland-info.txt").open("wb"),
        stderr=subprocess.STDOUT,
    )


def fixture_environment(kind: str, protocol_log: pathlib.Path) -> dict[str, str]:
    env = product_environment()
    if kind == "native-gtk":
        env["GDK_BACKEND"] = "wayland"
        env["WAYLAND_DEBUG"] = "1"
        env.pop("DISPLAY", None)
    elif kind == "native-qt":
        env["QT_QPA_PLATFORM"] = "wayland"
        env["WAYLAND_DEBUG"] = "1"
        env.pop("DISPLAY", None)
    elif kind == "xwayland-gtk":
        env["GDK_BACKEND"] = "x11"
        env["XDG_SESSION_TYPE"] = "x11"
        env.pop("WAYLAND_DISPLAY", None)
    else:
        raise SystemExit(f"unknown fixture kind: {kind}")
    env["GUI2TUI_PROTOCOL_LOG"] = str(protocol_log)
    return env


def start_fixture(name: str) -> None:
    filename, selector, kind = FIXTURES[name]
    if owned_pid(name, filename) is not None:
        raise SystemExit(f"fixture is already running: {name}")
    protocol = EVIDENCE / f"{name}.protocol"
    output = (EVIDENCE / f"{name}.stdout").open("ab", buffering=0)
    error = protocol.open("ab", buffering=0)
    process = subprocess.Popen(
        ["python3", f"/opt/gui2tui-fixtures/{filename}"],
        env=fixture_environment(kind, protocol),
        stdin=subprocess.DEVNULL,
        stdout=output,
        stderr=error,
        start_new_session=True,
    )
    write_pid(name, process)
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        result = subprocess.run(
            [INSPECT, "--session", "desktop", "--app", selector],
            env=product_environment(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if result.returncode == 0:
            if kind.startswith("native-"):
                trace = protocol.read_text(encoding="utf-8", errors="replace")
                if "wl_display" not in trace or "xdg_wm_base" not in trace:
                    raise SystemExit(f"fixture lacks native Wayland protocol evidence: {name}")
            return
        if process.poll() is not None:
            raise SystemExit(f"fixture exited before AT-SPI registration: {name}")
        time.sleep(0.1)
    raise SystemExit(f"fixture did not register in AT-SPI: {name}")


def stop_pid(name: str, expected: str) -> None:
    pid = owned_pid(name, expected)
    if pid is None:
        return
    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.05)
    else:
        raise SystemExit(f"owned process did not stop: {name} pid={pid}")
    (EVIDENCE / f"{name}.pid").unlink(missing_ok=True)


def stop_fixture(name: str) -> None:
    stop_pid(name, FIXTURES[name][0])


def stop_session() -> None:
    for name in FIXTURES:
        stop_fixture(name)
    stop_pid("weston", "weston")
    stop_pid("dbus", "dbus-daemon")
    SESSION.unlink(missing_ok=True)
    for path in (RUNTIME / WESTON_SOCKET, RUNTIME / "v07d-session-bus"):
        path.unlink(missing_ok=True)


def environment_json() -> None:
    print(json.dumps(product_environment()))


def main() -> None:
    require_user()
    EVIDENCE.mkdir(mode=0o700, parents=True, exist_ok=True)
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "install":
        install()
    elif command == "configure-vim":
        configure_vim()
    elif command == "start-session":
        start_session()
    elif command == "environment":
        environment_json()
    elif command == "start-fixture" and len(sys.argv) == 3 and sys.argv[2] in FIXTURES:
        start_fixture(sys.argv[2])
    elif command == "stop-fixture" and len(sys.argv) == 3 and sys.argv[2] in FIXTURES:
        stop_fixture(sys.argv[2])
    elif command == "stop-session":
        stop_session()
    else:
        raise SystemExit(
            "usage: v07d_wayland_container_control.py "
            "{install|configure-vim|start-session|environment|start-fixture NAME|stop-fixture NAME|stop-session}"
        )


if __name__ == "__main__":
    main()
