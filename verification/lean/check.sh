#!/usr/bin/env bash
# Recheck the Lean verification of the matching engine with one command.
# Usage: check.sh [SEEDS]   (default 1000 seeded random event logs)
# Needs: lean and lake on PATH, cargo, and the repository's Rust toolchain.
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
  for i in 1 2 3 4 5 6 7 8 9; do echo "#print axioms law${i}_holds"; done
} > "$scratch/Axioms.lean"
lake --dir "$LEAN_DIR" env lean "$scratch/Axioms.lean" | tee "$scratch/axioms.txt"
expected="depends on axioms: [propext, Classical.choice, Quot.sound]"
if [[ "$(grep -c -F "$expected" "$scratch/axioms.txt")" -ne 9 ]]; then
  echo "a law depends on an axiom outside propext, Classical.choice, Quot.sound" >&2
  exit 1
fi

echo "== independent kernel check"
lake --dir "$LEAN_DIR" env leanchecker --fresh Nanobook < /dev/null
echo "leanchecker accepted Nanobook"

echo "== corpus replay in Lean"
for case in "$ROOT"/oracle-ocaml/test/corpus/*/; do
  [ -f "$case/input.jsonl" ] || continue
  "$LEAN_DIR/.lake/build/bin/nanobook-replay" "$case/input.jsonl" "$scratch/lean.jsonl"
  diff -u "$case/output.jsonl" "$scratch/lean.jsonl"
done
echo "corpus matches"

echo "== differential replay"
"$LEAN_DIR/differential.sh" "$SEEDS"
