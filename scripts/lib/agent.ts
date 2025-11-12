/**
 * PodPilot agent startup logic.
 * Configures proxy environment and spawns the agent process.
 */

import { Result } from "true-myth";
import { logger } from "./logger";
import { spawnBackground } from "./process";
import { downloadFile, fileExists } from "./download";
import type { Config } from "./config";

export class AgentError extends Error {
  constructor(
    message: string,
    public readonly cause?: Error,
  ) {
    super(message);
    this.name = "AgentError";
  }
}

/**
 * Ensure the agent binary is available.
 * Supports three modes:
 * - embedded: Verify binary exists at expected path (production)
 * - download: Download binary from R2/remote URL (dev)
 * - local: Use local binary from volume mount (local dev)
 */
export async function ensureAgent(config: Config): Promise<Result<string, AgentError>> {
  const { binPath, source, downloadUrl } = config.agent;

  logger.info("Ensuring agent binary is available", { source, binPath });

  switch (source) {
    case "embedded": {
      // Check if embedded binary exists
      const exists = await fileExists(binPath);
      if (!exists) {
        return Result.err(
          new AgentError(`Embedded agent binary not found at ${binPath}`)
        );
      }
      logger.info("Embedded agent binary verified", { binPath });
      return Result.ok(binPath);
    }

    case "local": {
      // Validate local path exists
      const exists = await fileExists(binPath);
      if (!exists) {
        return Result.err(
          new AgentError(`Local agent binary not found at ${binPath}`)
        );
      }
      logger.info("Local agent binary verified", { binPath });
      return Result.ok(binPath);
    }

    case "download": {
      if (!downloadUrl) {
        return Result.err(
          new AgentError("AGENT_DOWNLOAD_URL must be set when AGENT_SOURCE is 'download'")
        );
      }

      // Check if already downloaded
      const exists = await fileExists(binPath);
      if (exists) {
        logger.info("Agent binary already downloaded", { binPath });
        return Result.ok(binPath);
      }

      // Download the agent binary
      logger.info("Downloading agent binary", { url: downloadUrl, destination: binPath });
      const downloadResult = await downloadFile(downloadUrl, binPath);

      if (downloadResult.isErr) {
        return Result.err(
          new AgentError(
            `Failed to download agent: ${downloadResult.error.message}`,
            downloadResult.error
          )
        );
      }

      logger.info("Agent binary downloaded successfully", { binPath });
      return Result.ok(binPath);
    }

    default: {
      return Result.err(new AgentError(`Unknown agent source: ${source}`));
    }
  }
}

/**
 * Start the PodPilot agent with proxy configuration.
 * The agent will use the Tailscale SOCKS5/HTTP proxy for all network requests.
 */
export function startAgent(agentBinPath: string): Result<Bun.Subprocess, AgentError> {
  logger.debug("Starting PodPilot agent with proxy configuration", { agentBinPath });

  const proxyEnv = {
    ALL_PROXY: "socks5://localhost:1055/",
    HTTP_PROXY: "http://localhost:1055/",
    http_proxy: "http://localhost:1055/",
  };

  logger.debug("Agent proxy environment configured", proxyEnv);

  const result = spawnBackground([agentBinPath], {
    env: proxyEnv,
    stdout: "inherit",
    stderr: "inherit",
  });

  if (result.isErr) {
    return Result.err(new AgentError("Failed to start PodPilot agent", result.error));
  }

  logger.info("PodPilot agent started", { pid: result.value.pid });
  return Result.ok(result.value);
}
