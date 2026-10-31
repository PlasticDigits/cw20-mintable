#!/usr/bin/env bash
set -euo pipefail
# Validate the actual aggregate PR diff, including inherited prerequisite commits.
# Source checks run whenever classification is uncertain.
source_changes=true
if [[ "${CI_PIPELINE_EVENT:-}" == pull_request ]]; then
  git fetch --no-tags --depth=128 origin main
  if git merge-base FETCH_HEAD HEAD >/dev/null; then
    git diff --check FETCH_HEAD...HEAD
    changed=$(git diff --name-only --no-renames FETCH_HEAD...HEAD)
    if ! printf '%s\n' "$changed" | grep -Evq '^(docs/.*|[^/]+\.md|LICENSE)$'; then
      source_changes=false
    fi
  fi
fi
if [[ "$source_changes" == false ]]; then
  # Docs-only gate: nonempty tracked Markdown and whitespace validation of exact diff.
  while IFS= read -r path; do
    if [[ "$path" == *.md && -f "$path" ]]; then test -s "$path"; fi
  done <<< "$changed"
  echo 'Docs-only aggregate diff: Markdown presence and git diff whitespace checks passed.'
  exit 0
fi
# Bootstrap main predates the security workspace; preserve a meaningful legacy gate.
if [[ ! -d packages/security-interface || ! -d contracts/security-controller ]]; then
  cargo test --locked
  cargo check --locked
  echo 'Legacy source tests and compile passed; strict workspace gates activate with security v1.'
  exit 0
fi
rustup component add rustfmt clippy
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked --quiet --bin schema
(cd contracts/security-controller && cargo run --locked --quiet --bin security-controller-schema)
git diff --exit-code -- schema contracts/security-controller/schema
echo 'Source tests, strict clippy, formatting and schema reproducibility passed.'
