# Matching model

Lean model of the limit book. Levels are plain lists, best price first.
Each level is a price and a FIFO list of order ids. There are no tombstones
and no queue positions.

The operations are the corpus dialect: `SubmitLimit`, `SubmitMarket`, and
`Cancel`, with an owner and one of the four self-trade policies. A market
order is a limit at the extreme price with immediate-or-cancel, as in
`submit_market`.

Build and replay one file, from the repository root:

```bash
lake --dir verification/lean build
verification/lean/.lake/build/bin/nanobook-replay \
  input.jsonl output.jsonl
```

`output.jsonl` uses the same bytes as `corpus-replay`. An empty trade list
writes an empty file.

Compare Rust and Lean on seeds 1 through N, 64 events each:

```bash
verification/lean/differential.sh 10000
```
