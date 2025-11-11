# syntax=docker/dockerfile:1.4

# ============================================
# VARIABLES
# ============================================
variable "REGISTRY" {
  default = "ghcr.io"
}

variable "REGISTRY_USER" {
  default = "xevion"
}

variable "BASE_VERSION" {
  default = "1.0.0"
}

variable "APP_VERSION" {
  default = "1.0.0"
}

variable "RUST_VERSION" {
  default = "1.89.0"
}

variable "GIT_SHA" {
  default = "dev"
}

# ============================================
# AGENT BUILDER (shared across all apps)
# ============================================
target "agent" {
  dockerfile = "docker/Dockerfile.agent"
  target = "agent"
  platforms = ["linux/amd64"]
  tags = ["${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/agent:buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/agent:buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# BASE IMAGE TARGETS
# ============================================

# Base: Python 3.10 + CUDA 12.1 + PyTorch 2.1.2 (Legacy)
target "base-cu121" {
  dockerfile = "docker/Dockerfile.base"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1-${BASE_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1",
  ]
  args = {
    BASE_IMAGE = "nvidia/cuda:12.1.1-cudnn8-devel-ubuntu22.04"
    PYTHON_VERSION = "3.10"
    TORCH_VERSION = "2.1.2+cu121"
    XFORMERS_VERSION = "0.0.23.post1"
    INDEX_URL = "https://download.pytorch.org/whl/cu121"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1-buildcache,mode=max",
    "type=inline"
  ]
}

# Base: Python 3.11 + CUDA 12.4 + PyTorch 2.6.0
target "base-cu124" {
  dockerfile = "docker/Dockerfile.base"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.4-${BASE_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.4",
  ]
  args = {
    BASE_IMAGE = "nvidia/cuda:12.4.1-cudnn-devel-ubuntu22.04"
    PYTHON_VERSION = "3.11"
    TORCH_VERSION = "2.6.0+cu124"
    XFORMERS_VERSION = "0.0.29.post3"
    INDEX_URL = "https://download.pytorch.org/whl/cu124"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.4-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.4"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.4-buildcache,mode=max",
    "type=inline"
  ]
}

# Base: Python 3.12 + CUDA 12.8 + PyTorch 2.8.0 (Latest)
target "base-cu128" {
  dockerfile = "docker/Dockerfile.base"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8-${BASE_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/base:latest",
  ]
  args = {
    BASE_IMAGE = "nvidia/cuda:12.8.1-cudnn-devel-ubuntu22.04"
    PYTHON_VERSION = "3.12"
    TORCH_VERSION = "2.8.0+cu128"
    XFORMERS_VERSION = "0.0.32.post2"
    INDEX_URL = "https://download.pytorch.org/whl/cu128"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8-buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# APPLICATION TARGETS (Matrix-generated)
# ============================================
# This target generates all 12 app variants (4 apps × 3 CUDA versions)
# using matrix expansion. To add a new app, add an entry to the app array below.
# To add a CUDA version, add it to the cuda array below.

target "app_matrix" {
  name = "${app.name}-cu${cuda}"
  matrix = {
    app = [
      {
        name = "a1111"
        dockerfile = "apps/a1111/Dockerfile"
        version_var = "WEBUI_VERSION"
        version = "v1.10.1"
      },
      {
        name = "comfyui"
        dockerfile = "apps/comfyui/Dockerfile"
        version_var = "COMFYUI_VERSION"
        version = "v0.3.68"
      },
      {
        name = "fooocus"
        dockerfile = "apps/fooocus/Dockerfile"
        version_var = "FOOOCUS_COMMIT"
        version = "main"
      },
      {
        name = "kohya"
        dockerfile = "apps/kohya/Dockerfile"
        version_var = "KOHYA_VERSION"
        version = "v24.1.7"
      },
    ]
    cuda = ["121", "124", "128"]
  }

  dockerfile = app.dockerfile

  # Format CUDA version: 121 -> 12.1, 124 -> 12.4, 128 -> 12.8
  tags = concat(
    [
      # Versioned with APP_VERSION
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}-${APP_VERSION}",
      # Floating CUDA-specific tag
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}",
      # Explicit floating tag with CUDA version
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:latest-cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}",
      # Upstream app version with CUDA
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:${app.version}-cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}",
      # Git SHA with CUDA
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:${GIT_SHA}-cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}",
    ],
    # Add :latest tag only for cu128
    cuda == "128" ? ["${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:latest"] : []
  )

  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}"
    "${app.version_var}" = app.version
    VENV_PATH = "/workspace/venvs/${app.name}"
    APP_VERSION = "${APP_VERSION}"
  }

  platforms = ["linux/amd64"]

  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}"
  ]

  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${app.name}:cu${substr(cuda, 0, 2)}.${substr(cuda, 2, 1)}-buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# GROUPS
# ============================================

# Default: Quick dev build (latest versions only)
group "default" {
  targets = [
    "base-cu128",
    "a1111-cu128",
  ]
}

# All base images
group "bases" {
  targets = [
    "base-cu121",
    "base-cu124",
    "base-cu128",
  ]
}

# All A1111 variants
group "a1111" {
  targets = [
    "a1111-cu121",
    "a1111-cu124",
    "a1111-cu128",
  ]
}

# All ComfyUI variants
group "comfyui" {
  targets = [
    "comfyui-cu121",
    "comfyui-cu124",
    "comfyui-cu128",
  ]
}

# All Fooocus variants
group "fooocus" {
  targets = [
    "fooocus-cu121",
    "fooocus-cu124",
    "fooocus-cu128",
  ]
}

# All Kohya variants
group "kohya" {
  targets = [
    "kohya-cu121",
    "kohya-cu124",
    "kohya-cu128",
  ]
}

# All application images (assumes bases exist)
group "apps" {
  targets = [
    "a1111-cu121",
    "a1111-cu124",
    "a1111-cu128",
    "comfyui-cu121",
    "comfyui-cu124",
    "comfyui-cu128",
    "fooocus-cu121",
    "fooocus-cu124",
    "fooocus-cu128",
    "kohya-cu121",
    "kohya-cu124",
    "kohya-cu128",
  ]
}

# Everything
group "all" {
  targets = [
    "base-cu121",
    "base-cu124",
    "base-cu128",
    "a1111-cu121",
    "a1111-cu124",
    "a1111-cu128",
    "comfyui-cu121",
    "comfyui-cu124",
    "comfyui-cu128",
    "fooocus-cu121",
    "fooocus-cu124",
    "fooocus-cu128",
    "kohya-cu121",
    "kohya-cu124",
    "kohya-cu128",
  ]
}
