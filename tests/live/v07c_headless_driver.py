#!/usr/bin/env python3
"""Drive real Docker and SSH PTYs for the v0.7C headless qualification."""

import argparse
import errno
import fcntl
import hashlib
import json
import os
import pathlib
import pty
import re
import select
import signal
import struct
import subprocess
import termios
import time


HOME = "/home/gui2tui"
ENVIRONMENT = {
    "HOME": HOME,
    "XDG_CONFIG_HOME": f"{HOME}/.config",
    "XDG_STATE_HOME": f"{HOME}/.local/state",
    "XDG_RUNTIME_DIR": f"{HOME}/.local/run",
    "LANG": "C.UTF-8",
    "TERM": "xterm-256color",
}
GUI = f"{HOME}/.local/bin/gui2tui"
INSPECT = f"{HOME}/.local/libexec/gui2tui/gui2tui-inspect"
SELECTION_APP = "gui2tui-v05a-gtk-selection"
LIVE_APP = "gui2tui-live-fixture"
PROMPT = b"V07C_PROMPT> "
EOF = object()
ANSI = re.compile(
    rb"(?:\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-_])"
)


class PtyProcess:
    """Small bounded PTY driver using only the Python standard library."""

    def __init__(self, command: list[str], dimensions: tuple[int, int]) -> None:
        pid, descriptor = pty.fork()
        if pid == 0:
            os.execvp(command[0], command)
        self.pid = pid
        self.child_fd = descriptor
        self.buffer = bytearray()
        self.cursor = 0
        self.exitstatus: int | None = None
        self._eof = False
        rows, columns = dimensions
        fcntl.ioctl(descriptor, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))

    def _record_status(self, block: bool = False) -> None:
        if self.exitstatus is not None:
            return
        options = 0 if block else os.WNOHANG
        try:
            waited, status = os.waitpid(self.pid, options)
        except ChildProcessError:
            return
        if waited == 0:
            return
        if os.WIFEXITED(status):
            self.exitstatus = os.WEXITSTATUS(status)
        elif os.WIFSIGNALED(status):
            self.exitstatus = -os.WTERMSIG(status)

    def isalive(self) -> bool:
        self._record_status()
        return self.exitstatus is None

    def read_nonblocking(self, size: int, timeout: float) -> bytes:
        if self._eof:
            return b""
        readable, _, _ = select.select([self.child_fd], [], [], timeout)
        if not readable:
            return b""
        try:
            chunk = os.read(self.child_fd, size)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            chunk = b""
        if not chunk:
            self._eof = True
            self._record_status(block=True)
            return b""
        self.buffer.extend(chunk)
        return chunk

    def send(self, data: bytes) -> None:
        os.write(self.child_fd, data)

    def sendline(self, data: bytes) -> None:
        self.send(data + b"\n")

    def expect(self, pattern: bytes | object, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if pattern is not EOF:
                index = bytes(self.buffer).find(pattern, self.cursor)
                if index >= 0:
                    self.cursor = index + len(pattern)
                    return
            elif self._eof or not self.isalive():
                return
            self.read_nonblocking(65536, min(0.05, max(0.0, deadline - time.monotonic())))
        wanted = "EOF" if pattern is EOF else repr(pattern)
        tail = ANSI.sub(b"", bytes(self.buffer[-4000:])).decode("utf-8", "replace")
        raise AssertionError(f"PTY did not reach {wanted}\n{tail}")

    def close(self, force: bool = False) -> None:
        if force and self.isalive():
            os.kill(self.pid, signal.SIGKILL)
        if self.isalive():
            self._record_status(block=True)
        try:
            os.close(self.child_fd)
        except OSError:
            pass


class Qualification:
    def __init__(self, arguments: argparse.Namespace) -> None:
        self.container = arguments.container
        self.port = str(arguments.port)
        self.identity = arguments.identity
        self.known_hosts = arguments.known_hosts
        self.evidence = arguments.evidence
        self.results: dict[str, object] = {}

    def docker(self, *command: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        args = ["docker", "exec", "--user", "gui2tui"]
        for name, value in ENVIRONMENT.items():
            args.extend(["--env", f"{name}={value}"])
        args.extend([self.container, *command])
        return subprocess.run(args, check=check, text=True, capture_output=True)

    def ssh_base(self, tty: bool) -> list[str]:
        args = [
            "ssh",
            "-F",
            "/dev/null",
            "-o",
            "IdentityAgent=none",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=2",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            f"UserKnownHostsFile={self.known_hosts}",
            "-i",
            self.identity,
            "-p",
            self.port,
        ]
        if tty:
            args.append("-tt")
        args.append("gui2tui@127.0.0.1")
        return args

    def shell(self, transport: str, evidence_name: str) -> "RemoteShell":
        if transport == "docker":
            command = ["docker", "exec", "-it", "--user", "gui2tui"]
            for name, value in ENVIRONMENT.items():
                command.extend(["--env", f"{name}={value}"])
            command.extend([self.container, "bash", "--noprofile", "--norc"])
        else:
            command = self.ssh_base(tty=True)
        return RemoteShell(command, evidence_name)

    def tree(self, application: str) -> str:
        return self.docker(INSPECT, "--session", "managed", "--app", application).stdout

    def wait_tree(self, application: str, wanted: str, timeout: float = 15) -> str:
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            result = self.docker(
                INSPECT, "--session", "managed", "--app", application, check=False
            )
            last = result.stdout
            if result.returncode == 0 and wanted in last:
                return last
            time.sleep(0.1)
        raise AssertionError(f"fresh AT-SPI readback missing expected synthetic state: {wanted!r}")

    def doctor(self, label: str, transport: str = "docker") -> None:
        command = [GUI, "--session", "managed", "doctor", "--json"]
        if transport == "docker":
            output = self.docker(*command).stdout
        else:
            remote = ["env", *[f"{key}={value}" for key, value in ENVIRONMENT.items()], *command]
            output = subprocess.run(
                [*self.ssh_base(tty=False), *remote],
                check=True,
                text=True,
                capture_output=True,
            ).stdout
        report = json.loads(output)
        checks = {item["name"]: item for item in report["checks"]}
        for name in (
            "installation-entry",
            "helper-inspector",
            "helper-managed-headless",
            "session-bus",
            "accessibility-bus",
            "accessibility-registry",
            "accessible-applications",
            "external-text-handler",
        ):
            if checks[name]["level"] != "PASS":
                raise AssertionError(f"Doctor {name} was not PASS: {checks[name]}")
        self.results[f"{label}_doctor"] = "PASS"

    def audit_private_artifacts(self) -> str:
        script = r"""
import os
import pathlib
import stat

root = pathlib.Path('/home/gui2tui/.local/run/gui2tui-owned-1001')
uid = os.geteuid()
namespaces = artifacts = 0
if root.exists():
    for directory, child_directories, files in os.walk(root, followlinks=False):
        path = pathlib.Path(directory)
        metadata = path.lstat()
        if metadata.st_uid != uid or not stat.S_ISDIR(metadata.st_mode) or metadata.st_mode & 0o077:
            raise SystemExit('unsafe private artifact directory metadata')
        for name in child_directories:
            child = path / name
            child_metadata = child.lstat()
            if child_metadata.st_uid != uid or not stat.S_ISDIR(child_metadata.st_mode) or child_metadata.st_mode & 0o077:
                raise SystemExit('unsafe private artifact namespace metadata')
            if name.startswith('operation-'):
                namespaces += 1
        for name in files:
            child = path / name
            child_metadata = child.lstat()
            if (child_metadata.st_uid != uid or not stat.S_ISREG(child_metadata.st_mode)
                    or child_metadata.st_nlink != 1 or child_metadata.st_mode & 0o077):
                raise SystemExit('unsafe private artifact file metadata')
            if name.startswith('artifact-'):
                artifacts += 1
print(f'{namespaces}:{artifacts}')
"""
        counts = self.docker("python3", "-c", script).stdout.strip()
        if not re.fullmatch(r"[0-9]+:[0-9]+", counts):
            raise AssertionError("private artifact audit returned an invalid summary")
        return counts

    def semantic_workflow(self, transport: str, label: str) -> None:
        shell = self.shell(transport, label)
        try:
            shell.start()
            shell.start_tui(SELECTION_APP, layout="spatial")
            shell.wait_text("Reorder current items", timeout=20)

            navigation = {}
            for name, sequence in (
                ("tab", b"\t"),
                ("shift_tab", b"\x1b[Z"),
                ("right", b"\x1b[C"),
                ("left", b"\x1b[D"),
                ("enter", b"\r"),
            ):
                before = shell.fingerprint()
                shell.send(sequence)
                shell.pump(0.25)
                navigation[name] = "OBSERVED" if shell.fingerprint() != before else "DELIVERED"
                if name == "enter":
                    # The initial GTK selection control opens its bounded choice
                    # overlay on Enter. Close that overlay before testing the
                    # global command palette.
                    shell.send(b"\x1b")
                    shell.pump(0.25)

            enhanced = {}
            for name, sequence in (
                ("f6", b"\x1b[17~"),
                ("shift_f6", b"\x1b[17;2~"),
                ("ctrl_tab", b"\x1b[9;5u"),
                ("ctrl_shift_tab", b"\x1b[9;6u"),
            ):
                before = shell.fingerprint()
                shell.send(sequence)
                shell.pump(0.25)
                enhanced[name] = "OBSERVED" if shell.fingerprint() != before else "DELIVERED"

            shell.command("Reorder current items")
            self.wait_tree(SELECTION_APP, "Structure: reordered; selected Gamma")
            # The raw host-side PTY transcript cannot reconstruct cursor-addressed
            # Ratatui cells perfectly, but it must expose the changed current state
            # before the next manual command. The fresh Inspector read above is the
            # authoritative assertion.
            shell.wait_text("selected Gamma", timeout=15)
            shell.command("Reset current items")
            self.wait_tree(SELECTION_APP, "Selected: Alpha")
            shell.finish_tui()
            shell.finish()
            self.results[f"{label}_tty"] = "PASS"
            self.results[f"{label}_navigation"] = navigation
            self.results[f"{label}_enhanced_keys"] = enhanced
            self.results[f"{label}_semantic_operation"] = "PASS"
            self.results[f"{label}_authoritative_readback"] = "PASS"
            self.results[f"{label}_dynamic_refresh"] = "PASS"
            self.results[f"{label}_terminal_restore"] = "PASS"
        finally:
            shell.close()

    def external_vim(self) -> None:
        shell = self.shell("ssh", "ssh-vim")
        try:
            shell.start()
            shell.start_tui(LIVE_APP, layout="spatial")
            shell.wait_text("Username", timeout=20)
            self.begin_vim(shell)
            shell.send(b"G")
            shell.send(b"oSSH qualified edit")
            shell.send(b"\x1b")
            shell.send(b":wq\r")
            shell.wait_text("External text update confirmed", timeout=20)
            self.wait_tree(LIVE_APP, "SSH qualified edit")
            # The external handler retires the old terminal reader and creates
            # exactly one replacement. Reuse the existing bounded hand-back
            # settling interval before testing the next user key.
            shell.pump(1.0)
            shell.finish_tui()
            shell.finish()
        finally:
            shell.close()
        artifacts = self.docker(
            "bash",
            "-c",
            "find /home/gui2tui/.local/run/gui2tui -type f -name 'artifact-*.txt' 2>/dev/null | wc -l",
        ).stdout.strip()
        if artifacts != "0":
            raise AssertionError("successful external edit left a private candidate artifact")
        self.results["ssh_external_vim"] = "PASS"
        self.results["ssh_external_writeback_readback"] = "PASS"
        self.results["ssh_external_candidate_cleanup"] = "PASS"

    @staticmethod
    def begin_vim(terminal: "ScreenDriver") -> None:
        # Focus order is derived from the current scene. Traverse it through
        # normal TUI navigation until the current complete plain-text target
        # advertises and accepts external editing, as in the existing lifecycle
        # qualification. No application identity enters production behavior.
        for _ in range(20):
            terminal.send(b"e")
            deadline = time.monotonic() + 1.0
            while time.monotonic() < deadline:
                frame = terminal.pump(0.08)
                if "alphaline" in re.sub(r"\s+", "", frame):
                    return
            terminal.send(b"\t")
        raise AssertionError("could not focus the current qualified external-text target")

    def remote_processes(self) -> set[str]:
        result = self.docker(
            "bash",
            "-c",
            "ps -u $(id -u) -o comm= | grep -E '^(gui2tui|vim)$' || true",
        )
        return set(result.stdout.split())

    def abrupt_disconnect(self, in_handler: bool) -> None:
        remote_command = ["env", *[f"{key}={value}" for key, value in ENVIRONMENT.items()]]
        remote_command.extend(
            [
                GUI,
                "--session",
                "managed",
                "--app",
                LIVE_APP,
                "--layout",
                "spatial",
                "--no-mouse",
            ]
        )
        child = PtyProcess([*self.ssh_base(tty=True), *remote_command], (48, 160))
        terminal = ScreenDriver(child)
        terminal.wait_text("Username", timeout=20)
        if in_handler:
            self.begin_vim(terminal)
        os.kill(child.pid, signal.SIGKILL)
        child.close(force=True)
        deadline = time.monotonic() + 10
        remaining: set[str] = set()
        while time.monotonic() < deadline:
            remaining = self.remote_processes()
            if not remaining:
                break
            time.sleep(0.2)
        if remaining:
            raise AssertionError(f"SSH disconnect left terminal-owned processes: {remaining}")
        session = self.docker(GUI, "setup", "status", check=False)
        if session.returncode != 0 or "running" not in (session.stdout + session.stderr).lower():
            raise AssertionError("SSH disconnect unexpectedly stopped the Managed session")
        fixture = self.tree(LIVE_APP)
        if "External text probe" not in fixture:
            raise AssertionError("SSH disconnect unexpectedly stopped the controlled GUI fixture")
        key = "ssh_handler_disconnect" if in_handler else "ssh_tui_disconnect"
        self.results[key] = "PASS_PROCESS_EXIT_SESSION_PRESERVED"
        if in_handler:
            counts = self.audit_private_artifacts()
            self.results["ssh_handler_disconnect_artifacts"] = (
                f"PASS_PRIVATE_METADATA_ONLY_{counts}"
            )

    def reconnect(self) -> None:
        self.semantic_workflow("ssh", "ssh_reconnect")
        self.results["ssh_reconnect_authority"] = "PASS_FRESH_PROCESS_AND_SELECTION"

    def restart_container(self) -> None:
        subprocess.run(["docker", "restart", self.container], check=True, capture_output=True)
        mapping = subprocess.run(
            ["docker", "port", self.container, "22/tcp"],
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip()
        host, separator, port = mapping.rpartition(":")
        if separator != ":" or host != "127.0.0.1" or not port.isdigit():
            raise AssertionError(f"restart produced an unsafe SSH port mapping: {mapping!r}")
        self.port = port
        scan_deadline = time.monotonic() + 10
        keyscan = None
        while time.monotonic() < scan_deadline:
            keyscan = subprocess.run(
                ["ssh-keyscan", "-p", self.port, "127.0.0.1"],
                check=False,
                capture_output=True,
            )
            if keyscan.returncode == 0 and keyscan.stdout:
                break
            time.sleep(0.1)
        if keyscan is None or keyscan.returncode != 0 or not keyscan.stdout:
            raise AssertionError("restart SSH host key did not become available")
        pathlib.Path(self.known_hosts).write_bytes(keyscan.stdout)
        pathlib.Path(self.known_hosts).chmod(0o600)
        deadline = time.monotonic() + 20
        probe = None
        while time.monotonic() < deadline:
            probe = subprocess.run(
                [*self.ssh_base(tty=False), "true"],
                text=True,
                capture_output=True,
            )
            if probe.returncode == 0:
                break
            time.sleep(0.2)
        else:
            detail = "" if probe is None else probe.stderr.strip()
            raise AssertionError(f"SSH did not return after container restart: {detail}")

        stale = self.docker(INSPECT, "--session", "managed", "--list", check=False)
        if stale.returncode == 0:
            raise AssertionError("old Managed descriptor remained usable after container restart")
        if self.docker("pgrep", "-u", "1001", "-f", "/opt/gui2tui-fixtures/", check=False).returncode == 0:
            raise AssertionError("old controlled GUI fixture survived container restart")
        self.results["container_restart_old_session_rejected"] = "PASS"
        self.results["container_restart_old_authority_not_restored"] = "PASS"
        recreated = self.docker(
            "/usr/local/libexec/gui2tui-v07c-container-control", "setup", check=False
        )
        if recreated.returncode != 0:
            detail = (recreated.stdout + recreated.stderr).strip()
            raise AssertionError(f"Managed session recreation failed after restart: {detail}")
        self.docker(
            "/usr/local/libexec/gui2tui-v07c-container-control",
            "start-fixture",
            "selection",
        )
        self.doctor("container_restart")
        self.semantic_workflow("ssh", "container_restart_fresh")
        self.results["container_restart_fresh_selection"] = "PASS"

    def run(self) -> None:
        source = self.docker("cat", "/opt/gui2tui-source-commit").stdout.strip()
        version = self.docker(GUI, "--version").stdout.strip()
        applications = self.docker(INSPECT, "--session", "managed", "--list").stdout
        if SELECTION_APP not in applications or LIVE_APP not in applications:
            raise AssertionError("fresh registry enumeration did not contain both controlled fixtures")
        self.results.update(
            {
                "source_commit": source,
                "installed_version": version,
                "installed_binary": "PASS",
                "fresh_registry_enumeration": "PASS",
                "explicit_application_selection": "PASS",
            }
        )
        self.doctor("docker")
        self.semantic_workflow("docker", "docker_exec")
        self.doctor("ssh", transport="ssh")
        self.semantic_workflow("ssh", "ssh")
        self.external_vim()
        self.abrupt_disconnect(in_handler=False)
        self.abrupt_disconnect(in_handler=True)
        self.reconnect()
        self.restart_container()
        self.evidence.write_text(
            json.dumps(self.results, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )


class ScreenDriver:
    def __init__(self, child: PtyProcess) -> None:
        self.child = child
        self.transcript = bytearray()

    def pump(self, seconds: float = 0.1) -> str:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline and self.child.isalive():
            try:
                chunk = self.child.read_nonblocking(65536, timeout=0.03)
                if not chunk:
                    continue
                self.transcript.extend(chunk)
                if len(self.transcript) > 2_000_000:
                    del self.transcript[:-1_000_000]
            except OSError:
                break
        return ANSI.sub(b"", bytes(self.transcript)).decode("utf-8", errors="replace")

    def wait_text(self, wanted: str, timeout: float = 10) -> str:
        deadline = time.monotonic() + timeout
        frame = self.pump(0.05)
        while time.monotonic() < deadline:
            compact_frame = re.sub(r"\s+", "", frame)
            compact_wanted = re.sub(r"\s+", "", wanted)
            if wanted in frame or compact_wanted in compact_frame:
                return frame
            frame = self.pump(0.08)
        raise AssertionError(f"terminal did not show {wanted!r}\n{frame}")

    def send(self, data: bytes) -> None:
        self.child.send(data)

    def fingerprint(self) -> str:
        return hashlib.sha256(bytes(self.transcript[-65536:])).hexdigest()


class RemoteShell(ScreenDriver):
    def __init__(self, command: list[str], evidence_name: str) -> None:
        child = PtyProcess(command, (48, 160))
        super().__init__(child)
        self.evidence_name = re.sub(r"[^a-z0-9-]", "-", evidence_name.lower())

    def start(self) -> None:
        self.child.sendline(
            b"export HOME=/home/gui2tui XDG_CONFIG_HOME=/home/gui2tui/.config "
            b"XDG_STATE_HOME=/home/gui2tui/.local/state "
            b"XDG_RUNTIME_DIR=/home/gui2tui/.local/run LANG=C.UTF-8 TERM=xterm-256color; "
            b"PS1='V07C_PROMPT> '; "
            + f"stty -g > /home/gui2tui/evidence/{self.evidence_name}-stty-before; ".encode()
            + b"test -t 0 && test -t 1 && test \"$(id -u)\" != 0 && "
            b"printf V07C_READY"
        )
        self.child.expect(b"V07C_READY", timeout=20)
        self.child.expect(PROMPT, timeout=10)

    def start_tui(self, application: str, layout: str) -> None:
        self.transcript.clear()
        command = (
            f"{GUI} --session managed --app {application} --layout {layout} "
            "--settle-ms 400 --no-mouse --log-level debug"
        )
        self.child.sendline(command.encode())

    def command(self, query: str) -> None:
        self.send(b":")
        self.wait_text("Command palette", timeout=10)
        self.send(query.encode() + b"\r")

    def finish_tui(self) -> None:
        self.child.cursor = len(self.child.buffer)
        self.send(b"q")
        self.child.expect(PROMPT, timeout=15)
        command = (
            f"stty -g > /home/gui2tui/evidence/{self.evidence_name}-stty-after; "
            "python3 -c 'import termios; mode=termios.tcgetattr(0)[3]; "
            "assert mode & termios.ICANON and mode & termios.ECHO' && "
            "printf V07C_STTY_RESTORED"
        )
        self.child.cursor = len(self.child.buffer)
        self.child.sendline(command.encode())
        self.child.expect(b"V07C_STTY_RESTORED", timeout=10)
        self.child.expect(PROMPT, timeout=10)

    def finish(self) -> None:
        self.child.sendline(b"exit")
        self.child.expect(EOF, timeout=15)
        self.child.close()
        if self.child.exitstatus != 0:
            raise AssertionError(f"interactive transport exited {self.child.exitstatus}")

    def close(self) -> None:
        if self.child.isalive():
            self.child.close(force=True)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--container", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--identity", required=True)
    parser.add_argument("--known-hosts", required=True)
    parser.add_argument("--evidence", required=True, type=pathlib.Path)
    return parser.parse_args()


if __name__ == "__main__":
    Qualification(parse_arguments()).run()
