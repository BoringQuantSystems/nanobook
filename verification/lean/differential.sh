#!/usr/bin/env bash
# Build the Rust and Lean replayers, then compare seeds 1 through N.
# Usage: differential.sh N
# Each seed writes 64 corpus events. A mismatch prints the seed and the first differing line.
set -euo pipefail

LEAN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$LEAN_DIR/../.." && pwd)"
LENGTH=64
export CARGO_BUILD_JOBS=2

run_seeds() {
  local n="$1"
  local tmp rust_bin gen_bin lean_bin
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  rust_bin="$ROOT/target/release/corpus-replay"
  gen_bin="$ROOT/target/release/examples/corpus_gen"
  lean_bin="$LEAN_DIR/.lake/build/bin/nanobook-replay"
  local seed
  for seed in $(seq 1 "$n"); do
    "$gen_bin" "$seed" "$LENGTH" "$tmp/input.jsonl"
    "$rust_bin" "$tmp/input.jsonl" "$tmp/rust.jsonl"
    "$lean_bin" "$tmp/input.jsonl" "$tmp/lean.jsonl"
    if ! cmp -s "$tmp/rust.jsonl" "$tmp/lean.jsonl"; then
      echo "mismatch seed $seed"
      awk '
        NR == FNR { rust[NR] = $0; rust_n = NR; next }
        {
          if ($0 != rust[FNR]) {
            print "line " FNR
            print "rust: " rust[FNR]
            print "lean: " $0
            exit
          }
        }
        END {
          if (FNR < rust_n) {
            print "line " FNR + 1
            print "rust: " rust[FNR + 1]
            print "lean: "
          }
        }
      ' "$tmp/rust.jsonl" "$tmp/lean.jsonl"
      exit 1
    fi
    if (( seed % 1000 == 0 )); then
      echo "seed $seed" >&2
    fi
  done
  echo "zero mismatches in $n seeds"
}

if [[ "${1:-}" == "--seeds" ]]; then
  if [[ $# -ne 2 ]]; then
    echo "Usage: differential.sh --seeds N" >&2
    exit 2
  fi
  run_seeds "$2"
  exit 0
fi

if [[ $# -ne 1 ]]; then
  echo "Usage: differential.sh N" >&2
  exit 2
fi

N="$1"
cargo build --manifest-path "$ROOT/Cargo.toml" --release --features serde --bin corpus-replay
cargo build --manifest-path "$ROOT/Cargo.toml" --release --example corpus_gen
lake --dir "$LEAN_DIR" build
start=$(date +%s%N)
run_seeds "$N"
end=$(date +%s%N)
elapsed_ms=$(( (end - start) / 1000000 ))
echo "replay ${N} seeds in ${elapsed_ms} ms"
