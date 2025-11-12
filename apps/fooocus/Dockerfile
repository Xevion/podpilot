# syntax=docker/dockerfile:1.4

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# ============================================
# BUILD ARGUMENTS
# ============================================
ARG FOOOCUS_COMMIT
ARG VENV_PATH
ARG APP_VERSION

# ============================================
# ENVIRONMENT VARIABLES
# ============================================
ENV APP_TYPE=fooocus \
    VENV_PATH=${VENV_PATH} \
    TEMPLATE_VERSION=${APP_VERSION} \
    AGENT_BIN=/app/podpilot-agent

# ============================================
# INSTALL FOOOCUS
# ============================================

WORKDIR /workspace

# Clone Fooocus repository
RUN git clone --depth=1 https://github.com/lllyasviel/Fooocus.git && \
    cd Fooocus && \
    git checkout ${FOOOCUS_COMMIT} && \
    rm -rf .git

WORKDIR /workspace/Fooocus

# Create and activate venv with --system-site-packages to inherit base PyTorch/xformers
RUN python3 -m venv --system-site-packages ${VENV_PATH}
ENV PATH="${VENV_PATH}/bin:$PATH"

# Install Fooocus requirements with BuildKit cache mount
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir -r requirements_versions.txt

# Create model directories
RUN mkdir -p /workspace/Fooocus/models/checkpoints \
             /workspace/Fooocus/models/loras \
             /workspace/Fooocus/models/embeddings \
             /workspace/Fooocus/models/vae_approx \
             /workspace/Fooocus/models/upscale_models \
             /workspace/Fooocus/models/inpaint \
             /workspace/Fooocus/models/controlnet \
             /workspace/Fooocus/models/clip_vision

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

# Copy TypeScript boot scripts
COPY scripts/ /app/scripts/

# The entrypoint is the TypeScript boot script
CMD ["bun", "run", "/app/scripts/boot.ts"]
