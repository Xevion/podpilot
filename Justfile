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

push-agent-image:
    docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.agent \
    --tag ghcr.io/xevion/podpilot-agent:latest \
    --push \
    .

# Build base image with Python + torch + A1111 (run weekly or when dependencies change)
build-base-image:
    DOCKER_BUILDKIT=1 docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.base \
    --tag ghcr.io/xevion/podpilot-base:latest \
    --tag ghcr.io/xevion/podpilot-base:$(date +%Y%m%d) \
    --load \
    .

# Push base image to GitHub Container Registry
push-base-image:
    DOCKER_BUILDKIT=1 docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.base \
    --tag ghcr.io/xevion/podpilot-base:latest \
    --tag ghcr.io/xevion/podpilot-base:$(date +%Y%m%d) \
    --push \
    .

build-a1111-image:
    DOCKER_BUILDKIT=1 docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.cuda \
    --build-arg BASE_IMAGE=nvidia/cuda:12.4.1-cudnn-runtime-ubuntu22.04 \
    --build-arg REQUIRED_CUDA_VERSION=12.4 \
    --build-arg PYTHON_VERSION=3.10 \
    --build-arg TORCH_VERSION=2.6.0 \
    --build-arg CUDA_VERSION=124 \
    --build-arg XFORMERS_VERSION=0.0.29.post3 \
    --build-arg WEBUI_VERSION=v1.10.1 \
    --build-arg CONTROLNET_COMMIT=56cec5b2958edf3b1807b7e7b2b1b5186dbd2f81 \
    --build-arg CIVITAI_BROWSER_PLUS_VERSION=v3.6.0 \
    --build-arg CIVITAI_DOWNLOADER_VERSION=3.0.0 \
    --build-arg VENV_PATH=/venv \
    --build-arg INDEX_URL=https://download.pytorch.org/whl/cu124 \
    --tag podpilot-a1111:test \
    --output type=docker \
    .

# Run A1111 image with default settings
test-a1111-image: build-a1111-image
    docker run --rm -it \
    --gpus all \
    -p 3001:3001 \
    -p 7860:7860 \
    -v /home/xevion/projects/podpilot/models:/workspace/models \
    -e PORT=7860 \
    podpilot-a1111:test

# Run A1111 image with custom port (example: just run-a1111-custom 8080)
run-a1111-custom PORT="7860":
    docker run --rm -it \
    --gpus all \
    -p {{PORT}}:{{PORT}} \
    -v /home/xevion/projects/podpilot/models:/workspace/models \
    -e PORT={{PORT}} \
    podpilot-a1111:test

# Build and test agent image locally (requires base image)
# Ports: 7861 (SD API), 8081 (Agent API)
# For web UI: use test-agent-image-webui or set ENABLE_WEBUI=1
test-agent-image:
    DOCKER_BUILDKIT=1 docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.agent \
    --build-arg BASE_IMAGE=ghcr.io/xevion/podpilot-base:latest \
    --tag podpilot-agent:test \
    --load \
    .

    docker run --rm -it \
    --gpus all \
    -p 7861:7861 \
    -p 8081:8081 \
    -v podpilot-models:/app/stable-diffusion-webui/models \
    -v podpilot-hf-cache:/workspace/huggingface \
    podpilot-agent:test

# Build and test agent image with Stable Diffusion web UI enabled
# Ports: 7860 (SD Web UI), 8081 (Agent API)
test-agent-image-webui:
    DOCKER_BUILDKIT=1 docker buildx build \
    --platform linux/amd64 \
    --file ./Dockerfile.agent \
    --build-arg BASE_IMAGE=ghcr.io/xevion/podpilot-base:latest \
    --tag podpilot-agent:test \
    --load \
    .

    docker run --rm -it \
    --gpus all \
    -p 7860:7860 \
    -p 8081:8081 \
    -e ENABLE_WEBUI=1 \
    -v podpilot-models:/app/stable-diffusion-webui/models \
    -v podpilot-hf-cache:/workspace/huggingface \
    podpilot-agent:test

# Run auto-reloading development build with release characteristics (frontend is embedded, non-auto-reloading)
# This is useful for testing backend release-mode details.
dev-build-hub *ARGS='--services web --tracing pretty': build-frontend
    bacon --headless run -- --profile dev-release -- {{ARGS}}

# Auto-reloading development build for both frontend and backend
# Will not notice if either the frontend/backend crashes, but will generally be resistant to stopping on their own.
[parallel]
dev *ARGS='--services web,bot': frontend (backend ARGS)