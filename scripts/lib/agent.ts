/**
 * PodPilot agent startup logic.
 * Configures proxy environment and spawns the agent process.
 */

import { Result } from "true-myth";
import { logger } from "./logger";
import { spawnBackground } from "./process";

export class AgentError extends Error {
  constructor(
    message: string,
    public readonly cause?: Error
  ) {
    super(message);
    this.name = "AgentError";
  }
}

/**
 * Start the PodPilot agent with proxy configuration.
 * The agent will use the Tailscale SOCKS5/HTTP proxy for all network requests.
 */
export function startAgent(
  agentBinPath: string,
  tailscaleIp: string
): Result<Bun.Subprocess, AgentError> {
  logger.debug("Starting PodPilot agent with proxy configuration", { agentBinPath, tailscaleIp });

  const proxyEnv = {
    ALL_PROXY: "socks5://localhost:1055/",
    HTTP_PROXY: "http://localhost:1055/",
    http_proxy: "http://localhost:1055/",
    TAILSCALE_IP: tailscaleIp,
  };

  logger.debug("Agent environment configured", proxyEnv);

  const result = spawnBackground([agentBinPath], {
    env: proxyEnv,
    stdout: "inherit",
    stderr: "inherit",
  });

  if (result.isErr) {
    return Result.err(new AgentError("Failed to start PodPilot agent", result.error));
  }

  logger.info("PodPilot agent started", { pid: result.value.pid, tailscaleIp });
  return Result.ok(result.value);
}
