# Public API baselines

Generated snapshots of the exported Rust surface for each workspace crate. They
exist so that a change to what nanobook exports shows up as a text diff during
code review, rather than being noticed after release. See [SEMVER.md](../../SEMVER.md)
for what they do and do not promise — they are a review aid, not a 1.0 stability
contract.

## Regenerating

```sh
cargo install cargo-public-api --locked
for c in nanobook nanobook-broker nanobook-risk nanobook-rebalancer nanobook-python; do
    cargo public-api -p "$c" --all-features --simplified > "docs/public-api/$c.txt"
done
```

`--all-features` matters: without it the feature-gated modules (`portfolio`,
`scenarios`, `persistence`, and the broker backends) are missing entirely.

Regenerate as part of release preparation — the step is in
[RELEASING.md](../../RELEASING.md). A stale baseline is worse than none,
because it reads as a guarantee while describing an API that has moved on.

## Provenance

Last regenerated 2026-08-05 for the 0.18.0 bump, with `cargo-public-api` 0.52.0
on rustc 1.97.1 — the same tool version as the previous run, so the diff is the
real API change and not rendering churn. Only `nanobook.txt` moved: `Shares`
arrives with its full surface, `Position::quantity` and `Position::apply_fill`
change type from `i64`, and `BacktestBridgeOptions` gains the execution
constraints. Nothing else was removed.

Record the tool version when you regenerate. The output format is not stable
across `cargo-public-api` releases: the 0.52 line above dropped argument names
from function signatures and renders standard-library paths differently
(`core::io::error::Result` where an earlier version wrote
`std::io::error::Result`). Mixing versions produces diffs full of churn that
hide the real change. The regeneration that produced these files moved roughly
1,300 lines while removing nothing — the growth is `scenarios` and `indicators`
arriving in v0.17.0, plus that rendering change.
