#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

DEFAULT_REPO="dreamyoungs/trex"
DEFAULT_TARGETS=(
  "x86_64-apple-darwin"
  "aarch64-apple-darwin"
  "x86_64-unknown-linux-gnu"
  "aarch64-unknown-linux-gnu"
  "x86_64-pc-windows-msvc"
  "aarch64-pc-windows-msvc"
)

VERSION=""
REPO="${GITHUB_REPOSITORY:-$DEFAULT_REPO}"
UPLOAD="0"
SKIP_BUILD="0"
STRICT="0"
OUT_ROOT="${ROOT_DIR}/target/release-assets"
TARGETS=("${DEFAULT_TARGETS[@]}")

usage() {
  cat <<'EOF'
Build and optionally upload TREX release binaries.

Usage:
  scripts/release/publish_assets.sh --version <semver> [options]

Options:
  --version <value>     Required. Example: 0.1.0
  --repo <owner/repo>   GitHub repository for gh release upload (default: dreamyoungs/trex)
  --upload              Upload built assets to GitHub Release tag v<version>
  --skip-build          Skip cargo build step; upload/copy only existing assets
  --strict              Fail immediately if any target build fails
  --out-dir <path>      Output root directory (default: target/release-assets)
  --target <triple>     Add/override target triple (repeatable)
  --help                Show this help

Output asset names:
  trex-<target-triple>            (unix)
  trex-<target-triple>.exe        (windows)

Examples:
  scripts/release/publish_assets.sh --version 0.1.0
  scripts/release/publish_assets.sh --version 0.1.0 --upload
  scripts/release/publish_assets.sh --version 0.1.0 --target x86_64-unknown-linux-gnu --strict
EOF
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

warn() {
  printf 'WARN: %s\n' "$*" >&2
}

info() {
  printf 'INFO: %s\n' "$*"
}

is_windows_target() {
  [[ "$1" == *"windows"* ]]
}

asset_name_for_target() {
  local target="$1"
  if is_windows_target "$target"; then
    printf 'trex-%s.exe' "$target"
  else
    printf 'trex-%s' "$target"
  fi
}

binary_name_for_target() {
  if is_windows_target "$1"; then
    printf 'trex.exe'
  else
    printf 'trex'
  fi
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || fail "--version requires a value"
        VERSION="$2"
        shift 2
        ;;
      --repo)
        [[ $# -ge 2 ]] || fail "--repo requires a value"
        REPO="$2"
        shift 2
        ;;
      --upload)
        UPLOAD="1"
        shift
        ;;
      --skip-build)
        SKIP_BUILD="1"
        shift
        ;;
      --strict)
        STRICT="1"
        shift
        ;;
      --out-dir)
        [[ $# -ge 2 ]] || fail "--out-dir requires a value"
        OUT_ROOT="$2"
        shift 2
        ;;
      --target)
        [[ $# -ge 2 ]] || fail "--target requires a value"
        if [[ "${TARGETS[*]}" == "${DEFAULT_TARGETS[*]}" ]]; then
          TARGETS=()
        fi
        TARGETS+=("$2")
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown option: $1"
        ;;
    esac
  done

  [[ -n "$VERSION" ]] || fail "--version is required"
  [[ "${#TARGETS[@]}" -gt 0 ]] || fail "at least one target is required"
}

ensure_dependencies() {
  command -v cargo >/dev/null 2>&1 || fail "cargo not found"
  command -v rustup >/dev/null 2>&1 || fail "rustup not found"
  if [[ "$UPLOAD" == "1" ]]; then
    command -v gh >/dev/null 2>&1 || fail "gh CLI not found (required for --upload)"
  fi
}

build_assets() {
  local out_dir="$1"
  local failed_targets=()

  mkdir -p "$out_dir"

  for target in "${TARGETS[@]}"; do
    info "Preparing target: ${target}"

    if ! rustup target add "$target" >/dev/null 2>&1; then
      warn "failed to add rust target ${target}"
      if [[ "$STRICT" == "1" ]]; then
        fail "strict mode enabled"
      fi
    fi

    if [[ "$SKIP_BUILD" == "0" ]]; then
      info "Building ${target}"
      if ! cargo build --release --locked --target "$target"; then
        warn "cargo build failed for ${target}"
        failed_targets+=("$target")
        if [[ "$STRICT" == "1" ]]; then
          fail "strict mode enabled"
        fi
        continue
      fi
    fi

    local bin_name
    bin_name="$(binary_name_for_target "$target")"
    local source_path="${ROOT_DIR}/target/${target}/release/${bin_name}"

    if [[ ! -f "$source_path" ]]; then
      warn "missing built binary: ${source_path}"
      failed_targets+=("$target")
      if [[ "$STRICT" == "1" ]]; then
        fail "strict mode enabled"
      fi
      continue
    fi

    local asset_name
    asset_name="$(asset_name_for_target "$target")"
    cp "$source_path" "${out_dir}/${asset_name}"
    info "Prepared asset: ${asset_name}"
  done

  if [[ "${#failed_targets[@]}" -gt 0 ]]; then
    warn "Some targets failed: ${failed_targets[*]}"
  fi
}

ensure_release_exists() {
  local tag="$1"

  if gh release view "$tag" --repo "$REPO" >/dev/null 2>&1; then
    return
  fi

  info "Creating release ${tag} on ${REPO}"
  gh release create "$tag" \
    --repo "$REPO" \
    --title "$tag" \
    --notes "Automated TREX release assets for ${tag}."
}

upload_assets() {
  local tag="$1"
  local out_dir="$2"

  ensure_release_exists "$tag"

  local uploaded_any="0"
  for target in "${TARGETS[@]}"; do
    local asset_name
    asset_name="$(asset_name_for_target "$target")"
    local asset_path="${out_dir}/${asset_name}"

    if [[ ! -f "$asset_path" ]]; then
      warn "skip upload (asset missing): ${asset_name}"
      if [[ "$STRICT" == "1" ]]; then
        fail "strict mode enabled"
      fi
      continue
    fi

    info "Uploading ${asset_name}"
    gh release upload "$tag" "$asset_path" --repo "$REPO" --clobber
    uploaded_any="1"
  done

  if [[ "$uploaded_any" == "0" ]]; then
    fail "no assets uploaded"
  fi
}

main() {
  parse_args "$@"
  ensure_dependencies

  local tag="v${VERSION#v}"
  local out_dir="${OUT_ROOT}/${tag}"

  info "Version: ${VERSION}"
  info "Tag: ${tag}"
  info "Output dir: ${out_dir}"
  info "Targets: ${TARGETS[*]}"

  build_assets "$out_dir"

  if [[ "$UPLOAD" == "1" ]]; then
    upload_assets "$tag" "$out_dir"
    info "Upload complete: ${REPO} ${tag}"
  else
    info "Build complete. To upload later, run with --upload"
  fi

  info "Done"
}

main "$@"
