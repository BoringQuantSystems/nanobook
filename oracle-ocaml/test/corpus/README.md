# Golden Corpus (verified)

Byte-identical trade output from the Rust `corpus-replay` binary and the OCaml
`replay_bin` oracle across all 18 cases below. CI runs the differential loop in
`.github/workflows/oracle.yml`.

## Layout

Each case directory has:

- `input.jsonl` — event log to replay
- `output.jsonl` — expected trades (one JSON object per line, trailing newline)

## Event dialect

One JSON object per line. `type` is required; other fields depend on the type.

### SubmitLimit

```json
{"type":"SubmitLimit","side":"BUY","price":10100,"quantity":100,"time_in_force":"GTC"}
```

| Field | Required | Values |
|-------|----------|--------|
| `side` | yes | `"BUY"`, `"SELL"` |
| `price` | yes | integer or decimal string (i64 cents) |
| `quantity` | yes | positive integer |
| `time_in_force` | yes | `"GTC"`, `"IOC"`, `"FOK"` |
| `owner` | no | integer or `null` (STP participant id) |
| `stp_policy` | no | `"Off"` (default), `"CancelNewest"`, `"CancelOldest"`, `"DecrementAndCancel"` |

### SubmitMarket

```json
{"type":"SubmitMarket","side":"BUY","quantity":50}
```

Same optional `owner` and `stp_policy` as SubmitLimit. Market orders use IOC
semantics (sweep best prices, cancel remainder).

### Cancel

```json
{"type":"Cancel","order_id":1}
```

## Trade output dialect

Field order is fixed for byte diffs:

```json
{"id":1,"price":10100,"quantity":50,"aggressor_order_id":2,"passive_order_id":1,"aggressor_side":"BUY","timestamp":3}
```

| Field | Meaning |
|-------|---------|
| `id` | Monotonic trade id from 1 |
| `price` | Resting (passive) order price |
| `quantity` | Fill size |
| `aggressor_order_id` | Incoming order id |
| `passive_order_id` | Resting order id |
| `aggressor_side` | `"BUY"` or `"SELL"` |
| `timestamp` | Monotonic counter from 1; order creation consumes one tick, each trade consumes another |

Order ids and timestamps share one monotonic counter sequence per replay: each
submit allocates the next order id and the next timestamp; each trade allocates
the next timestamp (and trade id separately from 1).

## Cases

| Case | What it exercises |
|------|-------------------|
| 01-simple-cross | Basic limit cross |
| 02-no-cross | Spread, no trade |
| 03-market-order-sweep | Market sweeps multiple levels |
| 04-fok-no-match | FOK rejected, no liquidity |
| 05-fok-partial-cross | FOK rejected, partial liquidity |
| 06-ioc-partial-fill | IOC partial, remainder cancelled |
| 07-multiple-same-price | FIFO at one price |
| 08-cancel-resting | Cancel resting order |
| 09-cancel-partially-filled | Cancel after partial fill |
| 10-fok-full-fill | FOK full fill |
| 11-owner-basic | Owner tags, STP off |
| 12-stp-off | Same owner crosses |
| 13-stp-cancel-newest | STP cancels incoming |
| 14-stp-cancel-oldest | STP cancels resting |
| 15-stp-decrement | STP decrements smaller (incoming) |
| 16-stp-decrement-equal | STP equal qty, resting cancelled |
| 17-min-price | i64 near `MIN` |
| 18-max-price | i64 near `MAX` |

## Run locally

```bash
# OCaml
cd oracle-ocaml
opam exec -- dune build
opam exec -- dune exec bin/replay_bin.exe -- test/corpus/01-simple-cross/input.jsonl /tmp/ocaml.jsonl

# Rust
cargo build --release --features serde --bin corpus-replay
./target/release/corpus-replay oracle-ocaml/test/corpus/01-simple-cross/input.jsonl /tmp/rust.jsonl

diff test/corpus/01-simple-cross/output.jsonl /tmp/ocaml.jsonl
diff test/corpus/01-simple-cross/output.jsonl /tmp/rust.jsonl
```

Full loop (from repo root):

```bash
set -euo pipefail
for case in oracle-ocaml/test/corpus/*/; do
  name=$(basename "$case")
  opam exec -- dune exec --root oracle-ocaml bin/replay_bin.exe -- "$case/input.jsonl" "/tmp/ocaml-$name.jsonl"
  ./target/release/corpus-replay "$case/input.jsonl" "/tmp/rust-$name.jsonl"
  diff -u "$case/output.jsonl" "/tmp/ocaml-$name.jsonl"
  diff -u "$case/output.jsonl" "/tmp/rust-$name.jsonl"
  echo "ok $name"
done
```

## Add case 19

1. Create `oracle-ocaml/test/corpus/19-your-case/` with `input.jsonl`.
2. Run either engine to produce a candidate `output.jsonl`.
3. Run the other engine and `diff` until byte-identical.
4. Add a row to the table above and commit all three files.