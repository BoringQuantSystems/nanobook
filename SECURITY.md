# Security policy

## Reporting a vulnerability

Please report security problems privately, through GitHub:
**Security → Report a vulnerability** on this repository
([direct link](https://github.com/BoringQuantSystems/nanobook/security/advisories/new)).
Do not open a public issue for a security problem.

Include what you found, how to reproduce it, and which version and features you
used. We aim to acknowledge a report within 7 days and to publish a fix and a
GitHub security advisory once it is ready, crediting you unless you ask otherwise.

## Supported versions

| Version | Security fixes |
|---------|----------------|
| 0.20.x  | yes |
| < 0.20  | no; please upgrade |

nanobook is pre-1.0; see [SEMVER.md](SEMVER.md) for what that means for upgrades.

## Check every claim yourself

Each row is a claim, the command that checks it from a clone of this repository, and the
output we got when preparing 0.20.0.

| Claim | Command | Output for 0.20.0 |
|---|---|---|
| No `unsafe` code in the Rust crates | `grep -rn 'unsafe' src broker/src risk/src rebalancer/src` | the five `#![forbid(unsafe_code)]` lines, one comment and one error message that contain the word; no `unsafe` block |
| One runtime dependency | `cargo tree -p nanobook -e normal --depth 1` | `nanobook` → `rustc-hash` |
| Dependencies pass the policy | `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| No known vulnerable dependency | `cargo audit` | no findings (348 crates scanned) |
| The matching laws are proved | `verification/lean/check.sh 0` | 11 laws on `propext`, `Classical.choice`, `Quot.sound` only; `leanchecker accepted Nanobook`; `corpus matches (25 cases)` |
| The Rust engine matches the model | `verification/lean/differential.sh 1000` | `zero mismatches in 1000 seeds` |
| The tour's numbers are real | `python python/examples/tour.py` | the output shown in the README |
| A release binary was built here | `gh attestation verify <file> --repo BoringQuantSystems/nanobook` | `Verification succeeded` (from 0.20.0 on) |
| The tests catch planted bugs | `cargo mutants` (see VERIFICATION.md for the exact setup) | see "Mutation testing" in VERIFICATION.md |

## What we check, and where you can verify it

Every claim below can be checked in this repository or on a public service.

**The code**

- No `unsafe` code in `nanobook`, `nanobook-broker`, `nanobook-risk` or
  `nanobook-rebalancer`: each crate root has `#![forbid(unsafe_code)]`. The Python
  bindings (`nanobook-python`) call into Python through PyO3, which needs `unsafe`
  by nature; that crate has no hand-written `unsafe` blocks.
- The order-matching rules are proved in Lean 4 on a model of the engine, and CI
  runs the Rust engine against that model on every push
  ([VERIFICATION.md](VERIFICATION.md)).
- Property tests, regression tests, fuzz targets (`fuzz/`) and mutation testing
  (`fuzz/mutants-baseline.md`).

**What you install**

- The `nanobook` crate with default features has one runtime dependency,
  `rustc-hash`. Every other dependency is behind an opt-in feature.
- The Python package declares no Python runtime dependencies. Its only build script
  sets linker flags for the extension module; nothing is downloaded at install time.
- The network-facing code (IBKR and Binance adapters) lives in `nanobook-broker`,
  behind the `ibkr` and `binance` features, and is not in the PyPI wheels.

**Dependencies**

- `cargo deny` (licences, bans, sources, advisories; `deny.toml`) and `cargo audit`
  run on every change to a manifest or lock file and every Monday
  (`.github/workflows/audit.yml`).
- Dependabot opens update pull requests for Cargo, pip and GitHub Actions.
- [Socket](https://socket.dev) scans every pull request for supply-chain risks.

**Build and release**

- Every GitHub Action is pinned to a full commit hash, and every workflow runs with
  read-only permissions unless a job needs more.
- PyPI releases use trusted publishing (no stored token) and carry PEP 740
  attestations, so you can check that a wheel was built by this repository's
  workflow.
- GitHub Releases carry a `SHA256SUMS` file and a signed build-provenance attestation
  for every `lob` binary. Check a download with `sha256sum -c SHA256SUMS` and its origin
  with `gh attestation verify <file> --repo BoringQuantSystems/nanobook`.
- The [OpenSSF Scorecard](https://scorecard.dev/viewer/?uri=github.com/BoringQuantSystems/nanobook)
  rates the repository's security practices weekly.
