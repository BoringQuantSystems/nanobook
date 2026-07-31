# Releasing

## Prerequisites

1. Add `CARGO_REGISTRY_TOKEN` to GitHub repo secrets:
   - Get token from https://crates.io/settings/tokens
   - Add to repo: Settings → Secrets → Actions → New secret

## Release Process

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml  # Change version = "0.1.0" to "0.2.0" etc.

# 2. Refresh the public API baselines and read the diff
#    This is the step that makes an unintended export change visible before
#    it ships. Skipping it is how they went eight releases out of date.
for c in nanobook nanobook-broker nanobook-risk nanobook-rebalancer nanobook-python; do
    cargo public-api -p "$c" --all-features --simplified > "docs/public-api/$c.txt"
done
git diff docs/public-api/   # anything surprising here belongs in CHANGELOG.md

# 3. Commit the version bump
git add Cargo.toml docs/public-api/
git commit -m "Release v0.2.0"

# 4. Create and push tag
git tag v0.2.0
git push origin main
git push origin v0.2.0

# 5. Free local disk (target/ is gitignored; it only lives on this machine)
cargo clean
```

GitHub Actions will automatically:
- Build binaries for 6 platforms (Linux, macOS, Windows)
- Create GitHub Release with downloadable binaries
- Publish to crates.io
- Build and publish Python wheels to PyPI

Step 5 is local hygiene only. CI runners already discard their workspace after the job.
`cargo clean` does not affect the release artifacts that Actions built.

## Benchmark Baselines

To maintain performance across releases, capture a baseline for major versions:

```bash
# Capture v0.5 baseline
cargo bench --save-baseline v0.5
```

CI will store these baselines as artifacts to compare performance in future PRs.

## Installation Methods

After release, users can install via:

```bash
# Python (PyPI)
pip install nanobook

# Rust (crates.io - compiles from source)
cargo install nanobook
```
