# Build image for the Anthropic OSS Scanner (https://github.com/anthropics/oss-scanner).
# The scanner builds this file with the repository as the build context, then runs
# the image without network access. Everything the tests and fuzz targets need is
# fetched here.
FROM rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates git python3 clang pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=ghcr.io/astral-sh/uv:0.12.23 /uv /uvx /usr/local/bin/

# Number of parallel compile jobs. The scanner's build machine has 16 CPUs.
ARG JOBS=4
ENV CARGO_BUILD_JOBS=${JOBS} \
    RUSTFLAGS="-C force-frame-pointers=yes" \
    UV_PYTHON_INSTALL_DIR=/opt/uv-python \
    UV_CACHE_DIR=/opt/uv-cache \
    FEATURES="nanobook/serde,nanobook/persistence,nanobook/portfolio,nanobook/parallel,nanobook/itch,nanobook/scenarios,nanobook-broker/binance,nanobook-broker/ibkr,nanobook-broker/strict-market-reject"

COPY . /src
WORKDIR /src

# Rust workspace: engine, broker adapters, risk checks, rebalancer, Python binding crate.
RUN cargo fetch --locked \
    && cargo build --locked --workspace --all-targets --features "$FEATURES" \
    && cargo build --locked --features serde,itch --bin corpus-replay --bin itch-slice --bin lob \
    && cargo test --locked --workspace --features "$FEATURES" --no-run

# Python package (maturin build of python/) with the test dependencies.
RUN cd python && uv sync --python 3.13 --group dev

# Fuzz targets (fuzz/) need a nightly toolchain and cargo-fuzz. The nightly is pinned
# so that a broken nightly cannot break this build; move it forward on purpose.
ENV FUZZ_TOOLCHAIN=nightly-2026-10-08
RUN rustup toolchain install "$FUZZ_TOOLCHAIN" --profile minimal \
    && cargo +"$FUZZ_TOOLCHAIN" install cargo-fuzz --locked \
    && cd fuzz && cargo +"$FUZZ_TOOLCHAIN" fuzz build

# Run the tests once so a broken image is visible in the build log. A failure does not stop the build.
RUN cargo test --locked --workspace --features "$FEATURES" || echo "WARNING: cargo test reported failures"
RUN cd python && (uv run --offline python -m pytest tests/ -q --ignore=tests/reference || echo "WARNING: pytest reported failures")

# From here on there is no network.
ENV CARGO_NET_OFFLINE=true \
    UV_OFFLINE=1
