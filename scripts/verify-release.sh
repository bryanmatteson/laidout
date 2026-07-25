#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ -n "$(git status --porcelain)" ]]; then
  echo "release verification requires a clean Laidout worktree" >&2
  exit 1
fi

./scripts/verify.sh

evidence_dir="$(mktemp -d "target/release-evidence.XXXXXX")"
baseline="$evidence_dir/laidout-baseline.json"
candidate="$evidence_dir/laidout-final.json"

cargo bench --bench prepared_kernel --features research -- \
  capture \
  --role baseline \
  --implementation laidout-0.1-baseline \
  --output "$baseline"

cargo bench --bench prepared_kernel --features research -- \
  capture \
  --role final \
  --implementation prepared-kernel \
  --output "$candidate"

cargo bench --bench prepared_kernel --features research -- \
  compare \
  --baseline "$baseline" \
  --candidate "$candidate" \
  --dependency-transition docs/benchmark-dependency-transition-0.3.toml

shasum -a 256 "$baseline" "$candidate"
echo "release evidence retained in $evidence_dir"

git diff --check
