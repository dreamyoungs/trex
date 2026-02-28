# TREX NAPI-RS Node.js Bindings

Native Node.js bindings powered by NAPI-RS.

## Build

```bash
cd bindings/node
npm install
npm run build
```

## Smoke test

```bash
cd bindings/node
npm test
```

## Use

```js
const trex = require("./index.js");

const tables = trex.extract("../../tests/pdfs/twotables.pdf", {
  mode: "Auto",
});

console.log(tables.length);
```

## Exported APIs

- `extract(pdfPath, options?)`
- `extractCsv(pdfPath, options?)`
- `extractFromBuffer(pdfBuffer, options?)`
- `extractCsvFromBuffer(pdfBuffer, options?)`

## Notes

- This package is native and requires build tooling on unsupported platforms.
- The existing CLI-wrapper package is still available at `npm/trex`.
- Release checklist and v0.1.0 notes are in `RELEASE_NOTES.md`.
