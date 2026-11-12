#!/usr/bin/env bash
# Recheck the Lean verification of the matching engine with one command.
# Usage: check.sh [SEEDS]   (default 1000 seeded random event logs; 0 skips the
# Rust-versus-Lean replay, for a machine without Rust)
# Needs: lean and lake on PATH; cargo only when SEEDS > 0.
# Steps: build the proofs; reject forbidden constructs; require every law to use
# only propext, Classical.choice and Quot.sound; replay all proofs in the
# independent checker; replay the corpus in Lean; compare Rust and Lean on
# seeded random logs.
set -euo pipefail

LEAN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$LEAN_DIR/../.." && pwd)"
SEEDS="${1:-1000}"

echo "== build"
lake --dir "$LEAN_DIR" build

echo "== forbidden constructs"
if grep -rnwE 'sorry|admit|native_decide|implemented_by|unsafe' "$LEAN_DIR/Nanobook" \
  || grep -rnE '^[[:space:]]*(axiom|partial def)[[:space:]]' "$LEAN_DIR/Nanobook"; then
  echo "forbidden construct found" >&2
  exit 1
fi

echo "== axioms"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
{
  echo 'import Nanobook'
  echo 'open Nanobook'
  for i in 1 2 3 4 5 6 7 8 9 10 11; do echo "#print axioms law${i}_holds"; done
} > "$scratch/Axioms.lean"
lake --dir "$LEAN_DIR" env lean "$scratch/Axioms.lean" | tee "$scratch/axioms.txt"
# Each law may use any subset of the three standard axioms, and nothing else.
awk '
  /does not depend on any axioms/ { laws++; next }
  /depends on axioms:/ {
    laws++
    list = $0
    sub(/.*\[/, "", list); sub(/\].*/, "", list)
    n = split(list, names, /, */)
    for (i = 1; i <= n; i++) {
      if (names[i] != "propext" && names[i] != "Classical.choice" && names[i] != "Quot.sound") {
        print "disallowed axiom " names[i] ": " $0 > "/dev/stderr"; bad = 1
      }
    }
  }
  END {
    if (laws != 11) { print "expected 11 laws, found " laws > "/dev/stderr"; bad = 1 }
    exit bad
  }
' "$scratch/axioms.txt"

echo "== independent kernel check"
lake --dir "$LEAN_DIR" env leanchecker --fresh Nanobook < /dev/null
echo "leanchecker accepted Nanobook"

echo "== corpus replay in Lean"
cases=0
for case in "$ROOT"/oracle-ocaml/test/corpus/*/; do
  # Every case directory must hold both files; a missing one fails the check.
  for f in input.jsonl output.jsonl; do
    if [ ! -f "$case/$f" ]; then
      echo "missing $case$f" >&2
      exit 1
    fi
  done
  "$LEAN_DIR/.lake/build/bin/nanobook-replay" "$case/input.jsonl" "$scratch/lean.jsonl"
  diff -u "$case/output.jsonl" "$scratch/lean.jsonl"
  cases=$((cases + 1))
done
if [ "$cases" -eq 0 ]; then
  echo "no corpus cases found" >&2
  exit 1
fi
echo "corpus matches ($cases cases)"

if [ "$SEEDS" = 0 ]; then
  echo "== differential replay skipped (SEEDS=0)"
else
  echo "== differential replay"
  "$LEAN_DIR/differential.sh" "$SEEDS"
fi
