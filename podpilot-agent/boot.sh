#!/bin/sh
set -e

# Start the Tailscale daemon in the background in userspace mode.
# This creates a HTTPS and SOCKS5 proxy on localhost port 1055 that your agent can use.
tailscaled \
  --tun=userspace-networking \
  --socks5-server=localhost:1055 \
  --outbound-http-proxy-listen=localhost:1055 \
  --socks5-server=localhost:1055 & 

# Bring Tailscale up using the auth key.
# The --accept-dns=false flag can prevent some DNS warnings.
# tailscale up \
#   --authkey=${TAILSCALE_AUTHKEY} \
#   --hostname=podpilot-agent \
#   --advertise-tags=tag:podpilot-agent \
#   --accept-dns=false \
#   --ssh
echo "Tailscale is up."

echo "Starting A1111 Web UI in the background..."
# SD Web UI runs on port 7860 with UI, or 7861 with --nowebui (API only)
# Set ENABLE_WEBUI=1 to enable the web interface
if [ "${ENABLE_WEBUI}" = "1" ]; then
  python3 /app/stable-diffusion-webui/launch.py --listen --xformers --enable-insecure-extension-access &
else
  python3 /app/stable-diffusion-webui/launch.py --listen --xformers --enable-insecure-extension-access --nowebui &
fi

ALL_PROXY=socks5://localhost:1055/ HTTP_PROXY=http://localhost:1055/ http_proxy=http://localhost:1055/ ${AGENT_BIN:-/app/podpilot-agent}