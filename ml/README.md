# TREX ML Feedback Loop

This directory contains scripts for turning extraction failures into router model updates.

## Can each team train their own model?

Yes.

- TREX does not enforce a single global model.
- Any user/team can build their own router model from their own event logs.
- Runtime just needs a compatible ONNX file and `--mode dl --dl-model <path>`.
- Model I/O compatibility details are documented in `ml/MODEL_CONTRACT.md`.

## 1) Collect extraction events

Use CLI event logging to append NDJSON events:

```bash
trex extract sample.pdf \
  --mode auto \
  --event-log logs/extraction_events.ndjson \
  --event-document-key "doc-123" \
  --event-tenant-id "tenant-a" \
  --event-request-id "req-001" \
  --event-training-opt-in
```

Schema: `ml/schemas/extraction_event.schema.json`

## 2) Build hard-case queue

```bash
python3 ml/build_hard_case_queue.py \
  --events logs/extraction_events.ndjson \
  --output ml/artifacts/hard_cases.ndjson
```

Schema: `ml/schemas/hard_case.schema.json`

## 3) Prepare router dataset

```bash
python3 ml/prepare_router_dataset.py \
  --events logs/extraction_events.ndjson \
  --queue ml/artifacts/hard_cases.ndjson \
  --output-csv ml/artifacts/router_dataset.csv \
  --output-meta ml/artifacts/router_dataset.meta.json
```

## 4) Train router model

```bash
python3 -m pip install -r ml/requirements.txt

python3 ml/train_router.py \
  --dataset-csv ml/artifacts/router_dataset.csv \
  --output-dir ml/artifacts/model \
  --onnx-output ml/artifacts/model/router.onnx
```

## 5) Batch update run

```bash
python3 ml/update_router.py \
  --events logs/extraction_events.ndjson \
  --work-dir ml/artifacts/update
```

TREX is open source and does not ship with an always-on server.
Run this step in one of these ways:

- Manually from your local/dev machine
- Scheduled CI (for example, GitHub Actions `schedule`)
- Your own batch runner (Airflow, Argo, Jenkins, etc.)

## Operational notes

- Keep `training_opt_in=true` only for documents you are allowed to use for model training.
- Do not commit customer PDF files or raw event logs to this repository.
- Store generated models in an external registry (GitHub Releases, S3, or Hugging Face).

## What to version-control vs. what not to

Recommended to commit:

- Training scripts (`ml/*.py`)
- Dataset schema (`ml/schemas/*.json`)
- Model metadata/manifest (version, metrics, hash)

Recommended NOT to commit:

- Raw customer event logs (NDJSON)
- Raw customer PDFs
- Temporary training artifacts (`ml/artifacts/*`)
