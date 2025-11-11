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

# ============================================
# AGENT BUILDER (shared across all apps)
# ============================================
target "agent-builder" {
  dockerfile = "Dockerfile.agent"
  target = "agent-builder"
  platforms = ["linux/amd64"]
  tags = ["${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest"]
  cache-from = ["type=gha,scope=agent"]
  cache-to = ["type=gha,mode=max,scope=agent"]
}

# ============================================
# BASE IMAGE TARGETS
# ============================================

# Base: Python 3.10 + CUDA 12.1 + PyTorch 2.1.2 (Legacy)
target "base-cu121" {
  dockerfile = "Dockerfile.base"
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
  dockerfile = "Dockerfile.base"
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
  dockerfile = "Dockerfile.base"
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
# A1111 APPLICATION TARGETS
# ============================================

# A1111 - CUDA 12.1 (Legacy)
target "a1111-cu121" {
  dockerfile = "apps/a1111/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.1-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.1",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1"
    WEBUI_VERSION = "v1.10.1"
    VENV_PATH = "/workspace/venvs/a1111"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.1-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.1"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.1-buildcache,mode=max",
    "type=inline"
  ]
}

# A1111 - CUDA 12.8 (Latest)
target "a1111-cu128" {
  dockerfile = "apps/a1111/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.8-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.8",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:latest",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8"
    WEBUI_VERSION = "v1.10.1"
    VENV_PATH = "/workspace/venvs/a1111"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.8-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.8"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/a1111:cu12.8-buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# COMFYUI APPLICATION TARGETS
# ============================================

# ComfyUI - CUDA 12.1 (Legacy)
target "comfyui-cu121" {
  dockerfile = "apps/comfyui/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.1-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.1",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1"
    COMFYUI_VERSION = "v0.3.68"
    VENV_PATH = "/workspace/venvs/comfyui"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.1-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.1"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.1-buildcache,mode=max",
    "type=inline"
  ]
}

# ComfyUI - CUDA 12.8 (Latest)
target "comfyui-cu128" {
  dockerfile = "apps/comfyui/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.8-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.8",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:latest",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8"
    COMFYUI_VERSION = "v0.3.68"
    VENV_PATH = "/workspace/venvs/comfyui"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.8-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.8"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/comfyui:cu12.8-buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# FOOOCUS APPLICATION TARGETS
# ============================================

# Fooocus - CUDA 12.1 (Legacy)
target "fooocus-cu121" {
  dockerfile = "apps/fooocus/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.1-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.1",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1"
    FOOOCUS_COMMIT = "main"
    VENV_PATH = "/workspace/venvs/fooocus"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.1-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.1"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.1-buildcache,mode=max",
    "type=inline"
  ]
}

# Fooocus - CUDA 12.8 (Latest)
target "fooocus-cu128" {
  dockerfile = "apps/fooocus/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.8-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.8",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:latest",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8"
    FOOOCUS_COMMIT = "main"
    VENV_PATH = "/workspace/venvs/fooocus"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.8-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.8"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/fooocus:cu12.8-buildcache,mode=max",
    "type=inline"
  ]
}

# ============================================
# KOHYA_SS APPLICATION TARGETS
# ============================================

# Kohya_ss - CUDA 12.1 (Legacy)
target "kohya-cu121" {
  dockerfile = "apps/kohya/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.1-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.1",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.1"
    KOHYA_VERSION = "v24.1.7"
    VENV_PATH = "/workspace/venvs/kohya"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.1-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.1"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.1-buildcache,mode=max",
    "type=inline"
  ]
}

# Kohya_ss - CUDA 12.8 (Latest)
target "kohya-cu128" {
  dockerfile = "apps/kohya/Dockerfile"
  tags = [
    "${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.8-${APP_VERSION}",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.8",
    "${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:latest",
  ]
  args = {
    BASE_IMAGE = "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu12.8"
    KOHYA_VERSION = "v24.1.7"
    VENV_PATH = "/workspace/venvs/kohya"
    APP_VERSION = "${APP_VERSION}"
  }
  platforms = ["linux/amd64"]
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.8-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.8"
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/kohya:cu12.8-buildcache,mode=max",
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
    "a1111-cu128",
  ]
}

# All ComfyUI variants
group "comfyui" {
  targets = [
    "comfyui-cu121",
    "comfyui-cu128",
  ]
}

# All Fooocus variants
group "fooocus" {
  targets = [
    "fooocus-cu121",
    "fooocus-cu128",
  ]
}

# All Kohya variants
group "kohya" {
  targets = [
    "kohya-cu121",
    "kohya-cu128",
  ]
}

# All application images (assumes bases exist)
group "apps" {
  targets = [
    "a1111-cu121",
    "a1111-cu128",
    "comfyui-cu121",
    "comfyui-cu128",
    "fooocus-cu121",
    "fooocus-cu128",
    "kohya-cu121",
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
    "a1111-cu128",
    "comfyui-cu121",
    "comfyui-cu128",
    "fooocus-cu121",
    "fooocus-cu128",
    "kohya-cu121",
    "kohya-cu128",
  ]
}
