"""Wall-clock mixed runtime soak. Default duration is intentionally 30 minutes."""
import csv
import json
import os
import pathlib
import re
import signal
import subprocess
import termios
import time

import pexpect
import pyte

result = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
duration = int(os.environ.get("SOAK_SECONDS", "1805"))
validation_run = duration >= 1800
assert validation_run or os.environ.get("SOAK_ALLOW_SHORT") == "1", \
    "the validation soak cannot be shortened below 30 minutes"
scale = 1.0 if validation_run else max(duration / 1805, 0.02)
selector = "gui2tui-live-fixture"
socket = result / "broker.sock"
env = {**os.environ, "TMPDIR": str(result)}
fixture = None
modality_fixture = None
broker = None
counters = {
    "semantic_actions": 0, "search_content": 0, "modality": 0,
    "reference": 0, "dialog": 0, "editable_text": 0,
    "detach_resume": 0, "resize": 0, "endpoint_restarts": 0,
    "event_overflows": 0,
}


def command(*args, check=True, timeout=30):
    return subprocess.run(args, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, check=check, timeout=timeout, env=env)


def inspect(*args, check=True, timeout=30):
    return command(str(binary / "gui2tui-inspect"), *args, check=check, timeout=timeout)


def start_fixture():
    global fixture
    fixture_log = result.joinpath("fixture.log").open("a")
    fixture = subprocess.Popen(["python3", "tests/fixtures/gtk4_live_fixture.py"],
        env=env, stdout=fixture_log, stderr=fixture_log, start_new_session=True)
    fixture_log.close()
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if selector in inspect("--list", check=False).stdout:
            return
        time.sleep(.1)
    raise AssertionError("GTK fixture unavailable")


def start_broker():
    global broker
    broker = subprocess.Popen([str(binary / "gui2tui-local"), "serve", "--socket", str(socket),
        "--mime", "image/*", "--recording-handler", "--authorization", "once"], env=env,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    for _ in range(4):
        if "broker ready" in broker.stdout.readline():
            return
    raise AssertionError("broker unavailable")


def stop_broker():
    global broker
    if broker is not None and broker.poll() is None:
        broker.send_signal(signal.SIGTERM)
        broker.wait(timeout=5)


screen = pyte.Screen(140, 70)
stream = pyte.Stream(screen)


def pump(child, seconds=.25):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.04).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def status(child):
    # Runtime JSON is intentionally compact but must not be parsed from a
    # clipped small-terminal frame during resize churn.
    child.setwinsize(70, 140)
    screen.resize(70, 140)
    pump(child, .08)
    child.send(b"\x1b[24~")
    frame = pump(child, .25)
    clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
    value = json.loads(clean[clean.index("{"):clean.rindex("}") + 1])
    child.send(b"\x1b")
    pump(child, .08)
    return value


def proc_resources(pid):
    text = pathlib.Path(f"/proc/{pid}/status").read_text()
    return {
        "rss_kib": int(re.search(r"VmRSS:\s+(\d+)", text).group(1)),
        "threads": int(re.search(r"Threads:\s+(\d+)", text).group(1)),
        "fds": len(list(pathlib.Path(f"/proc/{pid}/fd").iterdir())),
    }


def node_for(tree, pattern):
    line = next(line for line in tree.splitlines() if pattern in line)
    return re.search(r"atspi1_[A-Za-z0-9_-]+", line).group()


def artifact_count():
    runtime = pathlib.Path(env["XDG_RUNTIME_DIR"]) / "gui2tui"
    owned = runtime / f"gui2tui-owned-{os.geteuid()}"
    if not owned.exists():
        return 0
    return sum(1 for path in owned.glob("operation-*/*") if path.name.startswith("artifact-"))


start_fixture()
modality_env = {**env, "VISUAL_ONLY": "1"}
modality_log = result.joinpath("modality-fixture.log").open("w")
modality_fixture = subprocess.Popen(
    ["python3", "tests/fixtures/gtk4_modality_fixture.py"], env=modality_env,
    stdout=modality_log, stderr=modality_log, start_new_session=True,
)
modality_log.close()
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    if "python3" in inspect("--list", check=False).stdout:
        break
    time.sleep(.1)
else:
    raise AssertionError("modality fixture unavailable")
