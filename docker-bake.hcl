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
# FUNCTIONS
# ============================================

# Format CUDA version: 121 -> "12.1"
function "format_cuda" {
  params = [version]
  result = "${substr(version, 0, 2)}.${substr(version, 2, 1)}"
}

# Generate base image tags
function "base_tags" {
  params = [cuda_version, is_latest]
  result = concat(
    [
      "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu${format_cuda(cuda_version)}-${BASE_VERSION}",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu${format_cuda(cuda_version)}",
    ],
    is_latest ? ["${REGISTRY}/${REGISTRY_USER}/podpilot/base:latest"] : []
  )
}

# Generate cache-from configuration
function "cache_from" {
  params = [name, tag]
  result = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${name}:${tag}-buildcache",
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${name}:${tag}"
  ]
}

# Generate cache-to configuration
function "cache_to" {
  params = [name, tag]
  result = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/${name}:${tag}-buildcache,mode=max",
    "type=inline"
  ]
}

# Generate app image tags for composition targets
function "app_tags" {
  params = [app, cuda_formatted, is_latest]
  result = concat(
    [
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:cu${cuda_formatted}-${APP_VERSION}",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:cu${cuda_formatted}",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:latest-cu${cuda_formatted}",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:${GIT_SHA}-cu${cuda_formatted}",
    ],
    is_latest ? ["${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:latest"] : []
  )
}

# ============================================
# BASE IMAGE TARGETS
# ============================================

target "base_matrix" {
  name = "base-cu${cuda.version}"
  matrix = {
    cuda = [
      {
        version = "121"
        base_image = "nvidia/cuda:12.1.1-cudnn8-devel-ubuntu22.04"
        python_version = "3.10"
        torch_version = "2.1.2+cu121"
        xformers_version = "0.0.23.post1"
        index_url = "https://download.pytorch.org/whl/cu121"
        is_latest = false
      },
      {
        version = "124"
        base_image = "nvidia/cuda:12.4.1-cudnn-devel-ubuntu22.04"
        python_version = "3.11"
        torch_version = "2.6.0+cu124"
        xformers_version = "0.0.29.post3"
        index_url = "https://download.pytorch.org/whl/cu124"
        is_latest = false
      },
      {
        version = "128"
        base_image = "nvidia/cuda:12.8.1-cudnn-devel-ubuntu22.04"
        python_version = "3.12"
        torch_version = "2.8.0+cu128"
        xformers_version = "0.0.32.post2"
        index_url = "https://download.pytorch.org/whl/cu128"
        is_latest = true
      },
    ]
  }

  dockerfile = "apps/Dockerfile.base"
  tags = base_tags(cuda.version, cuda.is_latest)

  args = {
    BASE_IMAGE = cuda.base_image
    PYTHON_VERSION = cuda.python_version
    TORCH_VERSION = cuda.torch_version
    XFORMERS_VERSION = cuda.xformers_version
    INDEX_URL = cuda.index_url
  }

  platforms = ["linux/amd64"]
  cache-from = cache_from("base", "cu${format_cuda(cuda.version)}")
  cache-to = cache_to("base", "cu${format_cuda(cuda.version)}")
}

# ============================================
# AGENT BUILDER (local only, used as context)
# ============================================
target "agent-binary" {
  dockerfile = "crates/podpilot-agent/Dockerfile"
  context = "."
  platforms = ["linux/amd64"]
  tags = concat(
    [
      "${REGISTRY}/${REGISTRY_USER}/podpilot/agent:${APP_VERSION}",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest",
      "${REGISTRY}/${REGISTRY_USER}/podpilot/agent:dev-${GIT_SHA}",
    ]
  )
  cache-from = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/agent:buildcache",
  ]
  cache-to = [
    "type=registry,ref=${REGISTRY}/${REGISTRY_USER}/podpilot/agent:buildcache,mode=max",
  ]
}

# ============================================
# APP-ONLY TARGETS (local only, used as contexts)
# ============================================
# These build just the application without agent, scripts, or Tailscale.
# They serve as the base for composition targets.

target "app_only_matrix" {
  name = "${app.name}-app-cu${cuda}"
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
  contexts = {
    baseimage = "docker-image://${REGISTRY}/${REGISTRY_USER}/podpilot/base:cu${format_cuda(cuda)}"
  }

  tags = ["podpilot-app-${app.name}:cu${format_cuda(cuda)}"]

  args = {
    "${app.version_var}" = app.version
    VENV_PATH = "/workspace/venvs/${app.name}"
    APP_VERSION = "${APP_VERSION}"
  }

  platforms = ["linux/amd64"]
  output = ["type=docker"]
}

# ============================================
# COMPOSITION TARGETS (final images, pushed to registry)
# ============================================
# These combine app + agent + scripts + Tailscale into final images.

# Base configuration for all composition targets
target "compose_base" {
  dockerfile = "apps/Dockerfile.compose"
  platforms = ["linux/amd64"]
}

# Static composition (production) - 12 images total
target "compose_static_matrix" {
  inherits = ["compose_base"]
  name = "${app}-cu${cuda}"
  matrix = {
    app = ["a1111", "comfyui", "fooocus", "kohya"]
    cuda = ["121", "124", "128"]
  }

  contexts = {
    sourceapp = "target:${app}-app-cu${cuda}"
    agentbuild = "docker-image://${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest"
  }

  args = {
    AGENT_TYPE = "static"
  }

  tags = app_tags(app, format_cuda(cuda), cuda == "128")
  cache-from = cache_from(app, "cu${format_cuda(cuda)}")
  cache-to = cache_to(app, "cu${format_cuda(cuda)}")
}

# Live composition (development) - 12 images total (all CUDA versions supported)
target "compose_live_matrix" {
  inherits = ["compose_base"]
  name = "${app}-cu${cuda}-live"
  matrix = {
    app = ["a1111", "comfyui", "fooocus", "kohya"]
    cuda = ["121", "124", "128"]
  }

  contexts = {
    sourceapp = "target:${app}-app-cu${cuda}"
    agentbuild = "docker-image://${REGISTRY}/${REGISTRY_USER}/podpilot/agent:latest"
  }

  args = {
    AGENT_TYPE = "live"
  }

  tags = ["${REGISTRY}/${REGISTRY_USER}/podpilot/${app}:cu${format_cuda(cuda)}-live"]
}

# ============================================
# GROUPS
# ============================================

# All base images
group "bases" {
  targets = [
    "base-cu121",
    "base-cu124",
    "base-cu128",
  ]
}

# Agent binary builder
group "agent" {
  targets = ["agent-binary"]
}

# All static composition targets (production)
group "static" {
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

# Live composition targets (for development, cu121 only by default)
group "live" {
  targets = [
    "a1111-cu121-live",
    "comfyui-cu121-live",
    "fooocus-cu121-live",
    "kohya-cu121-live",
  ]
}