#!/bin/sh

# Exit immediately if a command exits with a non-zero status.
set -e

# Start the Tailscale daemon in the background
# --state=mem: is crucial for containers; it stores state in memory.
tailscaled --state=mem: &

# Bring Tailscale up using an ephemeral auth key from an environment variable.
# Advertise the correct tag for our ACLs.
tailscale up \
  --authkey=${TAILSCALE_AUTHKEY} \
  --hostname=podpilot-agent-$(hostname) \
  --advertise-tags=tag:podpilot-agent \
  --accept-dns=false \
  --ssh

# Now that the network is up, run the main podpilot agent process
# The agent will now be able to connect to "podpilot-hub:8080"
echo "Tailscale is up. Starting PodPilot Agent..."
${AGENT_BIN:-/app/podpilot-agent}