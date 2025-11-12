# syntax=docker/dockerfile:1.4
ARG RUST_VERSION=1.89.0

# --- Chef Base Stage ---
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

# --- Planner Stage ---
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json --bin podpilot-agent

# --- Rust Agent Stage ---
FROM chef AS agent
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this layer is cached as long as dependencies don't change
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json --bin podpilot-agent

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the agent binary
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --bin podpilot-agent

# Strip the binary to reduce size
RUN strip ./target/release/podpilot-agent

# --- Runtime Stage ---
FROM debian:12-slim

ARG APP=/app
ARG APP_USER=appuser
ARG UID=1000
ARG GID=1000

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

ARG TZ=Etc/UTC
ENV TZ=${TZ}

# Create a non-root user for security
RUN addgroup --gid $GID $APP_USER \
    && adduser --uid $UID --disabled-password --gecos "" --ingroup $APP_USER $APP_USER \
    && mkdir -p ${APP}

# Copy the compiled application binary
COPY --from=agent --chown=$APP_USER:$APP_USER --chmod=755 /app/target/release/podpilot-agent ${APP}/podpilot-agent

USER $APP_USER
WORKDIR ${APP}

CMD ["./podpilot-agent"]
