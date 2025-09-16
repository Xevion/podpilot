#!/bin/sh
set -e

# Start the Tailscale daemon in the background in userspace mode.
# This creates a HTTPS and SOCKS5 proxy on localhost port 1055 that your agent can use.
# It also proxies the TCP ports 7860 and 8081 betwen the agent and the Tailscale network.
tailscaled \
  --tun=userspace-networking \
  --socks5-server=localhost:1055 \
  --outbound-http-proxy-listen=localhost:1055 \
  --proxied-tcp=7860 \
  --proxied-tcp=8081 &

# Bring Tailscale up using the auth key.
# The --accept-dns=false flag can prevent some DNS warnings.
tailscale up \
  --authkey=${TAILSCALE_AUTHKEY} \
  --hostname=podpilot-agent-$(hostname) \
  --advertise-tags=tag:podpilot-agent \
  --accept-dns=false \
  --ssh

echo "Tailscale is up in userspace mode. Starting PodPilot Agent..."

echo "Tailscale is up. Starting PodPilot Agent..."
ALL_PROXY=socks5://localhost:1055/ HTTP_PROXY=http://localhost:1055/ http_proxy=http://localhost:1055/ ${AGENT_BIN:-/app/podpilot-agent}