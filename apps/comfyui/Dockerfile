# syntax=docker/dockerfile:1.4

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# ============================================
# BUILD ARGUMENTS
# ============================================
ARG COMFYUI_VERSION
ARG VENV_PATH
ARG APP_VERSION

# ============================================
# ENVIRONMENT VARIABLES
# ============================================
ENV APP_TYPE=comfyui \
    VENV_PATH=${VENV_PATH} \
    TEMPLATE_VERSION=${APP_VERSION} \
    AGENT_BIN=/app/podpilot-agent

# ============================================
# INSTALL COMFYUI
# ============================================

WORKDIR /workspace

# Clone ComfyUI repository with version pinning
RUN git clone --branch ${COMFYUI_VERSION} --depth=1 https://github.com/comfyanonymous/ComfyUI.git || \
    (git clone --depth=1 https://github.com/comfyanonymous/ComfyUI.git && \
     cd ComfyUI && \
     git checkout ${COMFYUI_VERSION}) && \
    rm -rf ComfyUI/.git

WORKDIR /workspace/ComfyUI

# Create and activate venv with --system-site-packages to inherit base PyTorch/xformers
RUN python3 -m venv --system-site-packages ${VENV_PATH}
ENV PATH="${VENV_PATH}/bin:$PATH"

# Install ComfyUI requirements with BuildKit cache mount
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir -r requirements.txt

# Create model directories
RUN mkdir -p /workspace/ComfyUI/models/checkpoints \
             /workspace/ComfyUI/models/vae \
             /workspace/ComfyUI/models/loras \
             /workspace/ComfyUI/models/controlnet \
             /workspace/ComfyUI/models/clip \
             /workspace/ComfyUI/models/clip_vision \
             /workspace/ComfyUI/models/unet \
             /workspace/ComfyUI/models/embeddings

# Clean up to reduce image size
RUN apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* /root/.cache

# ============================================
# COPY AGENT AND STARTUP SCRIPTS
# ============================================

# Copy agent binary from registry
COPY --from=ghcr.io/xevion/podpilot/agent:latest /app/target/release/podpilot-agent /app/podpilot-agent

# Copy Tailscale binaries
COPY --from=ghcr.io/tailscale/tailscale:stable /usr/local/bin/tailscaled /usr/local/bin/tailscaled
COPY --from=ghcr.io/tailscale/tailscale:stable /usr/local/bin/tailscale /usr/local/bin/tailscale

# Copy shared startup script
COPY apps/common/boot.sh /app/boot.sh
RUN chmod +x /app/boot.sh

# The entrypoint is the shared boot script
CMD ["/app/boot.sh"]
