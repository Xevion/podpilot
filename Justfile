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

# Run any app with any CUDA version (e.g., just run a1111 121)
# CUDA: 121, 124, or 128 (optionally with 'cu' prefix)
# Auto-detects local agent binary and mounts it for development
# Forwards all environment variables from .env automatically
run APP CUDA *DOCKER_ARGS:
    #!/usr/bin/env fish
    set agent_bin (pwd)/target/debug/podpilot-agent

    # Strip 'cu' prefix if present, then format CUDA version: 121 -> 12.1
    set cuda_clean (string replace -r '^cu' '' {{CUDA}})
    set cuda_major (string sub -l 2 $cuda_clean)
    set cuda_minor (string sub -s 3 $cuda_clean)
    set cuda_formatted "$cuda_major.$cuda_minor"

    if test -f $agent_bin
        set image_suffix "-live"
        set dev_args -e APP_TYPE={{APP}} -e AGENT_SOURCE=local -e AGENT_BIN=/app/podpilot-agent -v $agent_bin:/app/podpilot-agent:ro
    else
        set image_suffix ""
        set dev_args
    end

    set image ghcr.io/xevion/podpilot/{{APP}}:cu$cuda_formatted$image_suffix

    switch {{APP}}
        case a1111 fooocus
            set ports -p 7860:7860 -p 8081:8081
            set volumes -v podpilot-models:/app/stable-diffusion-webui/models -v podpilot-hf-cache:/workspace/huggingface
        case comfyui
            set ports -p 8188:8188 -p 8189:8189
            set volumes -v podpilot-models:/app/models -v podpilot-comfyui-cache:/workspace/comfyui
        case kohya
            set ports -p 7860:7860
            set volumes -v podpilot-models:/app/models -v podpilot-hf-cache:/workspace/huggingface
        case '*'
            echo "Unknown app: {{APP}}"
            exit 1
    end

    set env_args
    for var in HUB_WEBSOCKET_URL AGENT_AUTHKEY LOG_LEVEL SSH_AUTHORIZED_KEYS
        if set -q $var
            set env_args $env_args -e $var=$$var
        end
    end

    docker run --rm --gpus all $ports $volumes $dev_args $env_args {{DOCKER_ARGS}} $image

# ==============================================
# AGENT DEVELOPMENT
# ==============================================

# Build agent binary (debug mode)
build-agent:
    cargo build --bin podpilot-agent

# Agent development workflow: build + bake + run (e.g., just dev-agent a1111 121)
# CUDA: 121, 124, or 128 (optionally with 'cu' prefix)
dev-agent APP CUDA: build-agent
    #!/usr/bin/env fish
    # Strip 'cu' prefix if present for bake target
    set cuda_clean (string replace -r '^cu' '' {{CUDA}})
    docker buildx bake {{APP}}-cu$cuda_clean-live
    just run {{APP}} {{CUDA}}

# Build image(s) using registry base images (default: latest base + a1111)
# Append `--push` to push to registry, or `--load` to load to local docker daemon
# Target examples: 'bases', 'static', 'live', 'a1111-cu121', 'a1111-cu121-live', etc.
bake TARGET="default" *ARGS:
    docker buildx bake {{TARGET}} {{ARGS}}

# Build image(s) using locally-built base images instead of registry
# Useful when base image changes haven't been pushed to registry yet
bake-local TARGET="default" *ARGS:
    USE_LOCAL_BASE=1 docker buildx bake {{TARGET}} {{ARGS}}

# Auto-reloading development for both frontend and backend (parallel)
[parallel]
dev *ARGS='': frontend (backend ARGS)

# Build Hub Docker image
build-hub-image:
    docker buildx build -t podpilot-hub:dev -f crates/podpilot-hub/Dockerfile .

# Run Hub Docker image
run-hub:
    docker run --rm -p 8080:8080 \
    -e DATABASE_URL="$DATABASE_URL" \
    -e HUB_TAILSCALE_CLIENT_ID="$HUB_TAILSCALE_CLIENT_ID" \
    -e HUB_TAILSCALE_CLIENT_SECRET="$HUB_TAILSCALE_CLIENT_SECRET" \
    --name podpilot-hub-dev \
    podpilot-hub:dev