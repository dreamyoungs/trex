#!/usr/bin/env python3
"""Train a lightweight router model from prepared dataset CSV.

Expected label space:
  0 -> lattice
  1 -> stream
  2 -> blend
"""

from __future__ import annotations

import argparse
import csv
import json
import pickle
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Train TREX router model")
    parser.add_argument("--dataset-csv", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--min-accuracy", type=float, default=0.65)
    parser.add_argument("--test-size", type=float, default=0.2)
    parser.add_argument("--random-state", type=int, default=42)
    parser.add_argument("--onnx-output", help="Optional ONNX output path")
    return parser.parse_args()


def load_dataset(path: Path) -> tuple[list[list[float]], list[int], list[str]]:
    rows = []
    labels = []

    with path.open("r", encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file)
        fieldnames = reader.fieldnames or []
        excluded = {
            "tenant_id",
            "document_key",
            "page",
            "label",
            "label_index",
            "score_lattice",
            "score_stream",
            "distinct_modes",
        }
        feature_names = [name for name in fieldnames if name not in excluded]

        for record in reader:
            try:
                label = int(record.get("label_index", ""))
            except ValueError:
                continue

            features = []
            valid = True
            for name in feature_names:
                value = record.get(name, "")
                try:
                    features.append(float(value))
                except ValueError:
                    valid = False
                    break

            if not valid:
                continue

            rows.append(features)
            labels.append(label)

    return rows, labels, feature_names


def main() -> None:
    args = parse_args()

    try:
        import numpy as np
        from sklearn.linear_model import LogisticRegression
        from sklearn.metrics import accuracy_score, f1_score
        from sklearn.model_selection import train_test_split
    except ImportError as error:
        raise SystemExit(
            "Missing Python dependencies. Install ml/requirements.txt first. "
            f"(detail: {error})"
        )

    data, labels, feature_names = load_dataset(Path(args.dataset_csv))
    if len(data) < 30:
        raise SystemExit(f"Not enough training rows: {len(data)} (need >= 30)")

    x = np.asarray(data, dtype=np.float32)
    y = np.asarray(labels, dtype=np.int64)

    x_train, x_test, y_train, y_test = train_test_split(
        x,
        y,
        test_size=args.test_size,
        random_state=args.random_state,
        stratify=y if len(set(labels)) > 1 else None,
    )

    model = LogisticRegression(
        max_iter=1000,
        multi_class="multinomial",
        class_weight="balanced",
        random_state=args.random_state,
    )
    model.fit(x_train, y_train)

    y_pred = model.predict(x_test)
    accuracy = float(accuracy_score(y_test, y_pred))
    f1_macro = float(f1_score(y_test, y_pred, average="macro"))

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    model_pickle = output_dir / "router.pkl"
    with model_pickle.open("wb") as file:
        pickle.dump(
            {
                "model": model,
                "feature_names": feature_names,
                "label_map": {"lattice": 0, "stream": 1, "blend": 2},
            },
            file,
        )

    metrics = {
        "rows": len(data),
        "features": len(feature_names),
        "accuracy": accuracy,
        "f1_macro": f1_macro,
        "min_accuracy_gate": args.min_accuracy,
        "passed": accuracy >= args.min_accuracy,
        "model_pickle": str(model_pickle),
    }

    if args.onnx_output:
        try:
            from skl2onnx import convert_sklearn
            from skl2onnx.common.data_types import FloatTensorType

            initial_types = [("input", FloatTensorType([None, len(feature_names)]))]
            onnx_model = convert_sklearn(model, initial_types=initial_types)
            onnx_path = Path(args.onnx_output)
            onnx_path.parent.mkdir(parents=True, exist_ok=True)
            onnx_path.write_bytes(onnx_model.SerializeToString())
            metrics["onnx_model"] = str(onnx_path)
        except ImportError:
            metrics["onnx_model"] = None
            metrics["onnx_export_warning"] = "skl2onnx not installed"

    metrics_path = output_dir / "metrics.json"
    with metrics_path.open("w", encoding="utf-8") as file:
        json.dump(metrics, file, ensure_ascii=False, indent=2)
        file.write("\n")

    print(json.dumps(metrics, ensure_ascii=False))

    if accuracy < args.min_accuracy:
        raise SystemExit(
            f"Accuracy gate failed: accuracy={accuracy:.4f} < min_accuracy={args.min_accuracy:.4f}"
        )


if __name__ == "__main__":
    main()
