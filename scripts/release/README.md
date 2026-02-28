# Release Asset Publishing

Use `publish_assets.sh` to build TREX binaries and upload them to GitHub Releases.

## Why this exists

The npm package (`@dreamyoungs/trex`) downloads prebuilt binaries from:

`https://github.com/dreamyoungs/trex/releases/download/v<version>/`

Expected filenames:

- `trex-x86_64-apple-darwin`
- `trex-aarch64-apple-darwin`
- `trex-x86_64-unknown-linux-gnu`
- `trex-aarch64-unknown-linux-gnu`
- `trex-x86_64-pc-windows-msvc.exe`
- `trex-aarch64-pc-windows-msvc.exe`

## Build only

```bash
scripts/release/publish_assets.sh --version 0.1.0
```

Assets are written to:

`target/release-assets/v0.1.0/`

## Build + upload

```bash
scripts/release/publish_assets.sh --version 0.1.0 --upload
```

Requirements for `--upload`:

- `gh` CLI installed
- `gh auth login` completed
- Publish permission on the target repo

## Useful options

- `--strict` : fail immediately on first target failure
- `--skip-build` : upload already-built assets only
- `--target <triple>` : build/upload specific targets only
- `--repo owner/repo` : upload to another repository
