#!/bin/sh
set -e

# Start the Tailscale daemon in the background in userspace mode.
# This creates a SOCKS5 proxy on localhost port 1055 that your agent can use.
tailscaled --tun=userspace-networking --socks5-server=localhost:1055 &

# Bring Tailscale up using the auth key.
# The --accept-dns=false flag can prevent some DNS warnings.
tailscale up \
  --authkey=${TAILSCALE_AUTHKEY} \
  --hostname=podpilot-agent-$(hostname) \
  --advertise-tags=tag:podpilot-agent \
  --accept-dns=false \
  --ssh

echo "Tailscale is up in userspace mode. Starting PodPilot Agent..."

# IMPORTANT: Set the proxy environment variable for the agent process.
export ALL_PROXY=socks5://localhost:1055
export HTTPS_PROXY=socks5://localhost:1055

# Now that the network is up, run the main podpilot agent process
# The agent will now be able to connect to "podpilot-hub:8080"
echo "Tailscale is up. Starting PodPilot Agent..."
${AGENT_BIN:-/app/podpilot-agent}