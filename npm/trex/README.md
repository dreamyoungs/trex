# @dreamyoungs/trex

Node.js wrapper around the TREX CLI.

## Prerequisites

- Node.js 18+
- Recommended: internet access during install (postinstall downloads prebuilt TREX binary)

## Install

```bash
npm install @dreamyoungs/trex
```

By default, package install tries to download a matching TREX binary from GitHub Releases.
If download is unavailable, set either:

- `TREX_BIN=/path/to/trex`
- `options.binPath` in API calls

To skip binary download explicitly:

```bash
TREX_SKIP_DOWNLOAD=1 npm install @dreamyoungs/trex
```

## Maintainer: publish release binaries

When bumping package version, upload matching release assets first:

```bash
scripts/release/publish_assets.sh --version 0.1.0 --upload
```

See `scripts/release/README.md` for details.

## Usage

```js
const { extract, extractCsv } = require("@dreamyoungs/trex");

async function run() {
  const tables = await extract("./invoice.pdf", {
    mode: "auto",
    pages: [1, 2],
  });

  const csv = await extractCsv("./invoice.pdf", {
    mode: "lattice",
  });

  console.log(tables.length, csv.length);
}

run().catch(console.error);
```

## Options

- `pages`: `number[] | string` (e.g. `[1,2,3]` or `"1-3,7"`)
- `mode`: `"auto" | "lattice" | "stream" | "dl"`
- `dlModel`, `dlMinConfidence`, `dlFallback`
- `binPath`: override TREX binary path
- `timeoutMs`: command timeout (default: 120000)
- `eventLog`, `eventDocumentKey`, `eventTenantId`, `eventRequestId`, `eventFeedbackTag`, `eventTrainingOptIn`

## Buffer APIs

- `extractFromBuffer(buffer, options)`
- `extractCsvFromBuffer(buffer, options)`

These methods write a temporary PDF file, run TREX, then clean up automatically.
