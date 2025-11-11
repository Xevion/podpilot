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
RUN git clone https://github.com/lllyasviel/Fooocus.git && \
    cd Fooocus && \
    git checkout ${FOOOCUS_COMMIT}

WORKDIR /workspace/Fooocus

# Create and activate venv
RUN python3 -m venv ${VENV_PATH}
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

# ============================================
# COPY AGENT AND STARTUP SCRIPTS
# ============================================

# Copy agent binary from agent-builder context
COPY --from=agent-builder /usr/src/podpilot-agent/target/release/podpilot-agent /app/podpilot-agent

# Copy Tailscale binaries
COPY --from=docker.io/tailscale/tailscale:stable /usr/local/bin/tailscaled /usr/local/bin/tailscaled
COPY --from=docker.io/tailscale/tailscale:stable /usr/local/bin/tailscale /usr/local/bin/tailscale

# Copy shared startup script
COPY apps/common/boot.sh /app/boot.sh
RUN chmod +x /app/boot.sh

# The entrypoint is the shared boot script
CMD ["/app/boot.sh"]
