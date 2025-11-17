set dotenv-load := true
set shell := ["fish", "-c"]

default:
    just --list

check:
    bun --cwd scripts/ typecheck
    bun --cwd crates/podpilot-hub/web typecheck
    cargo clippy --all-targets --all-features --workspace -- --deny warnings

format:
    cargo fmt --all
    bun --cwd scripts/ format
    bun --cwd crates/podpilot-hub/web format

format-check:
    cargo fmt --all -- --check
    bun --cwd scripts/ format:check
    bun --cwd crates/podpilot-hub/web format:check

# Auto-reloading development for both frontend and backend (parallel)
[parallel]
hub-dev *ARGS='': (_hub-frontend "dev") (_hub-backend ARGS)

# Production build of Hub (frontend + backend binary)
hub-build: (_hub-frontend "build")
    cargo build --release --package podpilot-hub

# Build and run Hub Docker image
hub-docker: (bake "hub")
    docker run --rm -p 8080:80 \
        -e DATABASE_URL="$DATABASE_URL" \
        -e HUB_TAILSCALE_CLIENT_ID="$HUB_TAILSCALE_CLIENT_ID" \
        -e HUB_TAILSCALE_CLIENT_SECRET="$HUB_TAILSCALE_CLIENT_SECRET" \
        --name podpilot-hub-dev \
        podpilot-hub:dev

# Create or recreate local Postgres database with Docker (use 'reset' to wipe volume)
db MODE='':
    #!/usr/bin/env fish
    set container_name podpilot-postgres
    set volume_name podpilot-postgres-data

    # Stop and remove existing container if it exists
    if docker ps -a -q -f name=^$container_name\$ | grep -q .
        echo "Removing existing database container..."
        docker rm -f $container_name
    end

    # Remove volume if reset mode
    if test "{{MODE}}" = reset
        if docker volume ls -q -f name=^$volume_name\$ | grep -q .
            echo "Removing database volume..."
            docker volume rm $volume_name
        end
    end

    # Find available port in ephemeral range (49152-65535)
    set port (shuf -i 49152-65535 -n 1)
    while ss -tlnp 2>/dev/null | grep -q ":$port "
        set port (shuf -i 49152-65535 -n 1)
    end

    set db_url "postgresql://podpilot:podpilot@localhost:$port/podpilot"

    # Create new container with named volume and random port
    echo "Creating new database container on port $port..."
    docker run -d \
        --name $container_name \
        -p $port:5432 \
        -e POSTGRES_USER=podpilot \
        -e POSTGRES_PASSWORD=podpilot \
        -e POSTGRES_DB=podpilot \
        -v podpilot-postgres-data:/var/lib/postgresql/data \
        postgres:16-alpine

    # Update .env file (add or replace DATABASE_URL)
    if grep -q "^DATABASE_URL=" .env
        sed -i "s|^DATABASE_URL=.*|DATABASE_URL=$db_url|" .env
        echo "Updated DATABASE_URL in .env"
    else
        echo "DATABASE_URL=$db_url" >> .env
        echo "Added DATABASE_URL to .env"
    end

    echo "✓ Database ready at $db_url"

# Internal: Run frontend (MODE: dev|build)
_hub-frontend MODE:
    bun --cwd crates/podpilot-hub/web {{MODE}}

# Internal: Auto-reloading backend server
_hub-backend *ARGS:
    bacon --headless run -- --bin podpilot-hub -- {{ARGS}}


# Specify app and CUDA version to run (e.g., "a1111 121", "comfyui cu124", "kohya 128")
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

# Build agent binary (debug mode)
_build-agent:
    cargo build --bin podpilot-agent

# Agent development workflow: build + bake + run (e.g., just dev-agent a1111 121)
# CUDA: 121, 124, or 128 (optionally with 'cu' prefix)
dev-agent APP CUDA: _build-agent
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