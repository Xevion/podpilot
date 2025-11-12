# syntax=docker/dockerfile:1.4
ARG RUST_VERSION=1.89.0

# --- Chef Base Stage ---
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

# --- Planner Stage ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json --bin podpilot-agent

# --- Rust Agent Stage ---
FROM chef AS agent
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this layer is cached as long as dependencies don't change
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json --bin podpilot-agent

# Copy source code
COPY . .
# Build the agent binary
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --bin podpilot-agent

# Strip the binary to reduce size
RUN strip ./target/release/podpilot-agent
