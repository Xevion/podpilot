/**
 * Tailscale daemon management with retry logic.
 * Handles starting the daemon, waiting for readiness, and connecting to the tailnet.
 */

import { Result } from "true-myth";
import { logger } from "./logger";
import { spawnBackground, spawnSync, ProcessError } from "./process";
import { waitForHttp } from "./wait";
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

const TAILSCALE_STATUS_URL = "http://localhost:41641/localapi/v0/status";
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
 * Polls the local API status endpoint.
 */
export async function waitForTailscaleDaemon(): Promise<Result<void, TailscaleError>> {
  logger.debug("Waiting for Tailscale daemon to be ready");

  const result = await waitForHttp(TAILSCALE_STATUS_URL, {
    timeoutMs: 10000, // 10 seconds
    intervalMs: 200, // Check every 200ms
  });

  if (result.isErr) {
    return Result.err(new TailscaleError("Tailscale daemon did not become ready in time", result.error));
  }

  logger.debug("Tailscale daemon is ready");
  return Result.ok(undefined);
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
 * Initialize Tailscale: start daemon, wait for readiness, and optionally connect.
 */
export async function initializeTailscale(
  authKey?: string,
  hostname: string = "podpilot-agent",
  tags: string = "tag:podpilot-agent"
): Promise<Result<Bun.Subprocess, TailscaleError>> {
  const daemonResult = startTailscaleDaemon();
  if (daemonResult.isErr) {
    return daemonResult;
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
    logger.info("No TAILSCALE_AUTHKEY provided, skipping network connection");
  }

  return daemonResult;
}
