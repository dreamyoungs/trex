# @dreamyoungs/trex-node v0.1.0 Release Notes

## Highlights

- First native Node.js bindings for TREX using NAPI-RS.
- Exposes both file-path and buffer APIs:
  - `extract(pdfPath, options?)`
  - `extractCsv(pdfPath, options?)`
  - `extractFromBuffer(pdfBuffer, options?)`
  - `extractCsvFromBuffer(pdfBuffer, options?)`
- Supports extraction mode options: `Auto`, `Lattice`, `Stream`, `Dl`.
- Supports DL runtime options: `dlModel`, `dlMinConfidence`, `dlFallback`.

## Requirements

- Node.js `>=18`
- Rust toolchain (for local source builds)

## Publish Checklist

1. `cd bindings/node`
2. `npm ci`
3. `npm test`
4. `npm run build`
5. `npm publish --access public`

## Post-Publish Verification

1. `npm view @dreamyoungs/trex-node version`
2. In a clean project: `npm i @dreamyoungs/trex-node`
3. Run a quick extraction to confirm native module loading succeeds.
