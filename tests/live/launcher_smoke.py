#!/usr/bin/env python3
"""Live Linux proof for saved launcher -> AT-SPI registration -> TUI."""

import os
import pathlib
import signal
import subprocess
import sys
import tempfile

import pexpect


def main() -> int:
    root = pathlib.Path(__file__).resolve().parents[2]
    binary = root / "target/debug/gui2tui"
    inspector = root / "target/debug/gui2tui-inspect"
    fixture = root / "tests/fixtures/gtk4_live_fixture.py"
    with tempfile.TemporaryDirectory(prefix="gui2tui-launcher-") as temp:
        env = os.environ.copy()
        env["HOME"] = temp
        env["XDG_CONFIG_HOME"] = str(pathlib.Path(temp) / "config")
        wizard = pexpect.spawn(
            str(binary),
            ["app", "add"],
            env=env,
            encoding="utf-8",
            timeout=10,
        )
        wizard.expect("Executable:")
        wizard.sendline(sys.executable)
        wizard.expect("Launcher name")
        wizard.sendline("gtk-fixture")
        wizard.expect("Expected AT-SPI name")
        wizard.sendline("gui2tui-live-fixture")
        wizard.expect("Extra argument")
        wizard.sendline(str(fixture))
        wizard.expect("Extra argument")
        wizard.sendline("")
        wizard.expect("Registered launcher 'gtk-fixture'")
        wizard.expect(pexpect.EOF)
        wizard.close()
        assert wizard.exitstatus == 0

        child = pexpect.spawn(
            str(binary),
            ["--no-mouse"],
            env=env,
            encoding="utf-8",
            timeout=20,
            dimensions=(30, 110),
        )
        try:
            child.expect("gtk-fixture")
            child.send("\r")
            child.expect("nodes via AT-SPI Cache")
            listed = subprocess.run(
                [inspector, "--list"],
                env=env,
                check=True,
                text=True,
                capture_output=True,
            )
            assert "gui2tui-live-fixture" in listed.stdout
            child.send("q")
            child.expect(pexpect.EOF)
            child.close()
            assert child.exitstatus == 0

            direct = pexpect.spawn(
                str(binary),
                ["launch", "gtk-fixture", "--no-mouse"],
                env=env,
                encoding="utf-8",
                timeout=20,
                dimensions=(30, 110),
            )
            direct.expect("nodes via AT-SPI Cache")
            direct.send("q")
            direct.expect(pexpect.EOF)
            direct.close()
            assert direct.exitstatus == 0
        finally:
            if child.isalive():
                child.close(force=True)
            # The launcher deliberately does not own application lifetime.
            # Clean only this uniquely named fixture from the isolated session.
            processes = subprocess.run(
                ["pgrep", "-f", str(fixture)], text=True, capture_output=True
            )
            for line in processes.stdout.splitlines():
                pid = int(line)
                if pid != os.getpid():
                    os.kill(pid, signal.SIGTERM)

    print(
        "LAUNCHER_LIVE=PASS wizard=true registered=true selector_launch=true "
        "direct_launch=true atspi=true tui=true"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
