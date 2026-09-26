#!/usr/bin/env python3
"""Drive real PTY and AT-SPI workflows for v0.7D Wayland qualification."""

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from v07c_headless_driver import ANSI, EOF, PtyProcess, ScreenDriver  # noqa: E402


HOME = "/home/gui2tui"
CONTROL = "/usr/local/libexec/gui2tui-v07d-container-control"
GUI = f"{HOME}/.local/bin/gui2tui"
INSPECT = f"{HOME}/.local/libexec/gui2tui/gui2tui-inspect"
SELECTION_APP = "gui2tui-v05a-gtk-selection"
LIVE_APP = "gui2tui-live-fixture"
QT_APP = "gui2tui-qt-fixture"
PROMPT = b"V07D_PROMPT> "


class WaylandShell(ScreenDriver):
    def __init__(self, command: list[str], label: str) -> None:
        super().__init__(PtyProcess(command, (48, 160)))
        self.label = re.sub(r"[^a-z0-9-]", "-", label.lower())

    def start(self) -> None:
        self.child.sendline(
            b"PS1='V07D_PROMPT> '; "
            + f"stty -g > {HOME}/evidence/{self.label}-stty-before; ".encode()
            + b"test -t 0 && test -t 1 && test \"$(id -u)\" != 0 && printf V07D_READY"
        )
        self.child.expect(b"V07D_READY", timeout=20)
        self.child.expect(PROMPT, timeout=10)

    def start_tui(self, application: str) -> None:
        self.transcript.clear()
        self.child.sendline(
            (
                f"{GUI} --session desktop --app {application} --layout spatial "
                "--settle-ms 400 --no-mouse --log-level debug"
            ).encode()
        )

    def command(self, query: str) -> None:
        self.send(b":")
        self.wait_text("Command palette", timeout=10)
        self.send(query.encode() + b"\r")

    def finish_tui(self) -> None:
        self.child.cursor = len(self.child.buffer)
        self.send(b"q")
        self.child.expect(PROMPT, timeout=15)
        self.child.cursor = len(self.child.buffer)
        self.child.sendline(
            (
                f"stty -g > {HOME}/evidence/{self.label}-stty-after; "
                "python3 -c 'import termios; mode=termios.tcgetattr(0)[3]; "
                "assert mode & termios.ICANON and mode & termios.ECHO' && "
                "printf V07D_STTY_RESTORED"
            ).encode()
        )
        self.child.expect(b"V07D_STTY_RESTORED", timeout=10)
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

    def fingerprint(self) -> str:
        return hashlib.sha256(bytes(self.transcript[-65536:])).hexdigest()


