# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — Build
#   Uses the official Rust image so Cargo and the toolchain are already present.
#   We install the protobuf compiler and build in release mode.
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.78-slim-bookworm AS builder

# Install protoc (required by tonic-build at compile time).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        protobuf-compiler \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# ── Cache layer: copy manifests and build an empty stub so dependencies are
#    compiled before the real source (speeds up iterative rebuilds). ──────────
COPY Cargo.toml                         ./
COPY lumen-core/Cargo.toml              lumen-core/
COPY lumen-server/Cargo.toml            lumen-server/
COPY lumen-server/build.rs              lumen-server/
COPY proto/                             proto/

# Create minimal stub sources so `cargo build` can resolve the workspace.
RUN mkdir -p lumen-core/src lumen-server/src \
    && echo 'pub mod engine; pub mod wal; pub use engine::{Engine,EngineError}; pub use wal::{WalRecord,WalError,WriteAheadLog};' \
       > lumen-core/src/lib.rs \
    && touch lumen-core/src/engine.rs \
    && touch lumen-core/src/wal.rs \
    && echo 'fn main() {}' > lumen-server/src/main.rs \
    && touch lumen-server/src/service.rs

RUN cargo build --release --package lumen-server 2>/dev/null || true

# ── Real source ───────────────────────────────────────────────────────────────
COPY lumen-core/src/   lumen-core/src/
COPY lumen-server/src/ lumen-server/src/

# Touch sources to force incremental recompile of changed crates only.
RUN touch lumen-core/src/lib.rs \
          lumen-core/src/engine.rs \
          lumen-core/src/wal.rs \
          lumen-server/src/main.rs \
          lumen-server/src/service.rs

RUN cargo build --release --package lumen-server

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — Runtime
#   Distroless-like minimal Debian image; only the binary and its C runtime.
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system lumen \
    && useradd  --system --gid lumen lumen

WORKDIR /app

COPY --from=builder /build/target/release/lumen-server /app/lumen-server

RUN mkdir -p /data && chown -R lumen:lumen /app /data

USER lumen

# ── Environment defaults (override at runtime) ────────────────────────────────
ENV DATA_DIR=/data
ENV BIND_ADDR=0.0.0.0:50051
ENV RUST_LOG=lumen_server=info,lumen_core=info

EXPOSE 50051

HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 \
    CMD ["/bin/sh", "-c", "ss -tnlp | grep -q ':50051'"]

ENTRYPOINT ["/app/lumen-server"]
