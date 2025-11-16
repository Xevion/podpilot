# syntax=docker/dockerfile:1.4

FROM baseimage

ARG KOHYA_VERSION
ARG VENV_PATH
ARG APP_VERSION

ENV APP_TYPE=kohya \
    VENV_PATH=${VENV_PATH} \
    TEMPLATE_VERSION=${APP_VERSION}

WORKDIR /workspace

# Clone Kohya_ss repository with submodules
RUN git clone --branch ${KOHYA_VERSION} --depth=1 --recurse-submodules --shallow-submodules \
    https://github.com/bmaltais/kohya_ss.git || \
    (git clone --depth=1 https://github.com/bmaltais/kohya_ss.git && \
     cd kohya_ss && \
     git checkout ${KOHYA_VERSION} && \
     git submodule update --init --recursive --depth=1) && \
    rm -rf kohya_ss/.git kohya_ss/sd-scripts/.git

WORKDIR /workspace/kohya_ss

# Create and activate venv with --system-site-packages to inherit base PyTorch/xformers
RUN python3 -m venv --system-site-packages ${VENV_PATH}
ENV PATH="${VENV_PATH}/bin:$PATH"

# Install Kohya_ss requirements with BuildKit cache mount
# Skip requirements_linux.txt to avoid CUDA version conflicts with base image
RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    pip3 install --no-cache-dir -r requirements.txt

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
