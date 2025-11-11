#!/bin/sh
set -e

# ============================================
# PODPILOT AGENT BOOT SCRIPT
# Shared across all PodPilot application images
# ============================================

echo "PodPilot Agent starting..."

# Start the Tailscale daemon in the background in userspace mode.
# This creates a HTTPS and SOCKS5 proxy on localhost port 1055.
# It also proxies TCP ports for the application and agent API.
echo "Starting Tailscale daemon..."
tailscaled \
  --tun=userspace-networking \
  --socks5-server=localhost:1055 \
  --outbound-http-proxy-listen=localhost:1055 \
  --state=mem: &

# Wait for Tailscale daemon to be ready
sleep 2

# Bring Tailscale up using the auth key if provided
if [ -n "${TAILSCALE_AUTHKEY}" ]; then
  echo "Connecting to Tailscale network..."
  tailscale up \
    --authkey=${TAILSCALE_AUTHKEY} \
    --hostname=${TAILSCALE_HOSTNAME:-podpilot-agent} \
    --advertise-tags=${TAILSCALE_TAGS:-tag:podpilot-agent} \
    --accept-dns=false \
    --ssh
  echo "Tailscale connected."
else
  echo "TAILSCALE_AUTHKEY not set, skipping Tailscale connection."
fi

# Start the application based on APP_TYPE environment variable
echo "Starting ${APP_TYPE:-application}..."
case "${APP_TYPE}" in
  a1111)
    echo "Launching A1111 Stable Diffusion WebUI..."
    cd /app/stable-diffusion-webui
    python3 launch.py --listen --xformers --enable-insecure-extension-access --skip-prepare-environment --skip-install &
    ;;

  comfyui)
    echo "Launching ComfyUI..."
    cd /workspace/ComfyUI
    python3 main.py --listen 0.0.0.0 --port 7860 &
    ;;

  fooocus)
    echo "Launching Fooocus..."
    cd /workspace/Fooocus
    python3 entry_with_update.py --listen 0.0.0.0 --port 7860 &
    ;;

  kohya)
    echo "Launching Kohya_ss..."
    cd /workspace/kohya_ss
    python3 kohya_gui.py --listen 0.0.0.0 --server_port 7860 &
    ;;

  *)
    echo "Unknown APP_TYPE: ${APP_TYPE}. Application not started."
    ;;
esac

# Give the application time to initialize
sleep 5

# Start the PodPilot agent with proxy configuration
echo "Starting PodPilot agent API server..."
ALL_PROXY=socks5://localhost:1055/ HTTP_PROXY=http://localhost:1055/ http_proxy=http://localhost:1055/ ${AGENT_BIN:-/app/podpilot-agent}
