"""Same-session alternating baseline/current first-frame measurements."""
import json
import os
import pathlib
import re
import statistics
import time
import pexpect
import pyte

paths = {
    "phase4a_debug": pathlib.Path(os.environ["BASELINE_GUI"]),
    "phase4b_debug": pathlib.Path(os.environ["TARGET_DIR"]) / "debug/gui2tui",
}
if os.environ.get("RELEASE_GUI"):
    paths["phase4b_release"] = pathlib.Path(os.environ["RELEASE_GUI"])
samples = []
for iteration in range(4):
    for strategy, binary in paths.items():
        screen = pyte.Screen(160, 38)
        stream = pyte.Stream(screen)
        started = time.monotonic()
        child = pexpect.spawn(str(binary), ["--app", os.environ["APP_SELECTOR"]], encoding=None, dimensions=(38, 160))
        try:
            while time.monotonic() - started < 20:
                try:
                    stream.feed(child.read_nonblocking(65536, timeout=.02).decode("utf-8", "replace"))
                except pexpect.TIMEOUT:
                    pass
                match = re.search(r"Loaded (\d+) semantic nodes via AT-SPI Cache in (\d+) ms", "\n".join(screen.display))
                if match:
                    break
            else:
                raise AssertionError(f"{strategy}: cache first frame did not arrive")
            samples.append({"strategy": strategy, "iteration": iteration, "warmup": iteration == 0,
                            "nodes": int(match[1]), "bootstrap_ms": int(match[2]),
                            "first_frame_ms": round((time.monotonic() - started) * 1000, 2)})
        finally:
            child.send(b"\x03")
            child.expect(pexpect.EOF, timeout=5)
            child.close()
        time.sleep(.2)
summary = {strategy: {key: round(statistics.median(s[key] for s in samples if s["strategy"] == strategy and not s["warmup"]), 2)
                      for key in ["bootstrap_ms", "first_frame_ms"]} for strategy in paths}
report = {"samples": samples, "warm_medians": summary, "note": "Same live Chrome tree; first run reported as warmup, next three medians. No eager doctor or endpoint probe."}
print(json.dumps(report, indent=2))
if os.environ.get("BENCHMARK_REPORT"):
    pathlib.Path(os.environ["BENCHMARK_REPORT"]).write_text(json.dumps(report, indent=2))
