# syntax=docker/dockerfile:1.4

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# ============================================
# BUILD ARGUMENTS
# ============================================
ARG WEBUI_VERSION
ARG VENV_PATH
ARG APP_VERSION

# ============================================
# ENVIRONMENT VARIABLES
# ============================================
ENV APP_TYPE=a1111 \
    VENV_PATH=${VENV_PATH} \
    TEMPLATE_VERSION=${APP_VERSION} \
    AGENT_BIN=/app/podpilot-agent

# ============================================
# INSTALL A1111
# ============================================

# Clone A1111 repository with version pinning
RUN git clone --branch ${WEBUI_VERSION} --depth=1 https://github.com/AUTOMATIC1111/stable-diffusion-webui.git || \
    (git clone --depth=1 https://github.com/AUTOMATIC1111/stable-diffusion-webui.git && \
     cd stable-diffusion-webui && \
     git checkout ${WEBUI_VERSION}) && \
    rm -rf stable-diffusion-webui/.git

WORKDIR /app/stable-diffusion-webui

# Install A1111 requirements with BuildKit cache mount
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir -r requirements_versions.txt

# Run A1111's prepare_environment to clone all dependencies at build time
# This automatically clones: stable-diffusion-webui-assets, stablediffusion,
# generative-models (SDXL), k-diffusion, BLIP, and installs CLIP/open_clip
RUN python3 -c "from modules.launch_utils import prepare_environment; prepare_environment()" --skip-torch-cuda-test && \
    find /app/stable-diffusion-webui -name ".git" -type d -exec rm -rf {} + 2>/dev/null || true

# Create model directories for volume mounts
RUN mkdir -p models/Stable-diffusion \
             models/VAE \
             models/LoRA \
             models/ESRGAN \
             models/Codeformer

# Clean up to reduce image size
RUN apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* /root/.cache

# ============================================
# INSTALL PODPILOT AGENT & TAILSCALE
# ============================================

# Copy agent binary from registry
COPY --from=ghcr.io/xevion/podpilot/agent:latest /app/target/release/podpilot-agent /app/podpilot-agent

# Copy Tailscale binaries
COPY --from=ghcr.io/tailscale/tailscale:stable /usr/local/bin/tailscaled /usr/local/bin/tailscaled
COPY --from=ghcr.io/tailscale/tailscale:stable /usr/local/bin/tailscale /usr/local/bin/tailscale

# Copy shared startup script
COPY apps/common/boot.sh /app/boot.sh
RUN chmod +x /app/boot.sh

# Working directory already set to /app/stable-diffusion-webui above

# The entrypoint is the shared boot script
CMD ["/app/boot.sh"]
