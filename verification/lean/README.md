# Matching model

Lean model of the limit book, with eleven laws proved about it (`Nanobook/Laws.lean`,
proofs in `Nanobook/Proofs/`). In `Nanobook/Spec.lean`, levels are plain lists, best
price first, and each level is a price and a FIFO list of order ids.
`Nanobook/Queue.lean` models the queue the Rust engine actually uses (tombstones,
stored positions, `popped_from_front`, compaction), and law 10 proves it always
holds the same orders, in the same order, as the plain lists.

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

## Recheck everything

From the repository root:

```bash
verification/lean/check.sh 1000
```

It builds the proofs of the eleven laws in `Nanobook/Laws.lean`, rejects `sorry`
and other unchecked constructs, requires every law to depend only on
`propext`, `Classical.choice` and `Quot.sound`, replays every declaration in
`leanchecker --fresh`, replays the corpus in Lean, and compares the Rust and
Lean engines on 1000 seeded random event logs. `check.sh 0` runs every Lean step
and skips the Rust comparison, for a machine without Rust. CI runs the same script
(`.github/workflows/lean.yml`). A change to matching, cancel or fill-or-kill
code in `src/` must keep this script passing; if the behaviour change is
intended, change `Nanobook/Spec.lean` to match and repair the proofs, without
weakening a law.
