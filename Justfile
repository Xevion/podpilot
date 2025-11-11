default:
    just --list

# Auto-reloading frontend server
frontend:
    pnpm run -C web dev

# Production build of frontend
build-frontend:
    pnpm run -C web build

# Auto-reloading backend server
backend *ARGS:
    bacon --headless run -- -- {{ARGS}}

# Production build
build-hub:
    pnpm run -C web build
    cargo build --release --package podpilot-hub

# ==============================================
# DOCKER RUN COMMANDS
# ==============================================

# Run A1111 with Web UI enabled (CUDA 12.1 - for older GPUs)
run-a1111:
    docker run --rm -it \
    --gpus all \
    -p 7860:7860 \
    -p 8081:8081 \
    -e ENABLE_WEBUI=1 \
    -v podpilot-models:/app/stable-diffusion-webui/models \
    -v podpilot-hf-cache:/workspace/huggingface \
    ghcr.io/xevion/podpilot/a1111:cu12.1

# ==============================================
# DOCKER BAKE COMMANDS (Multi-Image Builds)
# ==============================================

# Build image(s) using docker bake (default: latest base + a1111)
# Append `--push` to push to registry, or `--load` to load to local docker daemon, or `--pull` to pull from registry
# Specify targets like 'bases', 'apps', 'a1111', 'comfyui', 'fooocus', 'kohya', or 'all'
# App-specific targets can be specified like 'a1111-cu121', 'comfyui-cu128', etc.
bake TARGET="default" *ARGS:
    docker buildx bake {{TARGET}} {{ARGS}}

# ==============================================
# HUB DEVELOPMENT COMMANDS
# ==============================================

# Run auto-reloading development build with release characteristics (frontend is embedded, non-auto-reloading)
# This is useful for testing backend release-mode details.
dev-build-hub *ARGS='--services web --tracing pretty': build-frontend
    bacon --headless run -- --profile dev-release -- {{ARGS}}

# Auto-reloading development build for both frontend and backend
# Will not notice if either the frontend/backend crashes, but will generally be resistant to stopping on their own.
[parallel]
dev *ARGS='--services web,bot': frontend (backend ARGS)