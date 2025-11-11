# PodPilot

A personal management system for remote, ephemeral GPU instances used for image diffusion tasks on platforms like Vast.ai and Runpod.

## Core Problem

Managing remote GPU instances for creative work involves several repetitive and error-prone tasks:

- **Slow Setup:** Manually setting up a new instance with the correct models, extensions, and configurations is time-consuming.
- **Large File Management:** Diffusion models (checkpoints, LoRAs, etc.) are multiple gigabytes and must be re-downloaded for each new instance.
- **Asset Synchronization:** Manually backing up generated images, videos, and other assets from the remote instance is tedious.
- **Cost Control:** Forgetting to shut down an idle instance leads to unnecessary costs.
- **Environment Replication:** Recreating a specific working environment with the exact combination of extensions and settings is difficult.

## The PodPilot Solution

PodPilot solves these problems by providing a centralized **Hub** and a remote **Agent** that work together to automate the entire lifecycle of a GPU instance. It treats instances as cattle, not pets, allowing for rapid, reproducible, and cost-effective deployment.

## Architecture Overview

PodPilot uses a client-server architecture built on a Rust workspace. Secure networking is handled by Tailscale, ensuring services are not exposed to the public internet or the underlying machine owner.

- **Central Hub (`podpilot-hub`):** A web server that acts as the central control plane. It hosts the API, the frontend UI, and connects to a Postgres database to store all state.
- **Remote Agent (`podpilot-agent`):** A lightweight client that runs inside the Docker container on the rented GPU. It registers with the Hub and carries out tasks like downloading models, synchronizing files, and monitoring activity.
- **Shared Library (`podpilot-common`):** A shared Rust crate containing common data structures, error types, and utilities used by both the Hub and the Agent.
- **Data Storage (Cloudflare R2):** All large binary files (models, generated images, assets) are stored in an S3-compatible R2 bucket to leverage its zero egress fees.
- **Secure Networking (Tailscale):** All communication between the Agent and the Hub, and between the user and the services, occurs over a private Tailscale network, locked down with strict ACLs.

## Repository Structure

This is a Rust workspace (monorepo).

- **`/podpilot-hub`**: The main web server and API.
  - **`/podpilot-hub/src`**: Rust backend code (Axum/SQLx).
  - **`/podpilot-hub/web`**: React frontend code (Vite/TanStack Router).
- **`/podpilot-agent`**: The client application that runs on the remote GPU.
- **`/podpilot-common`**: The shared library crate.
- **`/migrations`**: SQLx database migration files.
- **`Dockerfile`**: A universal Docker image definition used to run the agent on any provider.

## Key Concepts

- **Profiles:** A "Profile" is a declarative snapshot of a complete working environment. It defines:

  - The base Web UI (e.g., A1111, ComfyUI).
  - A list of required git-based extensions.
  - A list of required models (Checkpoints, LoRAs, VAEs).
  - The exact configuration files needed. The agent uses a Profile to provision an instance from a blank state into a ready-to-use environment.

- **Tailscale Security Model:** The agent does not expose any ports on the host. It joins a private tailnet using a one-time, ephemeral auth key and is registered with the tag `tag:podpilot-agent`. Strict ACLs in the tailnet policy ensure the agent can _only_ communicate with the Hub on its designated port and nothing else, mitigating risks from a compromised container.

## Building the Project

This is a standard Rust workspace. To check for compile errors, use `cargo check`.

### Docker Builds & Caching Strategy

PodPilot uses Docker Buildx Bake for multi-image builds across different CUDA versions. The build system is optimized for fast rebuilds using a two-tier caching strategy:

#### Cache Types

1. **Local Cache Mounts** (Fast, machine-local)
   - BuildKit cache mounts persist pip downloads (~2GB for PyTorch) across builds
   - Stored in `/root/.cache/pip` inside build containers
   - Only benefits builds on the same machine
   - Automatically enabled in all Dockerfiles via `--mount=type=cache`

2. **Registry Cache** (Portable, works in CI/CD)
   - Layer cache and inline cache stored in container registry
   - Enables fast rebuilds across different machines and CI/CD
   - Requires initial push to populate registry cache
   - Configured in `docker-bake.hcl` with `cache-from` and `cache-to`

#### Build Commands

```bash
# Local development (uses cache mounts + registry cache)
just bake                    # Build default target (base-cu128 + a1111-cu128)
just bake bases              # Build all 3 base images (cu121, cu124, cu128)
just bake apps               # Build all application images

# First-time setup (populate registry cache for CI/CD)
just bake-with-cache bases   # Build and push buildcache to registry

# CI/CD builds (pull registry cache before building)
just bake-from-cache         # Pull cache, then build

# Cache management
just cache-info              # Show build cache disk usage
just prune-cache             # Delete all build cache (frees disk space)
```

#### How Caching Works

**Base Images (`Dockerfile.base`):**
- Separate RUN layers for torch, xformers, and app requirements
- Torch layer (~2GB) rarely changes = fast rebuilds
- Git repos pinned to specific versions via `WEBUI_VERSION` ARG
- ARGs declared close to usage to minimize layer invalidation

**Application Images (`apps/*/Dockerfile`):**
- Build on pre-built base images
- Only install app-specific dependencies
- Inherit torch/xformers from base (no re-download)

#### Performance Expectations

- **First build**: ~15-30 min (downloads PyTorch 2GB+)
- **Local rebuild** (cached): ~2-5 min (cache mounts + layer cache)
- **CI/CD rebuild** (registry cache): ~5-10 min (pulls layers from registry)
- **No-op rebuild**: ~30 sec (all layers cached)

#### Troubleshooting

If cache isn't working:
1. Ensure `DOCKER_BUILDKIT=1` is set (required for cache mounts)
2. Check if registry buildcache exists: `docker manifest inspect ghcr.io/xevion/podpilot/base:cu12.8-buildcache`
3. Populate registry cache: `just bake-with-cache bases`
4. Verify cache mount is working: look for `---> Using cache` in build output
