/**
 * Tailscale daemon management with retry logic.
 * Handles starting the daemon, waiting for readiness, and connecting to the tailnet.
 */

import { Result } from "true-myth";
import { logger } from "./logger";
import { spawnBackground, spawnSync, ProcessError } from "./process";
import { forwardProcessLogs } from "./log-forward";

export class TailscaleError extends Error {
  constructor(
    message: string,
    public readonly cause?: Error,
  ) {
    super(message);
    this.name = "TailscaleError";
  }
}

/**
 * Capture recent output from a subprocess for error diagnostics.
 * Reads up to the specified number of lines from stderr (and stdout if requested).
 */
async function captureDaemonLogs(
  proc: Bun.Subprocess,
  maxLines: number = 20
): Promise<string> {
  const logs: string[] = [];

  try {
    // Try to read from stderr first (most relevant for errors)
    if (proc.stderr && typeof proc.stderr !== "number") {
      const reader = proc.stderr.getReader();
      const chunks: Uint8Array[] = [];
      let totalBytes = 0;
      const maxBytes = 4096; // Limit to prevent memory issues

      try {
        while (totalBytes < maxBytes) {
          const { value, done } = await reader.read();
          if (done || !value) break;

          chunks.push(value);
          totalBytes += value.length;
        }
      } finally {
        reader.releaseLock();
      }

      if (chunks.length > 0) {
        const combined = new Uint8Array(totalBytes);
        let offset = 0;
        for (const chunk of chunks) {
          combined.set(chunk, offset);
          offset += chunk.length;
        }

        const text = new TextDecoder().decode(combined);
        const lines = text.split("\n").filter((line) => line.trim() !== "");
        logs.push(...lines.slice(-maxLines));
      }
    }
  } catch (error) {
    // Best effort - if we can't read logs, continue without them
    logger.debug("Failed to capture daemon logs", {
      error: error instanceof Error ? error.message : String(error),
    });
  }

  if (logs.length === 0) {
    return "(no logs available)";
  }

  return logs.join("\n");
}

const MAX_RETRIES = 5;
const INITIAL_BACKOFF_MS = 1000;

/**
 * Start the Tailscale daemon in userspace networking mode.
 * Creates a SOCKS5/HTTP proxy on localhost:1055.
 */
export function startTailscaleDaemon(): Result<Bun.Subprocess, TailscaleError> {
  logger.debug("Starting Tailscale daemon in userspace mode");

  const result = spawnBackground(
    [
      "tailscaled",
      "--tun=userspace-networking",
      "--socks5-server=localhost:1055",
      "--outbound-http-proxy-listen=localhost:1055",
      "--state=mem:",
    ],
    {
      stdout: "pipe",
      stderr: "pipe",
    }
  );

  if (result.isErr) {
    return Result.err(new TailscaleError("Failed to start Tailscale daemon", result.error));
  }

  const proc = result.value;
  logger.debug("Tailscale daemon started", { pid: proc.pid });

  // Forward process logs to structured logger
  forwardProcessLogs(proc, "tailscaled");

  return Result.ok(proc);
}

/**
 * Wait for the Tailscale daemon to be ready.
 * Polls the CLI status command with timing information.
 */
export async function waitForTailscaleDaemon(
  daemonProcess?: Bun.Subprocess
): Promise<Result<void, TailscaleError>> {
  logger.debug("Waiting for Tailscale daemon to be ready");

  const maxAttempts = 50;
  const intervalMs = 200;
  const startTime = performance.now();
  let lastError: string = "";

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    const proc = Bun.spawnSync(["tailscale", "status"]);

    if (proc.success) {
      const durationMs = Math.round(performance.now() - startTime);
      logger.debug("Tailscale daemon is ready", { attempt, durationMs });
      return Result.ok(undefined);
    }

    // Capture last error for diagnostics
    lastError = new TextDecoder().decode(proc.stderr).trim();

    if (attempt < maxAttempts) {
      await Bun.sleep(intervalMs);
    }
  }

  const durationMs = Math.round(performance.now() - startTime);
  const totalTimeoutMs = maxAttempts * intervalMs;

  let errorMsg = `Tailscale daemon did not become ready after ${maxAttempts} attempts (${durationMs}ms elapsed, ${totalTimeoutMs}ms timeout)`;

  if (lastError) {
    errorMsg += `\n\nLast error: ${lastError}`;
  }

  // If we have the daemon process, try to capture recent logs
  if (daemonProcess) {
    try {
      const recentLogs = await captureDaemonLogs(daemonProcess, 20);
      errorMsg += `\n\nRecent daemon logs:\n${recentLogs}`;
    } catch {
      // Best effort
    }
  }

  errorMsg += `\n\nTroubleshooting:
  - Check if tailscaled has the necessary permissions
  - Verify network connectivity
  - Ensure /var/run/tailscale directory is writable`;

  return Result.err(new TailscaleError(errorMsg));
}

