#!/usr/bin/env python3
"""Compare saved macOS release binaries with identical terminal workloads.

Uses Termy's opt-in benchmark metrics and records paint counts to reveal window
occlusion. Keeps isolated config, logs,
and raw samples under --output; never reads or changes the user's Termy config.
"""

import argparse
import json
import os
from pathlib import Path
import shlex
import statistics
import subprocess
import sys
import time


def drive(scenario):
    out = sys.stdout.buffer
    time.sleep(1)
    if scenario == "output":
        # Fixed 1,179,648 lines of normal log output, with the normal scrollback limit.
        line = b"build: compiling module 0123456789 abcdefghijklmnopqrstuvwxyz -- completed successfully\r\n"
        chunk = line * 768
        for _ in range(1536):
            out.write(chunk)
            out.flush()
    elif scenario == "styled":
        out.write(b"\x1b[?1049h\x1b[?25l")
        for frame in range(360):
            start = time.monotonic()
            rows = []
            for row in range(38):
                row_text = f"\x1b[{row + 1};1H"
                for col in range(28):
                    color = 31 + (col + row) % 7
                    row_text += f"\x1b[{color}m{(frame + row + col) % 10000:04}"
                rows.append(row_text)
            out.write("".join(rows).encode())
            out.flush()
            time.sleep(max(0, 1 / 60 - (time.monotonic() - start)))
        out.write(b"\x1b[0m")
        out.flush()
    elif scenario != "idle":
        raise ValueError(scenario)
    time.sleep(3 if scenario != "idle" else 7)


def run(binary, label, scenario, iteration, root):
    run_dir = root / f"{scenario}-{iteration}-{label}"
    run_dir.mkdir(parents=True, exist_ok=False)
    config = run_dir / "config" / "termy"
    config.mkdir(parents=True)
    (config / "config.txt").write_text(
        "tmux_enabled = false\nmultiplexer_enabled = false\n"
        "background_blur = false\nbackground_opacity = 1.0\n"
        "cursor_blink = false\nwindow_width = 1280\nwindow_height = 820\n"
        "show_debug_overlay = false\n"
    )
    home = run_dir / "home"
    home.mkdir()
    env = {
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(config.parent),
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "SHELL": "/bin/zsh",
        "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
        "TERMY_INSTANCE_HOME": str(run_dir / "instance"),
        "TERMY_BENCHMARK_COMMAND": shlex.join(
            [sys.executable, str(Path(__file__).resolve()), "--drive", scenario]
        ),
        "TERMY_BENCHMARK_SCENARIO": scenario,
        "TERMY_BENCHMARK_METRICS_PATH": str(run_dir / "metrics"),
        "TERMY_BENCHMARK_DURATION_SECS": "45",
        "TERMY_BENCHMARK_EXIT_ON_COMPLETE": "1",
        "TERMY_BENCHMARK_BUILD_LABEL": label,
    }
    with (run_dir / "app.log").open("w") as log, (run_dir / "time.txt").open("w") as timing:
        subprocess.run(
            ["/usr/bin/time", "-lp", str(binary), "--working-directory", str(home)],
            env=env, stdout=log, stderr=timing, check=True, timeout=65,
        )
    summary = json.loads((run_dir / "metrics" / "summary.json").read_text())
    samples = [json.loads(line) for line in (run_dir / "metrics" / "timeline.ndjson").read_text().splitlines()]
    # Last two seconds give comparable memory after output and rendering settle.
    end = samples[-1]["elapsed_ms"]
    settled = [s["memory_bytes"] for s in samples if s["elapsed_ms"] >= end - 2000]
    timing = (run_dir / "time.txt").read_text().splitlines()
    cpu = {}
    for line in timing:
        fields = line.split()
        if len(fields) == 2 and fields[0] in ("user", "sys", "real"):
            cpu[fields[0]] = float(fields[1])
    result = {
        "label": label, "scenario": scenario, "iteration": iteration,
        "cpu_seconds": cpu["user"] + cpu["sys"],
        "wall_seconds": cpu["real"],
        "sampled_cpu_percent": summary["cpu_avg_percent"],
        "settled_rss_mib": statistics.median(settled) / 1024**2,
        "peak_rss_mib": summary["memory_max_bytes"] / 1024**2,
        "frames": summary["total_frames"],
        "grid_paints": summary["grid_paint_count"],
        "active_rendering_observed": None if scenario == "idle" else summary["grid_paint_count"] >= 10,
    }
    (run_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    if result["active_rendering_observed"] is False:
        print("Window stopped painting: use this run only for background processing, not visible rendering.", file=sys.stderr, flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--scenario", action="append", choices=["idle", "output", "styled"])
    parser.add_argument("--drive", choices=["idle", "output", "styled"])
    args = parser.parse_args()
    if args.drive:
        drive(args.drive)
        return
    if not args.output or not (args.baseline or args.candidate) or args.repeats < 1:
        parser.error("provide --output, a binary, and positive --repeats")
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=True)
    builds = [(label, binary.resolve()) for label, binary in [("baseline", args.baseline), ("candidate", args.candidate)] if binary]
    results = []
    for scenario in args.scenario or ["idle", "output", "styled"]:
        for iteration in range(args.repeats):
            for label, binary in builds if iteration % 2 == 0 else builds[::-1]:
                results.append(run(binary, label, scenario, iteration, root))
    (root / "results.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
