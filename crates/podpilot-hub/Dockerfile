# This Dockerfile is designed for building and deploying the PodPilot Hub on Railway.

# Build arguments
ARG RUST_VERSION=1.89.0
ARG RAILWAY_GIT_COMMIT_SHA

# --- Frontend Build Stage ---
# This stage builds the React frontend assets.
FROM node:22-bookworm-slim AS frontend-builder

# Install pnpm for package management
RUN npm install -g pnpm

WORKDIR /app

# Copy workspace Cargo.toml for version info if needed
COPY ./Cargo.toml ./Cargo.toml

# Copy frontend package files from the hub's web directory
COPY ./crates/podpilot-hub/web/package.json ./crates/podpilot-hub/web/pnpm-lock.yaml ./web/

# Install frontend dependencies
RUN pnpm -C web install --frozen-lockfile

# Copy frontend source code
COPY ./crates/podpilot-hub/web ./web

# Build frontend assets
RUN pnpm -C web run build

# --- Chef Base Stage ---
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

# --- Planner Stage ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json --bin podpilot-hub

# --- Rust Build Stage ---
# This stage builds the Rust backend binary.
FROM chef AS builder

# Set build-time environment variable for Railway Git commit SHA
ENV RAILWAY_GIT_COMMIT_SHA=${RAILWAY_GIT_COMMIT_SHA}

# Copy recipe from planner and build dependencies
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin podpilot-hub

# Install build dependencies needed for final compilation
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source code and built frontend assets
COPY . .
COPY --from=frontend-builder /app/web/dist ./crates/podpilot-hub/web/dist

# Build hub binary with embedded assets
RUN cargo build --release --package podpilot-hub

# Strip the binary to reduce size
RUN strip target/release/podpilot-hub

# --- Runtime Stage ---
# This is the final, lightweight image that will run on Railway.
FROM debian:12-slim

ARG APP=/app
ARG APP_USER=appuser
ARG UID=1000
ARG GID=1000

# Install runtime dependencies (ca-certificates for SSL)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    wget \
    && rm -rf /var/lib/apt/lists/*

ARG TZ=Etc/UTC
ENV TZ=${TZ}

# Create a non-root user for security
RUN addgroup --gid $GID $APP_USER \
    && adduser --uid $UID --disabled-password --gecos "" --ingroup $APP_USER $APP_USER \
    && mkdir -p ${APP}

# Copy the compiled application binary
COPY --from=builder --chown=$APP_USER:$APP_USER /app/target/release/podpilot-hub ${APP}/podpilot-hub

# Set proper permissions
RUN chmod +x ${APP}/podpilot-hub

USER $APP_USER
WORKDIR ${APP}

# Configure port from Railway's environment variable, default to 8080
ARG PORT=8080
ENV PORT=${PORT}
EXPOSE ${PORT}

CMD ["./podpilot-hub"]
