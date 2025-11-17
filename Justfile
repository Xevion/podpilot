set dotenv-load := true
set shell := ["fish", "-c"]

default:
    just --list

# Auto-reloading frontend server
frontend:
    pnpm run -C crates/podpilot-hub/web dev

# Production build of frontend
build-frontend:
    pnpm run -C crates/podpilot-hub/web build

check-scripts:
    bun --cwd scripts/ tsc --noEmit

# Auto-reloading backend server
backend *ARGS:
    bacon --headless run -- --bin podpilot-hub -- {{ARGS}}

# Production build
build-hub:
    pnpm run -C crates/podpilot-hub/web build
    cargo build --release --package podpilot-hub

# ==============================================
# DOCKER RUN COMMANDS
# ==============================================

# Run A1111
run-a1111 tailscale_authkey=env_var_or_default('AGENT_AUTHKEY', ''):
    docker run --rm --gpus all -p 7860:7860 -p 8081:8081 \
    {{ if tailscale_authkey != '' { '-e AGENT_AUTHKEY=' + tailscale_authkey } else { '' } }} \
    -v podpilot-models:/app/stable-diffusion-webui/models \
    -v podpilot-hf-cache:/workspace/huggingface \
    "ghcr.io/xevion/podpilot/a1111:cu12.1"

# Run ComfyUI
run-comfyui tailscale_authkey=env_var_or_default('AGENT_AUTHKEY', ''):
    docker run --rm --gpus all -p 8188:8188 -p 8189:8189 \
    {{ if tailscale_authkey != '' { '-e AGENT_AUTHKEY=' + tailscale_authkey } else { '' } }} \
    -v podpilot-models:/app/models \
    -v podpilot-comfyui-cache:/workspace/comfyui \
    "ghcr.io/xevion/podpilot/comfyui:cu12.1"

# Run Fooocus
run-fooocus tailscale_authkey=env_var_or_default('AGENT_AUTHKEY', ''):
    docker run --rm --gpus all -p 7860:7860 -p 8081:8081 \
    {{ if tailscale_authkey != '' { '-e AGENT_AUTHKEY=' + tailscale_authkey } else { '' } }} \
    -v podpilot-models:/app/models \
    -v podpilot-hf-cache:/workspace/huggingface \
    "ghcr.io/xevion/podpilot/fooocus:cu12.1"

# ==============================================
# AGENT DEVELOPMENT
# ==============================================

# Build agent binary (debug mode)
build-agent:
    cargo build --bin podpilot-agent

# Build live a1111 image (CUDA 12.1)
build-a1111-live:
    docker buildx bake a1111-cu121-live

# Run live a1111 with mounted local agent
run-a1111-dev hub_ws_url=env_var_or_default('HUB_WEBSOCKET_URL', 'ws://localhost:8080/ws/agent') log_level=env_var_or_default('LOG_LEVEL', 'info') tailscale_authkey=env_var_or_default('AGENT_AUTHKEY', '') ssh_authorized_keys=env_var_or_default('SSH_AUTHORIZED_KEYS', ''):
    docker run --rm \
        --gpus all \
        -p 7860:7860 \
        -p 8081:8081 \
        -e APP_TYPE=a1111 \
        -e AGENT_SOURCE=local \
        -e AGENT_BIN=/app/podpilot-agent \
        -e HUB_WEBSOCKET_URL="{{hub_ws_url}}" \
        {{ if tailscale_authkey != '' { '-e AGENT_AUTHKEY=' + tailscale_authkey } else { '' } }} \
        -e LOG_LEVEL="{{log_level}}" \
        -e SSH_AUTHORIZED_KEYS="{{ssh_authorized_keys}}" \
        -v "$(pwd)/target/debug/podpilot-agent:/app/podpilot-agent:ro" \
        -v podpilot-models:/app/stable-diffusion-webui/models \
        ghcr.io/xevion/podpilot/a1111:cu12.1-live

# Build agent + run a1111 (full dev workflow)
dev-a1111: build-agent run-a1111-dev

# ==============================================
# DOCKER BAKE COMMANDS (Multi-Image Builds)
# ==============================================

# Build image(s) using docker bake (default: latest base + a1111)
# Append `--push` to push to registry, or `--load` to load to local docker daemon, or `--pull` to pull from registry
# Specify targets like 'bases', 'apps', 'a1111', 'comfyui', 'fooocus', 'kohya', or 'all'
# App-specific targets can be specified like 'a1111-cu121', 'comfyui-cu128', etc.
bake TARGET="default" *ARGS:
    docker buildx bake {{TARGET}} {{ARGS}}

# Build image(s) using locally built base images instead of registry
# Useful for local development when base image changes haven't been pushed to registry
bake-dev TARGET="default" *ARGS:
    USE_LOCAL_BASE=1 docker buildx bake {{TARGET}} {{ARGS}}

# ==============================================
# HUB DEVELOPMENT COMMANDS
# ==============================================

# Run auto-reloading development build with release characteristics (frontend is embedded, non-auto-reloading)
# This is useful for testing backend release-mode details.
dev-build-hub *ARGS='--tracing pretty': build-frontend
    bacon --headless run -- --bin podpilot-hub --profile dev-release -- {{ARGS}}

# Auto-reloading development build for both frontend and backend
# Will not notice if either the frontend/backend crashes, but will generally be resistant to stopping on their own.
[parallel]
dev *ARGS='': frontend (backend ARGS)