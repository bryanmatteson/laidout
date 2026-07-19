#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ -n "$(git status --porcelain)" ]]; then
  echo "release verification requires a clean Laidout worktree" >&2
  exit 1
fi

./scripts/verify.sh

evidence_dir="target/plan-evidence/prepared-kernel"
baseline="$evidence_dir/laidout-baseline.json"
candidate="$evidence_dir/laidout-final.json"
for artifact in "$baseline" "$candidate"; do
  if [[ ! -f "$artifact" ]]; then
    echo "missing release benchmark artifact: $artifact" >&2
    exit 1
  fi
done

cargo bench --bench prepared_kernel --features research -- \
  compare \
  --baseline "$baseline" \
  --candidate "$candidate" \
  --dependency-transition docs/benchmark-dependency-transition-0.2.toml

shasum -a 256 "$baseline" "$candidate"

git diff --check
