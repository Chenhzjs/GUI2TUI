#!/usr/bin/env python3
"""Exact-process control for the disposable v0.7C headless container."""

import json
import os
import pathlib
import signal
import subprocess
import sys
import time


HOME = pathlib.Path("/home/gui2tui")
PREFIX = HOME / ".local"
GUI = PREFIX / "bin/gui2tui"
INSPECT = PREFIX / "libexec/gui2tui/gui2tui-inspect"
STATE = HOME / ".local/state/gui2tui/headless"
RUNTIME = HOME / ".local/run"
EVIDENCE = HOME / "evidence"
FIXTURES = {
    "selection": ("v05a_gtk_selection_fixture.py", "gui2tui-v05a-gtk-selection"),
    "live": ("gtk4_live_fixture.py", "gui2tui-live-fixture"),
}


def environment() -> dict[str, str]:
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
            "XDG_SESSION_TYPE": "x11",
        }
    )
    return env


def require_user() -> None:
    if os.geteuid() == 0:
        raise SystemExit("container control must run as the unprivileged gui2tui user")


def install() -> None:
    subprocess.run(
        ["/opt/gui2tui-bundle/install-user.sh", "--prefix", str(PREFIX)],
        check=True,
        env=environment(),
    )
    subprocess.run([GUI, "--version"], check=True, env=environment())


def setup() -> None:
    subprocess.run([GUI, "setup", "persistent"], check=True, env=environment())
    descriptor = STATE / "session.json"
    metadata = descriptor.stat()
    if metadata.st_mode & 0o077 or metadata.st_uid != os.geteuid():
        raise SystemExit("Managed descriptor ownership or mode is unsafe")
    if (STATE.stat().st_mode & 0o077) or STATE.stat().st_uid != os.geteuid():
        raise SystemExit("Managed state directory ownership or mode is unsafe")


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


def descriptor_environment() -> dict[str, str]:
    descriptor = STATE / "session.json"
    data = json.loads(descriptor.read_text(encoding="utf-8"))
    env = environment()
    env["DISPLAY"] = data["display"]
    env["DBUS_SESSION_BUS_ADDRESS"] = data["session_bus_address"]
    return env


def start_fixture(name: str) -> None:
    filename, selector = FIXTURES[name]
    pid_file = EVIDENCE / f"{name}.pid"
    if pid_file.exists():
        try:
            old_pid = int(pid_file.read_text(encoding="ascii").strip())
            command = pathlib.Path(f"/proc/{old_pid}/cmdline").read_bytes()
        except (FileNotFoundError, ValueError):
            pid_file.unlink(missing_ok=True)
        else:
            if f"/opt/gui2tui-fixtures/{filename}".encode() in command:
                raise SystemExit(f"fixture is already running: {name}")
            raise SystemExit(f"fixture pid was reused by an unowned process: {old_pid}")
    log = (EVIDENCE / f"{name}.log").open("ab", buffering=0)
    process = subprocess.Popen(
        ["python3", f"/opt/gui2tui-fixtures/{filename}"],
        env=descriptor_environment(),
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    pid_file.write_text(f"{process.pid}\n", encoding="ascii")
    pid_file.chmod(0o600)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = subprocess.run(
            [INSPECT, "--session", "managed", "--app", selector],
            env=environment(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if result.returncode == 0:
            return
        if process.poll() is not None:
            raise SystemExit(f"fixture exited before registration: {name}")
        time.sleep(0.1)
    raise SystemExit(f"fixture did not register: {name}")


def stop_pid_file(path: pathlib.Path, expected_filename: str) -> None:
    try:
        pid = int(path.read_text(encoding="ascii").strip())
    except (FileNotFoundError, ValueError):
        return
    try:
        command = pathlib.Path(f"/proc/{pid}/cmdline").read_bytes()
    except FileNotFoundError:
        path.unlink(missing_ok=True)
        return
    expected = f"/opt/gui2tui-fixtures/{expected_filename}".encode()
    if expected not in command:
        raise SystemExit(f"refusing to stop reused, unowned pid: {pid}")
    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.05)
    else:
        raise SystemExit(f"owned fixture did not stop: pid={pid}")
    path.unlink(missing_ok=True)


def stop() -> None:
    for name, (filename, _selector) in FIXTURES.items():
        stop_pid_file(EVIDENCE / f"{name}.pid", filename)
    subprocess.run([GUI, "setup", "stop"], check=True, env=environment())


def main() -> None:
    require_user()
    EVIDENCE.mkdir(mode=0o700, parents=True, exist_ok=True)
    RUNTIME.mkdir(mode=0o700, parents=True, exist_ok=True)
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "install":
        install()
    elif command == "setup":
        setup()
    elif command == "configure-vim":
        configure_vim()
    elif command == "start-fixture" and len(sys.argv) == 3 and sys.argv[2] in FIXTURES:
        start_fixture(sys.argv[2])
    elif command == "stop":
        stop()
    else:
        raise SystemExit(
            "usage: v07c_container_control.py "
            "{install|setup|configure-vim|start-fixture NAME|stop}"
        )


if __name__ == "__main__":
    main()
