#!/usr/bin/env python3
"""Compare saved terminal_facade_bench executables with alternating paired runs.

Build both executables from identical source before running this script. Each
case runs in a separate process; no builds or other benchmarks should overlap.
The optional threshold applies to each case's median candidate/baseline paired
throughput ratio. Damage consumption exercises tracking, not presentation.
The baseline must report Alacritty or the custom engine; other labels are rejected.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys


CASES = ("plain", "styled", "unicode", "varied-unicode", "combining", "varied-combining", "fragmented")


def executable(path):
    path = path.resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise argparse.ArgumentTypeError(f"not an executable file: {path}")
    return path


def fingerprint(path):
    digest = hashlib.sha256()
    with path.open("rb") as binary:
        for block in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(block)
    return {"path": str(path), "sha256": digest.hexdigest()}


def measure(binary, label, case, pair, order, args):
    command = [str(binary), str(args.mib), case]
    if args.consume_damage:
        command.append("--consume-damage")
    env = os.environ.copy()
    env.pop("TERMY_CORE_TEST_BACKEND", None)
    if label == "baseline":
        env["TERMY_CORE_TEST_BACKEND"] = "alacritty"
    result = subprocess.run(
        command, env=env, capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=args.timeout, check=False,
    )
    if result.returncode:
        raise RuntimeError(
            f"{label}/{case}/pair {pair}: exit {result.returncode}\n"
            f"{result.stdout[-4000:]}{result.stderr[-4000:]}"
        )
    engine_labels = re.findall(r"^engine: ([a-z-]+)$", result.stdout, re.MULTILINE)
    allowed_engines = ("alacritty", "custom") if label == "baseline" else ("custom",)
    if len(engine_labels) != 1 or engine_labels[0] not in allowed_engines:
        raise RuntimeError(
            f"{label}/{case}/pair {pair}: expected engine in {allowed_engines}, got {engine_labels}"
        )
    pattern = rf"^{re.escape(case)}: ([0-9.]+) MiB/s; ([0-9]+) bytes; ([0-9.]+) seconds$"
    matches = re.findall(pattern, result.stdout, re.MULTILINE)
    if len(matches) != 1:
        raise RuntimeError(
            f"{label}/{case}/pair {pair}: expected one throughput record\n"
            f"{result.stdout[-4000:]}{result.stderr[-4000:]}"
        )
    throughput, byte_count, seconds = matches[0]
    throughput, byte_count, seconds = float(throughput), int(byte_count), float(seconds)
    if (not math.isfinite(throughput) or throughput <= 0 or
            not math.isfinite(seconds) or seconds <= 0 or byte_count < args.mib * 1024**2):
        raise RuntimeError(f"{label}/{case}/pair {pair}: invalid throughput record")
    return {
        "label": label, "case": case, "pair": pair, "order": order,
        "engine": engine_labels[0],
        "mib_per_second": throughput, "bytes": byte_count, "seconds": seconds,
        "stdout": result.stdout, "stderr": result.stderr,
    }


def summarize(samples):
    def stats(values):
        return {"median": statistics.median(values), "min": min(values), "max": max(values)}

    summaries = {}
    for case in CASES:
        by_label = {
            label: {sample["pair"]: sample for sample in samples
                    if sample["case"] == case and sample["label"] == label}
            for label in ("baseline", "candidate")
        }
        pairs = sorted(by_label["baseline"].keys() & by_label["candidate"].keys())
        if not pairs:
            continue
        values = {
            label: [by_label[label][pair]["mib_per_second"] for pair in pairs]
            for label in by_label
        }
        ratios = [candidate / baseline for baseline, candidate in
                  zip(values["baseline"], values["candidate"])]
        summaries[case] = {
            "completed_pairs": len(pairs),
            "baseline_mib_per_second": stats(values["baseline"]),
            "candidate_mib_per_second": stats(values["candidate"]),
            "paired_ratios": [{"pair": pair, "ratio": ratio} for pair, ratio in zip(pairs, ratios)],
            "ratio": stats(ratios),
            "ratio_of_medians": statistics.median(values["candidate"]) / statistics.median(values["baseline"]),
        }
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=lambda value: executable(Path(value)), required=True)
    parser.add_argument("--candidate", type=lambda value: executable(Path(value)), required=True)
    parser.add_argument("--mib", type=int, default=32)
    parser.add_argument("--pairs", type=int, default=6)
    parser.add_argument("--consume-damage", action="store_true")
    parser.add_argument("--output", type=Path, help="JSON output file; defaults to stdout")
    parser.add_argument("--timeout", type=float, default=120, help="seconds per executable/case (default: 120)")
    parser.add_argument("--min-ratio", type=float, help="fail if a case's median paired throughput ratio is lower")
    args = parser.parse_args()
    if not 1 <= args.mib <= 4096 or args.pairs < 1:
        parser.error("--mib must be in 1..=4096 and --pairs must be positive")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be finite and positive")
    if args.min_ratio is not None and (not math.isfinite(args.min_ratio) or args.min_ratio <= 0):
        parser.error("--min-ratio must be finite and positive")
    if args.output:
        if args.output.resolve() in (args.baseline, args.candidate):
            parser.error("--output must not overwrite either executable")
        args.output.parent.mkdir(parents=True, exist_ok=True)

    report = {
        "schema_version": 1, "started_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(), "mib": args.mib, "pairs": args.pairs,
        "consume_damage": args.consume_damage, "timeout_seconds": args.timeout,
        "min_ratio": args.min_ratio, "baseline_backend": None,
        "baseline_requested_backend": "alacritty",
        "baseline": fingerprint(args.baseline), "candidate": fingerprint(args.candidate),
        "samples": [], "summary": {}, "status": "running",
    }

    def save():
        report["summary"] = summarize(report["samples"])
        if args.output:
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    try:
        for case in CASES:
            for pair in range(1, args.pairs + 1):
                order = ("baseline", "candidate") if pair % 2 else ("candidate", "baseline")
                pair_samples = []
                for index, label in enumerate(order, start=1):
                    sample = measure(getattr(args, label), label, case, pair, index, args)
                    if report[label].setdefault("engine", sample["engine"]) != sample["engine"]:
                        raise RuntimeError(f"{label}: engine label changed during comparison")
                    if label == "baseline":
                        report["baseline_backend"] = sample["engine"]
                    report["samples"].append(sample)
                    pair_samples.append(sample)
                    print(f"{case} pair {pair}/{args.pairs} {label}: {sample['mib_per_second']:.3f} MiB/s",
                          file=sys.stderr, flush=True)
                if pair_samples[0]["bytes"] != pair_samples[1]["bytes"]:
                    raise RuntimeError(f"{case}/pair {pair}: binaries processed different byte counts")
                save()
        report["status"] = "completed"
        report["failed_cases"] = [
            case for case, summary in report["summary"].items()
            if args.min_ratio is not None and summary["ratio"]["median"] < args.min_ratio
        ]
        if report["failed_cases"]:
            report["status"] = "failed"
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        report["status"] = "error"
        report["error"] = str(error)
        print(report["error"], file=sys.stderr)
    finally:
        save()
        if not args.output:
            print(json.dumps(report, indent=2))
    for case, summary in report["summary"].items():
        print(f"{case}: median paired ratio {summary['ratio']['median']:.3f}x", file=sys.stderr)
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    sys.exit(main())
