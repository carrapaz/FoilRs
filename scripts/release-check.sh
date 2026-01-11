#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/release-check.sh [--publish-dry-run] [--release-binary] [--allow-dirty]

Runs local checks that mirror CI and release requirements.
  --publish-dry-run  Run cargo publish --dry-run for both crates.
  --release-binary   Build the FoilRs release binary locally.
  --allow-dirty      Skip the clean working tree check.
EOF
}

publish_dry_run=false
release_binary=false
allow_dirty=false

for arg in "$@"; do
  case "${arg}" in
    --publish-dry-run) publish_dry_run=true ;;
    --release-binary) release_binary=true ;;
    --allow-dirty) allow_dirty=true ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "Unknown argument: ${arg}" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ "${allow_dirty}" == "false" ]]; then
  if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Working tree is not clean. Commit or stash before release checks." >&2
    exit 1
  fi
fi

echo "==> Formatting"
cargo fmt --all --check

echo "==> Clippy"
cargo clippy -p foil_rs --lib --tests -- -D warnings

echo "==> Tests"
cargo test -p foil_rs

echo "==> Bevy frontend check"
cargo check -p foil_rs_bevy

if [[ "${release_binary}" == "true" ]]; then
  echo "==> Release binary build"
  cargo build -p foil_rs_bevy --release --bin FoilRs --locked
fi

if [[ "${publish_dry_run}" == "true" ]]; then
  echo "==> Cargo publish dry-run (foil_rs)"
  cargo publish -p foil_rs --locked --dry-run
  echo "==> Cargo publish dry-run (foil_rs_bevy)"
  cargo publish -p foil_rs_bevy --locked --dry-run
fi
