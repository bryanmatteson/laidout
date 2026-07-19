#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
output=$(z3 "$script_dir/cost_model_laws.smt2")
printf '%s\n' "$output"

if printf '%s\n' "$output" | grep -Eq '^(sat|unknown)$'; then
    printf '%s\n' "cost-model proof failed: counterexample or unknown result" >&2
    exit 1
fi

checks=$(printf '%s\n' "$output" | grep -c '^unsat$')
if [ "$checks" -ne 13 ]; then
    printf '%s\n' "cost-model proof failed: expected 13 obligations, saw $checks" >&2
    exit 1
fi