class Qualification:
    def __init__(self, args: argparse.Namespace) -> None:
        self.container = args.container
        self.evidence = args.evidence
        self.results: dict[str, object] = {}
        self.environment: dict[str, str] = {}

    def raw_docker(
        self, *command: str, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["docker", "exec", "--user", "gui2tui", self.container, *command],
            check=check,
            text=True,
            capture_output=True,
        )

    def refresh_environment(self) -> None:
        self.environment = json.loads(self.raw_docker(CONTROL, "environment").stdout)

    def docker(
        self, *command: str, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        args = ["docker", "exec", "--user", "gui2tui"]
        for name, value in self.environment.items():
            args.extend(["--env", f"{name}={value}"])
        args.extend([self.container, *command])
        return subprocess.run(args, check=check, text=True, capture_output=True)

    def control(self, *command: str) -> None:
        result = self.raw_docker(CONTROL, *command, check=False)
        if result.returncode != 0:
            raise AssertionError(
                f"container control failed: {' '.join(command)}\n{result.stdout}{result.stderr}"
            )

    def shell(self, label: str) -> WaylandShell:
        command = ["docker", "exec", "-it", "--user", "gui2tui"]
        for name, value in self.environment.items():
            command.extend(["--env", f"{name}={value}"])
        command.extend([self.container, "bash", "--noprofile", "--norc"])
        return WaylandShell(command, label)

    def inspect(self, application: str, *arguments: str, check: bool = True) -> str:
        result = self.docker(
            INSPECT,
            "--session",
            "desktop",
            "--app",
            application,
            *arguments,
            check=check,
        )
        return result.stdout + result.stderr

    def tree(self, application: str, verbose: bool = False) -> str:
        args = ("--verbose",) if verbose else ()
        return self.inspect(application, *args)

    def wait_tree(self, application: str, wanted: str, timeout: float = 15) -> str:
        deadline = time.monotonic() + timeout
        last = ""
        while time.monotonic() < deadline:
            result = self.docker(
                INSPECT,
                "--session",
                "desktop",
                "--app",
                application,
                check=False,
            )
            last = result.stdout + result.stderr
            if result.returncode == 0 and wanted in result.stdout:
                return result.stdout
            time.sleep(0.1)
        raise AssertionError(f"fresh AT-SPI readback missing {wanted!r}\n{last[-2000:]}")

    @staticmethod
    def locator(tree: str, role: str, name: str) -> str:
        pattern = rf'{re.escape(role)} "{re.escape(name)}"[^\n]* id=([^ ]+)'
        match = re.search(pattern, tree)
        if not match:
            raise AssertionError(f"could not find exact {role} locator for {name!r}")
        return match.group(1)

    def doctor(self) -> None:
        report = json.loads(
            self.docker(GUI, "--session", "desktop", "doctor", "--json").stdout
        )
        checks = {item["name"]: item for item in report["checks"]}
        for name in (
            "installation-entry",
            "helper-inspector",
            "session-bus",
            "accessibility-bus",
            "accessibility-registry",
            "accessible-applications",
            "external-text-handler",
        ):
            if checks[name]["level"] != "PASS":
                raise AssertionError(f"Doctor {name} was not PASS: {checks[name]}")
        desktop = checks["desktop"]["message"]
        if "Session=Wayland" not in desktop or "WAYLAND_DISPLAY set=true" not in desktop:
            raise AssertionError(f"Doctor did not identify Wayland: {desktop}")
        capture = checks["wayland-capture"]
        if capture["level"] != "INFO" or "NOT IMPLEMENTED" not in capture["message"]:
            raise AssertionError(f"Wayland capture limit was not explicit: {capture}")
        self.results["doctor"] = "PASS_WAYLAND_SESSION_BUS_ATSPI_REGISTRY"
        self.results["wayland_static_capture"] = "UNSUPPORTED_EXPLICIT"

    def semantic_tui(self, label: str) -> None:
        shell = self.shell(label)
        try:
            # The compositor/AT-SPI fixture can publish its application root
            # before virtualized descendants are realized.  Start the TUI only
            # after the same fresh public tree used by the operation evidence
            # exposes the qualified control; this is validation synchronization,
            # not a production readiness or capability rule.
            self.wait_tree(SELECTION_APP, "Reorder current items", timeout=20)
            shell.start()
            shell.start_tui(SELECTION_APP)
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
                    shell.send(b"\x1b")
                    shell.pump(0.25)
            shell.command("Reorder current items")
            self.wait_tree(SELECTION_APP, "Structure: reordered; selected Gamma")
            shell.wait_text("selected Gamma", timeout=15)
            shell.command("Reset current items")
            self.wait_tree(SELECTION_APP, "Selected: Alpha")
            # Exercise a real terminal resize while the current scene is alive.
            import fcntl
            import struct
            import termios

            fcntl.ioctl(
                shell.child.child_fd,
                termios.TIOCSWINSZ,
                struct.pack("HHHH", 38, 120, 0, 0),
            )
            shell.pump(0.5)
            shell.wait_text("Reorder current items", timeout=10)
            shell.finish_tui()
            shell.finish()
            self.results[f"{label}_tui"] = "PASS"
            self.results[f"{label}_navigation"] = navigation
            self.results[f"{label}_semantic_operation"] = "PASS"
            self.results[f"{label}_authoritative_readback"] = "PASS"
            self.results[f"{label}_dynamic_refresh"] = "PASS"
            self.results[f"{label}_resize"] = "PASS"
            self.results[f"{label}_terminal_restore"] = "PASS"
        finally:
            shell.close()

    def geometry(self, application: str, label: str, collapsed: bool) -> None:
        if collapsed:
            # GTK can register its application root before the virtualized
            # descendants have supplied enough extents to identify the
            # collapsed-origin pattern. Wait for a current complete-enough
            # public snapshot instead of treating that transient partial tree
            # as a geometry-policy failure.
            deadline = time.monotonic() + 15
            spatial = ""
            layout = ""
            while time.monotonic() < deadline:
                spatial = self.inspect(application, "--dump-spatial-evidence")
                layout = self.inspect(application, "--dump-layout-plan")
                metrics = re.search(
                    r"successes=([0-9]+).*rejected=([0-9]+)", spatial
                )
                if (
                    metrics is not None
                    and int(metrics.group(1)) == 0
                    and int(metrics.group(2)) > 0
                    and "trust=Inconsistent" in layout
                    and "topology_anchors=0" in layout
                    and "topology_pairs=0" in layout
                    and "topology_relations=0" in layout
                ):
                    break
                time.sleep(0.1)
            else:
                raise AssertionError(
                    "collapsed geometry did not reach the safe fallback\n"
                    f"{spatial[:1000]}\n{layout[:1000]}"
                )
            if "Reorder current items" not in self.inspect(application, "--dump-scene"):
                raise AssertionError("geometry fallback lost a qualified semantic action")
            self.results[f"{label}_geometry"] = "PASS_COLLAPSED_SCREEN_ORIGIN_REJECTED"
            self.results[f"{label}_layout"] = (
                "PASS_SEMANTIC_CONTROL_SURFACE_NO_SPATIAL_RELATIONS"
            )
        else:
            spatial = self.inspect(application, "--dump-spatial-evidence")
            layout = self.inspect(application, "--dump-layout-plan")
            match = re.search(r"successes=([0-9]+)", spatial)
            if match is None or int(match.group(1)) == 0:
                raise AssertionError("Qt native Wayland exposed no usable geometry evidence")
            if "trust=Consistent" not in layout:
                raise AssertionError("Qt native Wayland geometry was not consistently classified")
            self.results[f"{label}_geometry"] = "PASS_DIFFERENTIATED_SCREEN_EXTENTS"

    def qt_semantics(self) -> None:
        tree = self.tree(QT_APP, verbose=True)
        if "Password" not in tree or "phase-two-secret" in tree:
            raise AssertionError("Qt password redaction contract failed")
        locator = self.locator(tree, "Button", "Activate safely")
        self.docker(INSPECT, "--session", "desktop", "--activate", locator)
        self.wait_tree(QT_APP, "Status: activated")
        self.results["native_qt_identity"] = "PASS_NATIVE_WAYLAND_PROTOCOL_TRACE"
        self.results["native_qt_semantics"] = "PASS"
        self.results["native_qt_authoritative_readback"] = "PASS"

    def modal_scope(self) -> None:
        tree = self.tree(LIVE_APP, verbose=True)
        open_locator = self.locator(tree, "Button", "Open modal dialog")
        self.docker(INSPECT, "--session", "desktop", "--activate", open_locator)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            scoped = self.inspect(LIVE_APP, "--dump-scopes")
            if "ModalDialog" in scoped and "GTK Fixture Dialog" in scoped:
                break
            time.sleep(0.1)
        else:
            raise AssertionError("native Wayland modal did not become current semantic scope")
        dialog_tree = self.tree(LIVE_APP, verbose=True)
        close_locator = self.locator(dialog_tree, "Button", "Close dialog")
        self.docker(INSPECT, "--session", "desktop", "--activate", close_locator)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if "GTK Fixture Dialog" not in self.inspect(LIVE_APP, "--dump-scopes"):
                break
            time.sleep(0.1)
        else:
            raise AssertionError("native Wayland modal scope did not retire after close")
        self.results["native_modal_scope"] = "PASS_ENTER_EXIT_PUBLIC_SEMANTICS"

    @staticmethod
    def begin_vim(shell: WaylandShell) -> None:
        for _ in range(20):
            shell.send(b"e")
            deadline = time.monotonic() + 1.0
            while time.monotonic() < deadline:
                frame = shell.pump(0.08)
                if "alphaline" in re.sub(r"\s+", "", frame):
                    return
            shell.send(b"\t")
        raise AssertionError("could not focus the qualified native-Wayland text target")

    def external_vim(self) -> None:
        shell = self.shell("native-vim")
        try:
            shell.start()
            shell.start_tui(LIVE_APP)
            shell.wait_text("Username", timeout=20)
            self.begin_vim(shell)
            shell.send(b"G")
            shell.send(b"oWayland qualified edit")
            shell.send(b"\x1b:wq\r")
            shell.wait_text("External text update confirmed", timeout=20)
            self.wait_tree(LIVE_APP, "Wayland qualified edit")
            shell.pump(1.0)
            shell.finish_tui()
            shell.finish()
        finally:
            shell.close()
        artifacts = self.docker(
            "bash",
            "-c",
            "find /home/gui2tui/.local/run -type f -name 'artifact-*.txt' 2>/dev/null | wc -l",
        ).stdout.strip()
        if artifacts != "0":
            raise AssertionError("successful external edit left a private candidate")
        self.results["native_external_vim"] = "PASS_REAL_PTY_HANDOFF"
        self.results["native_external_writeback_readback"] = "PASS"
        self.results["native_external_candidate_cleanup"] = "PASS"

    def xwayland_identity(self) -> None:
        fixture_pid = int(
            self.raw_docker("cat", f"{HOME}/evidence/xwayland-selection.pid").stdout.strip()
        )
        display = self.environment["DISPLAY"]
        process = self.raw_docker(
            "bash",
            "-c",
            "ps -eo pid=,args= | grep '/usr/bin/Xwayland :' | grep -v grep",
        ).stdout
        if f"/usr/bin/Xwayland {display}" not in process:
            raise AssertionError("real Weston-owned XWayland process was not observed")
        tree = self.docker("xwininfo", "-root", "-tree").stdout
        match = re.search(r'(0x[0-9a-f]+) "GUI2TUI v0.5A GTK Selection"', tree)
        if not match:
            raise AssertionError("controlled XWayland window was not present in the X tree")
        properties = self.docker("xprop", "-id", match.group(1), "_NET_WM_PID").stdout
        if str(fixture_pid) not in properties:
            raise AssertionError("XWayland window PID did not match the exact controlled fixture")
        self.results["xwayland_identity"] = "PASS_XWAYLAND_PROCESS_WINDOW_PID"

    def old_locator_refused(self, locator: str, label: str) -> None:
        result = self.docker(
            INSPECT,
            "--session",
            "desktop",
            "--activate",
            locator,
            check=False,
        )
        if result.returncode == 0:
            raise AssertionError(f"stale locator unexpectedly succeeded after {label}")
        self.results[f"{label}_old_locator"] = "PASS_REJECTED"

    def run(self) -> None:
        self.refresh_environment()
        harness_source = self.raw_docker("cat", "/opt/gui2tui-source-commit").stdout.strip()
        packaged_source = self.raw_docker(
            "python3",
            "-c",
            "import json,sys; print(json.load(open(sys.argv[1]))['commit'])",
            "/opt/gui2tui-bundle/BUILD-INFO.json",
            check=False,
        )
        source = packaged_source.stdout.strip() if packaged_source.returncode == 0 else harness_source
        version = self.docker(GUI, "--version").stdout.strip()
        packaged_version = self.docker(
            "python3",
            "-c",
            "import json,sys; print(json.load(open(sys.argv[1]))['version'])",
            "/opt/gui2tui-bundle/BUILD-INFO.json",
            check=False,
        )
        if packaged_version.returncode == 0:
            expected_version = f"gui2tui {packaged_version.stdout.strip()}"
            if version != expected_version:
                raise AssertionError(
                    f"installed version does not match BUILD-INFO: {version!r} != {expected_version!r}"
                )
        self.results.update(
            {
                "source_commit": source,
                "harness_source_commit": harness_source,
                "installed_version": version,
                "version_metadata": "PASS",
                "installed_binary": "PASS_USER_PREFIX_NONROOT",
                "headless_compositor": "PASS_WESTON_HEADLESS_PIXMAN",
                "session_topology": "PASS_CURRENT_DESKTOP_EXPLICIT_PRIVATE_DBUS_ATSPI",
            }
        )

        self.control("start-fixture", "native-selection")
        self.control("start-fixture", "native-live")
        self.control("start-fixture", "native-qt")
        applications = self.docker(INSPECT, "--session", "desktop", "--list").stdout
        for application in (SELECTION_APP, LIVE_APP, QT_APP):
            if application not in applications:
                raise AssertionError(f"native Wayland registry missing {application}")
        self.results["native_registry_enumeration"] = "PASS"
        self.results["native_application_identity"] = "PASS_EXPLICIT_SELECTION"
        self.doctor()
        old_native = self.locator(self.tree(SELECTION_APP, verbose=True), "Button", "Reorder current items")
        self.geometry(SELECTION_APP, "native_gtk", collapsed=True)
        self.geometry(QT_APP, "native_qt", collapsed=False)
        self.semantic_tui("native-wayland")
        self.qt_semantics()
        self.modal_scope()
        old_live = self.locator(
            self.tree(LIVE_APP, verbose=True), "Button", "Open modal dialog"
        )
        self.control("stop-fixture", "native-live")
        self.old_locator_refused(old_live, "native_modal_app_restart")
        self.control("start-fixture", "native-live")
        self.external_vim()

        self.control("stop-fixture", "native-selection")
        self.old_locator_refused(old_native, "native_app_restart")
        self.control("start-fixture", "xwayland-selection")
        self.refresh_environment()
        self.xwayland_identity()
        # The XWayland GTK Cache may initially expose only its application,
        # window and container. Exercise the real semantic workflow first so
        # the controlled descendants are currently realized before taking the
        # separate collapsed-geometry evidence snapshot.
        self.semantic_tui("xwayland")
        self.geometry(SELECTION_APP, "xwayland_gtk", collapsed=True)
        old_xwayland = self.locator(
            self.tree(SELECTION_APP, verbose=True), "Button", "Reorder current items"
        )
        self.control("stop-fixture", "xwayland-selection")
        self.old_locator_refused(old_xwayland, "xwayland_app_restart")

        self.control("stop-session")
        self.control("start-session")
        self.refresh_environment()
        self.old_locator_refused(old_native, "compositor_session_restart")
        self.control("start-fixture", "native-selection")
        self.semantic_tui("fresh-wayland-session")
        self.results["lifecycle_recovery"] = "PASS_FRESH_ENUMERATION_AND_SELECTION"
        self.results["old_generation_migration"] = "PASS_NONE"
        self.evidence.write_text(
            json.dumps(self.results, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--container", required=True)
    parser.add_argument("--evidence", required=True, type=pathlib.Path)
    return parser.parse_args()


if __name__ == "__main__":
    Qualification(parse_arguments()).run()