/**
 * Connect to the Tailscale network with retry and exponential backoff.
 */
export async function connectToTailnet(
  authKey: string,
  hostname: string,
  tags: string
): Promise<Result<void, TailscaleError>> {
  logger.info("Connecting to Tailscale network", { hostname, tags });

  let lastError: ProcessError | undefined;
  let backoffMs = INITIAL_BACKOFF_MS;

  for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
    logger.info(`Tailscale connection attempt ${attempt}/${MAX_RETRIES}`, { backoffMs });

    const result = await spawnSync([
      "tailscale",
      "up",
      `--authkey=${authKey}`,
      `--hostname=${hostname}`,
      `--advertise-tags=${tags}`,
      "--accept-dns=false",
      "--ssh",
    ]);

    if (result.isOk) {
      logger.info("Successfully connected to Tailscale network", { hostname });
      return Result.ok(undefined);
    }

    lastError = result.error;
    logger.warn(`Tailscale connection attempt ${attempt} failed`, {
      attempt,
      error: result.error.message,
      exitCode: result.error.exitCode,
    });

    if (attempt < MAX_RETRIES) {
      logger.debug(`Retrying in ${backoffMs}ms...`, { attempt, nextBackoff: backoffMs });
      await Bun.sleep(backoffMs);
      backoffMs *= 2; // Exponential backoff
    }
  }

  const errorMessage = `Failed to connect to Tailscale after ${MAX_RETRIES} attempts`;
  logger.error(errorMessage, { lastError: lastError?.message });
  return Result.err(new TailscaleError(errorMessage, lastError));
}

/**
 * Get the Tailscale IP address using the CLI.
 */
export async function getTailscaleIp(): Promise<Result<string, TailscaleError>> {
  logger.debug("Fetching Tailscale IP via CLI");

  const proc = Bun.spawnSync(["tailscale", "status", "--json"]);

  if (!proc.success) {
    const stderr = new TextDecoder().decode(proc.stderr);
    return Result.err(
      new TailscaleError(`Failed to get Tailscale status: ${stderr.trim()}`)
    );
  }

  try {
    const status = JSON.parse(new TextDecoder().decode(proc.stdout));
    const ip = status?.Self?.TailscaleIPs?.[0];

    if (!ip) {
      return Result.err(new TailscaleError("No Tailscale IP found in status output"));
    }

    logger.info("Detected Tailscale IP", { ip });
    return Result.ok(ip);
  } catch (error) {
    return Result.err(
      new TailscaleError(
        `Failed to parse Tailscale status output: ${error instanceof Error ? error.message : String(error)}`
      )
    );
  }
}

/**
 * Initialize Tailscale: start daemon, wait for readiness, and optionally connect.
 * Returns both the daemon process and the Tailscale IP address.
 */
export async function initializeTailscale(
  authKey?: string,
  hostname: string = "podpilot-agent",
  tags: string = "tag:podpilot-agent"
): Promise<Result<{ process: Bun.Subprocess; ip: string }, TailscaleError>> {
  const daemonResult = startTailscaleDaemon();
  if (daemonResult.isErr) {
    return Result.err(daemonResult.error);
  }

  // Wait 2 seconds for daemon to initialize (matches old bash script behavior)
  logger.debug("Waiting 2 seconds for Tailscale daemon to initialize");
  await Bun.sleep(2000);

  if (authKey) {
    const connectResult = await connectToTailnet(authKey, hostname, tags);
    if (connectResult.isErr) {
      return Result.err(connectResult.error);
    }
  } else {
    logger.info("No AGENT_AUTHKEY provided, skipping network connection");
  }

  // Auto-detect Tailscale IP
  const ipResult = await getTailscaleIp();
  if (ipResult.isErr) {
    return Result.err(ipResult.error);
  }

  return Result.ok({ process: daemonResult.value, ip: ipResult.value });
}
