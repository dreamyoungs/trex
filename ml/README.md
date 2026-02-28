# TREX ML Feedback Loop

This directory contains scripts for turning extraction failures into router model updates.

## Can each team train their own model?

Yes.

- TREX does not enforce a single global model.
- Any user/team can build their own router model from their own event logs.
- Runtime just needs a compatible ONNX file and `--mode dl --dl-model <path>`.
- Model I/O compatibility details are documented in [`MODEL_CONTRACT.md`](MODEL_CONTRACT.md).

## Pipeline

### 1) Collect extraction events

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

### 2) Build hard-case queue

```bash
python3 ml/build_hard_case_queue.py \
  --events logs/extraction_events.ndjson \
  --output ml/artifacts/hard_cases.ndjson
```

Schema: `ml/schemas/hard_case.schema.json`

### 3) Prepare router dataset

```bash
python3 ml/prepare_router_dataset.py \
  --events logs/extraction_events.ndjson \
  --queue ml/artifacts/hard_cases.ndjson \
  --output-csv ml/artifacts/router_dataset.csv \
  --output-meta ml/artifacts/router_dataset.meta.json
```

### 4) Train router model

```bash
python3 -m pip install -r ml/requirements.txt

python3 ml/train_router.py \
  --dataset-csv ml/artifacts/router_dataset.csv \
  --output-dir ml/artifacts/model \
  --onnx-output ml/artifacts/model/router.onnx
```

### 5) Batch update (all-in-one)

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

## Operational Notes

- Keep `training_opt_in=true` only for documents you are allowed to use for model training.
- Do not commit customer PDF files or raw event logs to this repository.
- Store generated models in an external registry (GitHub Releases, S3, or Hugging Face).

## What to version-control vs. what not to

| Commit ✅                             | Do NOT commit ❌                                |
| ------------------------------------- | ----------------------------------------------- |
| Training scripts (`ml/*.py`)          | Raw customer event logs (NDJSON)                |
| Dataset schemas (`ml/schemas/*.json`) | Raw customer PDFs                               |
| Model metadata/manifest               | Temporary training artifacts (`ml/artifacts/*`) |

---

<br>

# 🇰🇷 한국어

이 디렉토리에는 추출 실패를 라우터 모델 업데이트로 전환하는 스크립트가 포함되어 있습니다.

## 팀별로 자체 모델을 학습할 수 있나요?

네.

- TREX는 단일 글로벌 모델을 강제하지 않습니다.
- 모든 사용자/팀이 자체 이벤트 로그로 라우터 모델을 학습할 수 있습니다.
- 런타임에서는 호환 가능한 ONNX 파일과 `--mode dl --dl-model <경로>`만 필요합니다.
- 모델 입출력 호환 규격은 [`MODEL_CONTRACT.md`](MODEL_CONTRACT.md)를 참고하세요.

## 파이프라인

### 1) 추출 이벤트 수집

CLI 이벤트 로깅으로 NDJSON 이벤트를 기록합니다:

```bash
trex extract sample.pdf \
  --mode auto \
  --event-log logs/extraction_events.ndjson \
  --event-document-key "doc-123" \
  --event-tenant-id "tenant-a" \
  --event-request-id "req-001" \
  --event-training-opt-in
```

스키마: `ml/schemas/extraction_event.schema.json`

### 2) 하드 케이스 큐 구성

```bash
python3 ml/build_hard_case_queue.py \
  --events logs/extraction_events.ndjson \
  --output ml/artifacts/hard_cases.ndjson
```

스키마: `ml/schemas/hard_case.schema.json`

### 3) 라우터 데이터셋 준비

```bash
python3 ml/prepare_router_dataset.py \
  --events logs/extraction_events.ndjson \
  --queue ml/artifacts/hard_cases.ndjson \
  --output-csv ml/artifacts/router_dataset.csv \
  --output-meta ml/artifacts/router_dataset.meta.json
```

### 4) 라우터 모델 학습

```bash
python3 -m pip install -r ml/requirements.txt

python3 ml/train_router.py \
  --dataset-csv ml/artifacts/router_dataset.csv \
  --output-dir ml/artifacts/model \
  --onnx-output ml/artifacts/model/router.onnx
```

### 5) 일괄 업데이트 (통합 실행)

```bash
python3 ml/update_router.py \
  --events logs/extraction_events.ndjson \
  --work-dir ml/artifacts/update
```

TREX는 오픈소스이며 상시 실행 서버를 포함하지 않습니다.
이 단계는 다음 방법 중 하나로 실행하세요:

- 로컬/개발 머신에서 수동 실행
- 스케줄 CI (예: GitHub Actions `schedule`)
- 자체 배치 러너 (Airflow, Argo, Jenkins 등)

## 운영 시 유의사항

- `training_opt_in=true`는 모델 학습에 사용해도 되는 문서에만 적용하세요.
- 고객 PDF 파일이나 원본 이벤트 로그를 이 저장소에 커밋하지 마세요.
- 생성된 모델은 외부 레지스트리(GitHub Releases, S3, Hugging Face)에 보관하세요.

## 버전 관리 대상 vs 비대상

| 커밋 ✅                               | 커밋 금지 ❌                        |
| ------------------------------------- | ----------------------------------- |
| 학습 스크립트 (`ml/*.py`)             | 원본 고객 이벤트 로그 (NDJSON)      |
| 데이터셋 스키마 (`ml/schemas/*.json`) | 원본 고객 PDF                       |
| 모델 메타데이터/매니페스트            | 임시 학습 산출물 (`ml/artifacts/*`) |
