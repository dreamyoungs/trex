#!/usr/bin/env python3
"""Run router update pipeline.

Pipeline:
1) Build hard-case queue from event logs
2) Prepare supervised router dataset
3) Train/evaluate router model
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run TREX router update pipeline")
    parser.add_argument("--events", required=True, help="Input extraction event NDJSON")
    parser.add_argument("--work-dir", default="ml/artifacts/update")
    parser.add_argument("--queue-min-priority", type=float, default=6.0)
    parser.add_argument("--queue-limit", type=int, default=2000)
    parser.add_argument("--min-accuracy", type=float, default=0.65)
    parser.add_argument("--onnx-output", help="Optional ONNX output path")
    return parser.parse_args()


def run_command(command: list[str]) -> None:
    print("$", " ".join(command))
    subprocess.run(command, check=True)


def main() -> None:
    args = parse_args()

    python = sys.executable
    work_dir = Path(args.work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    queue_path = work_dir / "hard_cases.ndjson"
    dataset_csv = work_dir / "router_dataset.csv"
    dataset_meta = work_dir / "router_dataset.meta.json"
    model_dir = work_dir / "model"
    onnx_output = Path(args.onnx_output) if args.onnx_output else model_dir / "router.onnx"

    run_command(
        [
            python,
            "ml/build_hard_case_queue.py",
            "--events",
            args.events,
            "--output",
            str(queue_path),
            "--min-priority",
            str(args.queue_min_priority),
            "--limit",
            str(args.queue_limit),
        ]
    )

    run_command(
        [
            python,
            "ml/prepare_router_dataset.py",
            "--events",
            args.events,
            "--queue",
            str(queue_path),
            "--output-csv",
            str(dataset_csv),
            "--output-meta",
            str(dataset_meta),
        ]
    )

    run_command(
        [
            python,
            "ml/train_router.py",
            "--dataset-csv",
            str(dataset_csv),
            "--output-dir",
            str(model_dir),
            "--min-accuracy",
            str(args.min_accuracy),
            "--onnx-output",
            str(onnx_output),
        ]
    )

    summary = {
        "events": args.events,
        "queue": str(queue_path),
        "dataset_csv": str(dataset_csv),
        "dataset_meta": str(dataset_meta),
        "model_dir": str(model_dir),
        "onnx_output": str(onnx_output),
    }

    summary_path = work_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"ok": True, "summary": str(summary_path)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
