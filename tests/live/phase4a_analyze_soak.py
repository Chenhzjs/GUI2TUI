"""Contents-free soak resource analysis; wall-clock time cannot be synthesized."""
import json
import pathlib
import statistics
import sys

root = pathlib.Path(sys.argv[1])
summary = json.loads((root / "soak-summary.json").read_text())
samples = [json.loads(line) for line in (root / "soak-samples.jsonl").read_text().splitlines()]
assert summary["wall_seconds"] >= 1800
assert samples[-1]["elapsed_seconds"] >= 1800
assert summary["generations"] >= 20
assert len(samples) >= 30

def trend(rows):
    x = [row["elapsed_seconds"] / 60 for row in rows]
    y = [row["rss_kib"] for row in rows]
    mx, my = statistics.mean(x), statistics.mean(y)
    variance = sum((value - mx) ** 2 for value in x)
    slope = sum((a - mx) * (b - my) for a, b in zip(x, y)) / variance
    residual = sum((b - my - slope * (a - mx)) ** 2 for a, b in zip(x, y))
    total = sum((b - my) ** 2 for b in y)
    return {"rss_kib_per_minute": round(slope, 3), "r_squared": round(1 - residual / total, 4) if total else 1}

warm = [row for row in samples if row["elapsed_seconds"] >= 300]
late = [row for row in samples if row["elapsed_seconds"] >= 900]
report = {
    "wall_seconds": summary["wall_seconds"], "sample_count": len(samples),
    "generations": summary["generations"], "counters": summary["counters"],
    "first": samples[0], "middle": samples[len(samples)//2], "last": samples[-1],
    "fd_values_after_warmup": sorted({row["fds"] for row in warm}),
    "thread_values_after_warmup": sorted({row["threads"] for row in warm}),
    "warm_rss_trend": trend(warm), "late_rss_trend": trend(late),
    "rss_min_kib": min(row["rss_kib"] for row in samples),
    "rss_max_kib": max(row["rss_kib"] for row in samples),
}
for metric in ["active_operations", "temporary_artifacts", "owned_artifact_files", "event_queue"]:
    assert samples[-1][metric] == 0, (metric, samples[-1][metric])
print(json.dumps(report, indent=2))
# RSS trend is evidence for review, not an arbitrary universal pass threshold.
