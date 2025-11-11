# syntax=docker/dockerfile:1.4

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# ============================================
# BUILD ARGUMENTS
# ============================================
ARG KOHYA_VERSION
ARG VENV_PATH
ARG APP_VERSION

# ============================================
# ENVIRONMENT VARIABLES
# ============================================
ENV APP_TYPE=kohya \
    VENV_PATH=${VENV_PATH} \
    TEMPLATE_VERSION=${APP_VERSION} \
    AGENT_BIN=/app/podpilot-agent

# ============================================
# INSTALL KOHYA_SS
# ============================================

WORKDIR /workspace

# Clone Kohya_ss repository
RUN git clone --depth=1 https://github.com/bmaltais/kohya_ss.git && \
    cd kohya_ss && \
    git checkout ${KOHYA_VERSION} && \
    rm -rf .git

WORKDIR /workspace/kohya_ss

# Create and activate venv with --system-site-packages to inherit base PyTorch/xformers
RUN python3 -m venv --system-site-packages ${VENV_PATH}
ENV PATH="${VENV_PATH}/bin:$PATH"

# Install Kohya_ss requirements with BuildKit cache mount
# Kohya has specific installation steps
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir -r requirements.txt && \
    pip3 install --no-cache-dir -r requirements_linux.txt

# Install additional training dependencies
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir lion-pytorch lycoris_lora dadaptation prodigyopt

# Create model and output directories
RUN mkdir -p /workspace/kohya_ss/models \
             /workspace/kohya_ss/outputs \
             /workspace/kohya_ss/datasets \
             /workspace/kohya_ss/logs

# Clean up to reduce image size
RUN apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* /root/.cache

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
