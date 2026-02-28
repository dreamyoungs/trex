# Router ONNX Model Contract

This document defines the ONNX input/output contract expected by TREX `--mode dl`.

## Input

- Tensor name: any (TREX uses first input)
- Data type: `float32`
- Shape: `[1, N]`
- Current expected feature order (`N=10`):

1. `text_count`
2. `line_count`
3. `horizontal_count`
4. `vertical_count`
5. `intersection_count`
6. `mean_text_len`
7. `median_text_width`
8. `median_text_height`
9. `text_span_x`
10. `text_span_y`

## Output

- TREX uses the first output tensor.
- Output must contain at least 3 numeric values interpreted as logits:
  - index `0` => `lattice`
  - index `1` => `stream`
  - index `2` => `blend`

## Decision logic in TREX

- TREX applies softmax to output logits.
- It picks the highest-probability class.
- If confidence `< --dl-min-confidence`, TREX falls back to `--dl-fallback` strategy.

## Compatibility check

Recommended smoke test after training/export:

```bash
cargo run --features dl -- extract tests/pdfs/twotables.pdf \
  --mode dl \
  --dl-model path/to/router.onnx \
  --format json
```

If the ONNX shape is incompatible, TREX returns a `DL 에러` with reason.
