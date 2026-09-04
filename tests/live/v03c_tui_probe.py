#!/usr/bin/env python3
"""Small Phase 0.3C TUI workflow probe; invoked inside one AT-SPI session."""

import os
import pathlib
import re
import stat
import subprocess
import tempfile
import time

import pexpect


root = pathlib.Path(os.environ["PROJECT_ROOT"])
binary = pathlib.Path(os.environ["GUI2TUI"])
mode = os.environ.get("GUI2TUI_VALIDATION_HANDLER_MODE", "positive")
application = os.environ.get("GUI2TUI_VALIDATION_APP", "gui2tui-live-fixture")
with tempfile.TemporaryDirectory(prefix="gui2tui-v03c-config-") as temp:
    config_home = pathlib.Path(temp)
    config_dir = config_home / "gui2tui"
    config_dir.mkdir(mode=0o700)
    handler = root / "tests/fixtures/v03c_text_handler.py"
    config = config_dir / "config.toml"
    config.write_text(
        "version=1\n"
        + (
            ""
            if mode == "nohandler"
            else "[interaction.complex_text]\n"
            "program='python3'\n"
            f"args=[{str(handler)!r},'{{file}}']\n"
        ),
        encoding="utf-8",
    )
    config.chmod(0o600)
    env = os.environ.copy()
    env["XDG_CONFIG_HOME"] = str(config_home)
    child = pexpect.spawn(
        str(binary),
        ["--app", application, "--settle-ms", "200"],
        env=env,
        encoding=None,
        dimensions=(36, 120),
    )
    time.sleep(2)
    if mode != "readonly":
        child.send(b"e")
    if mode == "conflict":
        ready = pathlib.Path(env["GUI2TUI_VALIDATION_HANDLER_READY"])
        deadline = time.monotonic() + 10
        while not ready.exists():
            if time.monotonic() >= deadline:
                raise RuntimeError("handler did not reach conflict barrier")
            time.sleep(0.05)
        tree = subprocess.check_output(
            [os.environ["INSPECT"], "--app", "gui2tui-live-fixture"],
            env=env,
            text=True,
        )
        node_id = next(
            line.rsplit("id=", 1)[1].split()[0]
            for line in tree.splitlines()
            if 'Button "Change external text independently"' in line
        )
        subprocess.run(
            [os.environ["INSPECT"], "--action-name", node_id, "Click"],
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        time.sleep(0.5)
        after = subprocess.check_output(
            [os.environ["INSPECT"], "--app", "gui2tui-live-fixture"],
            env=env,
            text=True,
        )
        assert "GUI concurrent B" in after, after
        pathlib.Path(env["GUI2TUI_VALIDATION_HANDLER_RESUME"]).touch()
    time.sleep(2)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)
    transcript = child.before.decode("utf-8", "replace")
    plain_transcript = " ".join(
        re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", " ", transcript).split()
    )

if mode == "positive":
    assert "External text update confirmed" in plain_transcript, transcript[-4000:]
    print("EXTERNAL_TEXT_END_TO_END=PASS")
elif mode == "conflict":
    assert "External text conflict detected" in plain_transcript, transcript[-4000:]
    authoritative = subprocess.check_output(
        [os.environ["INSPECT"], "--app", "gui2tui-live-fixture"],
        env=env,
        text=True,
    )
    assert "GUI concurrent B" in authoritative, authoritative
    artifacts = list(
        pathlib.Path(env["XDG_RUNTIME_DIR"]).glob(
            "gui2tui/gui2tui-owned-*/operation-*/artifact-*.txt"
        )
    )
    assert len(artifacts) == 1, artifacts
    artifact = artifacts[0]
    assert stat.S_ISREG(artifact.lstat().st_mode)
    assert artifact.lstat().st_mode & 0o077 == 0
    assert artifact.parent.lstat().st_mode & 0o077 == 0
    assert "handler candidate C" in artifact.read_text(encoding="utf-8")
    print("EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS")
elif mode == "fail":
    assert "handler exited unsuccessfully" in plain_transcript, transcript[-4000:]
    authoritative = subprocess.check_output(
        [os.environ["INSPECT"], "--app", "gui2tui-live-fixture"],
        env=env,
        text=True,
    )
    assert "alpha line" in authoritative, authoritative
    assert "handler candidate C" not in authoritative, authoritative
    print("EXTERNAL_TEXT_HANDLER_FAILURE=PASS")
elif mode == "readonly":
    assert "Edit with configured handler" not in plain_transcript, transcript[-4000:]
    print("EXTERNAL_TEXT_READ_ONLY=PASS")
elif mode == "nohandler":
    assert "Edit handler not configured" in plain_transcript, transcript[-4000:]
    print("EXTERNAL_TEXT_NO_HANDLER=PASS")
