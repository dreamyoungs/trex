#!/usr/bin/env python3
"""Prepare router training dataset from extraction event logs.

This script creates page-level supervised samples for deciding
`lattice` / `stream` / `blend` strategy.
"""

from __future__ import annotations

import argparse
import csv
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


LABEL_TO_INDEX = {"lattice": 0, "stream": 1, "blend": 2}


@dataclass
class ModeObservation:
    mode: str
    score: float
    ts_unix_ms: int
    features: dict[str, float]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Prepare router dataset")
    parser.add_argument("--events", required=True, help="Input event NDJSON path")
    parser.add_argument(
        "--queue",
        help="Optional hard-case queue NDJSON path. If set, only queued items are used.",
    )
    parser.add_argument("--output-csv", required=True, help="Output dataset CSV path")
    parser.add_argument("--output-meta", required=True, help="Output metadata JSON path")
    parser.add_argument("--min-mode-observations", type=int, default=2)
    parser.add_argument("--blend-margin", type=float, default=0.18)
    return parser.parse_args()


def read_ndjson(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if not path.exists():
        return rows

    with path.open("r", encoding="utf-8") as file:
        for line in file:
            line = line.strip()
            if not line:
                continue
            try:
                parsed = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(parsed, dict):
                rows.append(parsed)
    return rows


def queue_key(tenant_id: str, document_key: str, page: int | None) -> tuple[str, str, int | None]:
    return (tenant_id or "default", document_key, page)


def load_queue_filter(path: Path | None) -> set[tuple[str, str, int | None]]:
    if path is None:
        return set()

    keys: set[tuple[str, str, int | None]] = set()
    for row in read_ndjson(path):
        tenant = str(row.get("tenant_id") or "default")
        document = str(row.get("document_key") or "unknown")
        page = row.get("page")
        page = page if isinstance(page, int) and page >= 1 else None
        keys.add(queue_key(tenant, document, page))
    return keys


def extract_page_metrics(event: dict[str, Any]) -> list[dict[str, Any]]:
    page_metrics = event.get("page_metrics")
    if isinstance(page_metrics, list) and page_metrics:
        normalized: list[dict[str, Any]] = []
        for item in page_metrics:
            if not isinstance(item, dict):
                continue
            normalized.append(
                {
                    "page": item.get("page"),
                    "table_count": item.get("table_count"),
                    "non_empty_cells": item.get("non_empty_cells"),
                    "quality_score": item.get("mean_density"),
                }
            )
        if normalized:
            return normalized

    return [
        {
            "page": None,
            "table_count": event.get("table_count"),
            "non_empty_cells": None,
            "quality_score": event.get("quality_score"),
        }
    ]


def feature_dict(event: dict[str, Any], page_metric: dict[str, Any]) -> dict[str, float]:
    features: dict[str, float] = {}

    raw_page_features = event.get("page_features")
    if isinstance(raw_page_features, dict):
        for key, value in raw_page_features.items():
            if isinstance(value, (int, float)):
                features[str(key)] = float(value)

    if isinstance(page_metric.get("table_count"), int):
        features["obs_table_count"] = float(page_metric["table_count"])
    if isinstance(page_metric.get("non_empty_cells"), int):
        features["obs_non_empty_cells"] = float(page_metric["non_empty_cells"])
    if isinstance(page_metric.get("quality_score"), (int, float)):
        features["obs_quality_score"] = float(page_metric["quality_score"])
    if isinstance(event.get("duration_ms"), (int, float)):
        features["obs_duration_ms"] = float(event["duration_ms"])

    return features


def quality_score(event: dict[str, Any], page_metric: dict[str, Any]) -> float:
    if event.get("status") == "error":
        return -3.0

    score = 0.0
    table_count = page_metric.get("table_count")
    if isinstance(table_count, int):
        score += min(table_count, 8) * 0.35
        if table_count == 0:
            score -= 2.0

    non_empty_cells = page_metric.get("non_empty_cells")
    if isinstance(non_empty_cells, int):
        score += min(non_empty_cells, 80) / 25.0

    density = page_metric.get("quality_score")
    if isinstance(density, (int, float)):
        score += float(density) * 3.0

    feedback_tag = event.get("feedback_tag")
    if feedback_tag in {"missing_table", "bad_extract", "broken_table"}:
        score -= 2.0

    return score


def choose_label(lattice_score: float, stream_score: float, blend_margin: float) -> str:
    gap = abs(lattice_score - stream_score)
    if gap <= blend_margin and max(lattice_score, stream_score) > 0.0:
        return "blend"
    return "lattice" if lattice_score >= stream_score else "stream"


def build_dataset(args: argparse.Namespace) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    events = read_ndjson(Path(args.events))
    queue_filter = load_queue_filter(Path(args.queue) if args.queue else None)
    use_queue_filter = len(queue_filter) > 0

    grouped: dict[tuple[str, str, int | None], list[ModeObservation]] = {}

    for event in events:
        if not bool(event.get("training_opt_in", False)):
            continue

        mode = str(event.get("requested_mode") or "")
        if mode not in {"lattice", "stream", "auto", "dl"}:
            continue

        tenant = str(event.get("tenant_id") or "default")
        document = str(event.get("document_key") or "unknown")
        ts_unix_ms = int(event.get("ts_unix_ms") or 0)

        for metric in extract_page_metrics(event):
            page = metric.get("page")
            page = page if isinstance(page, int) and page >= 1 else None

            key = queue_key(tenant, document, page)
            if use_queue_filter and key not in queue_filter:
                continue

            obs = ModeObservation(
                mode=mode,
                score=quality_score(event, metric),
                ts_unix_ms=ts_unix_ms,
                features=feature_dict(event, metric),
            )

            grouped.setdefault(key, []).append(obs)

    rows: list[dict[str, Any]] = []

    for (tenant, document, page), observations in grouped.items():
        by_mode: dict[str, ModeObservation] = {}
        for observation in observations:
            existing = by_mode.get(observation.mode)
            if existing is None or observation.score > existing.score:
                by_mode[observation.mode] = observation

        lattice_obs = by_mode.get("lattice")
        stream_obs = by_mode.get("stream")
        if lattice_obs is None or stream_obs is None:
            continue

        distinct_modes = len(by_mode)
        if distinct_modes < args.min_mode_observations:
            continue

        preferred_features = None
        for mode_name in ("dl", "auto", "lattice", "stream"):
            obs = by_mode.get(mode_name)
            if obs and obs.features:
                preferred_features = obs.features
                break
        if not preferred_features:
            continue

        label = choose_label(lattice_obs.score, stream_obs.score, args.blend_margin)
        row = {
            "tenant_id": tenant,
            "document_key": document,
            "page": page,
            "label": label,
            "label_index": LABEL_TO_INDEX[label],
            "score_lattice": round(lattice_obs.score, 6),
            "score_stream": round(stream_obs.score, 6),
            "distinct_modes": distinct_modes,
        }
        row.update(preferred_features)
        rows.append(row)

    metadata = {
        "input_events": len(events),
        "grouped_pages": len(grouped),
        "dataset_rows": len(rows),
        "labels": LABEL_TO_INDEX,
        "used_queue_filter": use_queue_filter,
    }

    return rows, metadata


def write_dataset(rows: list[dict[str, Any]], csv_path: Path, meta: dict[str, Any], meta_path: Path) -> None:
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    meta_path.parent.mkdir(parents=True, exist_ok=True)

    if rows:
        base_fields = [
            "tenant_id",
            "document_key",
            "page",
            "label",
            "label_index",
            "score_lattice",
            "score_stream",
            "distinct_modes",
        ]
        dynamic_fields = sorted({key for row in rows for key in row.keys() if key not in base_fields})
        fieldnames = base_fields + dynamic_fields

        with csv_path.open("w", encoding="utf-8", newline="") as file:
            writer = csv.DictWriter(file, fieldnames=fieldnames)
            writer.writeheader()
            for row in rows:
                writer.writerow(row)
    else:
        with csv_path.open("w", encoding="utf-8", newline="") as file:
            writer = csv.writer(file)
            writer.writerow(["tenant_id", "document_key", "page", "label", "label_index"])

    with meta_path.open("w", encoding="utf-8") as file:
        json.dump(meta, file, ensure_ascii=False, indent=2)
        file.write("\n")


def main() -> None:
    args = parse_args()
    rows, meta = build_dataset(args)
    write_dataset(rows, Path(args.output_csv), meta, Path(args.output_meta))

    print(json.dumps({"rows": len(rows), "meta": args.output_meta}, ensure_ascii=False))


if __name__ == "__main__":
    main()