start_broker()
child = pexpect.spawn(str(binary / "gui2tui"),
    ["--app", selector, "--modality-socket", str(socket)], env=env,
    encoding=None, dimensions=(70, 140))
samples = []
sample_log = result.joinpath("soak-samples.jsonl").open("w")


def record_sample(sample):
    samples.append(sample)
    sample_log.write(json.dumps(sample, sort_keys=True) + "\n")
    sample_log.flush()
    os.fsync(sample_log.fileno())


started = time.monotonic()
next_sample = started
next_action = started
next_resize = started
next_restart = started + 75 * scale
next_detach = started + 60 * scale
next_endpoint = started + 140 * scale
next_search = started + 45 * scale
next_dialog = started + 240 * scale
next_modality = started + 260 * scale
storm_done = False
generation = 1
resize_sizes = [(24, 80), (50, 160), (35, 100), (70, 140)]
resize_index = 0
try:
    pump(child, 2)
    initial = status(child)
    assert initial["generation"] == 1

    # One real TUI atomic edit; subsequent generations start clean fixtures.
    child.send(b"\r-soak\r")
    pump(child, 1)
    if 'value="alice-soak"' in inspect("--app", selector).stdout:
        counters["editable_text"] += 1

    while time.monotonic() - started < duration:
        now = time.monotonic()
        if now >= next_action:
            tree = inspect("--app", selector).stdout
            button = node_for(tree, 'Button "Activate safely"')
            inspect("--activate", button)
            counters["semantic_actions"] += 1
            child.send(b"\t")
            pump(child, .08)
            next_action += 15 * scale

        if now >= next_resize:
            rows, columns = resize_sizes[resize_index % len(resize_sizes)]
            child.setwinsize(rows, columns)
            screen.resize(rows, columns)
            resize_index += 1
            counters["resize"] += 1
            pump(child, .08)
            next_resize += 30 * scale

        if now >= next_detach:
            before = status(child)
            os.kill(child.pid, signal.SIGUSR1)
            time.sleep(.2)
            tree = inspect("--app", selector).stdout
            inspect("--activate", node_for(tree, 'Button "Activate safely"'))
            counters["semantic_actions"] += 1
            os.kill(child.pid, signal.SIGUSR2)
            pump(child, .5)
            after = status(child)
            assert after["generation"] == before["generation"]
            counters["detach_resume"] += 1
            next_detach += 75 * scale

        if now >= next_search:
            inspect("--app", selector, "--dump-content")
            inspect("--app", selector, "--dump-commands", "--command-query", "Username")
            counters["search_content"] += 2
            next_search += 60 * scale

        if now >= next_dialog:
            tree = inspect("--app", selector).stdout
            inspect("--activate", node_for(tree, 'Button "Open modal dialog"'))
            time.sleep(.3)
            dialog_tree = inspect("--app", selector).stdout
            close = node_for(dialog_tree, 'Button "Close dialog"')
            inspect("--activate", close)
            counters["semantic_actions"] += 2
            counters["dialog"] += 1
            next_dialog += 240 * scale

        if not storm_done and now - started >= 90 * scale:
            tree = inspect("--app", selector).stdout
            inspect("--activate", node_for(tree, 'Button "Run accessibility event storm"'))
            counters["semantic_actions"] += 1
            pump(child, 8)
            storm = status(child)
            assert storm["events"]["resync_requests"] >= 1
            assert "Storm complete: 2000" in inspect("--app", selector).stdout
            counters["event_overflows"] = 1
            storm_done = True

        if now >= next_endpoint:
            stop_broker()
            child.send(b"\x1bOS")
            pump(child, .5)
            child.send(b"\x1b")
            pump(child, .1)
            start_broker()
            child.send(b"\x1bOS")
            pump(child, .5)
            child.send(b"\x1b")
            pump(child, .1)
            counters["endpoint_restarts"] += 1
            next_endpoint += 150 * scale

        if now >= next_modality:
            tree = inspect("--app", "python3", "--verbose").stdout
            image = node_for(tree, 'Image "Architecture diagram"')
            inspect("--app", "python3", "--materialize-modality", image,
                    "--artifact-ttl-secs", "2", "--open-materialized",
                    "--modality-socket", str(socket), timeout=60)
            counters["modality"] += 1
            command(str(binary / "gui2tui-local"), "reference",
                    "--uri", "https://example.invalid/soak.png", "--mime", "image/png",
                    "--kind", "image", "--authorization", "once")
            counters["reference"] += 1
            next_modality += 300 * scale

        if now >= next_restart:
            old_tree = inspect("--app", selector).stdout
            old_node = node_for(old_tree, 'Button "Activate safely"')
            os.killpg(fixture.pid, signal.SIGTERM)
            fixture.wait(timeout=5)
            deadline = time.monotonic() + 8
            while "Application is no longer available" not in pump(child, .2):
                assert time.monotonic() < deadline
            start_fixture()
            assert inspect("--actions", old_node, check=False).returncode != 0
            child.send(b"\x1b[15~")
            pump(child, 1)
            generation += 1
            assert status(child)["generation"] == generation
            next_restart += 75 * scale

        if now >= next_sample:
            runtime = status(child)
            resource = proc_resources(child.pid)
            record_sample({
                "elapsed_seconds": round(now - started, 3),
                "generation": runtime["generation"],
                "rss_kib": resource["rss_kib"], "fds": resource["fds"],
                "threads": resource["threads"],
                "cache_nodes": runtime["cache_nodes"],
                "event_queue": runtime["event_queue_depth"],
                "active_operations": runtime["active_operations"],
                "temporary_artifacts": runtime["temporary_artifacts"],
                "owned_artifact_files": artifact_count(),
                "endpoint_connected": int(socket.exists() and broker.poll() is None),
                "resync_count": runtime["events"]["resync_requests"],
            })
            next_sample += 30 * scale
        pump(child, .15)

    # Finish at idle baseline and take an end sample after TTL cleanup.
    time.sleep(3)
    final = status(child)
    resources = proc_resources(child.pid)
    record_sample({
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "generation": final["generation"], "rss_kib": resources["rss_kib"],
        "fds": resources["fds"], "threads": resources["threads"],
        "cache_nodes": final["cache_nodes"], "event_queue": final["event_queue_depth"],
        "active_operations": final["active_operations"],
        "temporary_artifacts": final["temporary_artifacts"],
        "owned_artifact_files": artifact_count(),
        "endpoint_connected": int(socket.exists() and broker.poll() is None),
        "resync_count": final["events"]["resync_requests"],
    })
    if validation_run:
        assert generation >= 20 and counters["detach_resume"] >= 20
        assert counters["resize"] >= 50 and counters["endpoint_restarts"] >= 10
        assert counters["semantic_actions"] >= 100 and counters["event_overflows"] == 1
        assert counters["search_content"] >= 6 and counters["modality"] >= 3
    else:
        assert generation >= 3 and counters["detach_resume"] >= 2
        assert counters["endpoint_restarts"] >= 2 and counters["modality"] >= 1
        assert counters["event_overflows"] == 1
    assert final["active_operations"] == 0 and final["event_queue_depth"] == 0
    assert final["temporary_artifacts"] == 0 and artifact_count() == 0
    with result.joinpath("soak-samples.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=samples[0].keys())
        writer.writeheader(); writer.writerows(samples)
    result.joinpath("soak-summary.json").write_text(json.dumps({
        "wall_seconds": time.monotonic() - started, "generations": generation,
        "counters": counters, "first": samples[0], "middle": samples[len(samples)//2],
        "last": samples[-1], "sample_count": len(samples),
    }, indent=2))
    print(json.dumps({"SOAK_PASS": validation_run, "SMOKE_PASS": not validation_run,
                      "wall_seconds": time.monotonic() - started,
                      "generations": generation, "counters": counters,
                      "samples": len(samples)}, sort_keys=True))
finally:
    sample_log.close()
    if child.isalive():
        child.send(b"\x03")
        try:
            child.expect(pexpect.EOF, timeout=8)
        except pexpect.TIMEOUT:
            child.close(force=True)
    stop_broker()
    if fixture is not None and fixture.poll() is None:
        os.killpg(fixture.pid, signal.SIGTERM)
        fixture.wait(timeout=5)
    if modality_fixture is not None and modality_fixture.poll() is None:
        os.killpg(modality_fixture.pid, signal.SIGTERM)
        modality_fixture.wait(timeout=5)
