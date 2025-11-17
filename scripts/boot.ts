#!/usr/bin/env bun

/**
 * PodPilot Agent Boot Script
 * Main entrypoint for all PodPilot application Docker images.
 *
 * This script orchestrates the startup sequence:
 * 1. Load and validate configuration
 * 2. Start Tailscale daemon
 * 3. Wait for Tailscale readiness
 * 4. Connect to Tailscale network (if auth key provided)
 * 5. Start SSH daemon for remote access
 * 6. Launch application based on APP_TYPE
 * 7. Wait for application readiness
 * 8. Start PodPilot agent
 * 9. Keep all processes running
 */

import { logger } from "./lib/logger";
import { loadConfig } from "./lib/config";
import { initializeTailscale } from "./lib/tailscale";
import { launchApp } from "./lib/apps";
import { startAgent } from "./lib/agent";
import { spawnBackground } from "./lib/process";
import { forwardProcessLogs } from "./lib/log-forward";
import { existsSync, writeFileSync } from "fs";

/**
 * Fetch SSH public keys from GitHub for a given username.
 */
async function fetchGitHubKeys(username: string): Promise<string[]> {
  const url = `https://github.com/${username}.keys`;
  logger.debug(`Fetching SSH keys from GitHub for ${username}`);

  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 10000); // 10s timeout

    const response = await fetch(url, { signal: controller.signal });
    clearTimeout(timeoutId);

    if (!response.ok) {
      logger.warn(`Failed to fetch GitHub keys for ${username}`, {
        status: response.status,
      });
      return [];
    }

    const text = await response.text();
    const keys = text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    logger.info(`Fetched ${keys.length} SSH keys from GitHub`, { username });
    return keys;
  } catch (error) {
    logger.warn(`Error fetching GitHub keys for ${username}`, {
      error: error instanceof Error ? error.message : String(error),
    });
    return [];
  }
}

/**
 * Setup SSH authorized_keys from environment variable or mounted file.
 * Priority: mounted file > environment variable
 * Supports GitHub username fetching: github.com/username
 */
async function setupAuthorizedKeys(): Promise<void> {
  const sshDir = "/root/.ssh";
  const authorizedKeysPath = "/root/.ssh/authorized_keys";

  // Ensure .ssh directory exists
  if (!existsSync(sshDir)) {
    const fs = await import("fs/promises");
    await fs.mkdir(sshDir, { mode: 0o700, recursive: true });
    logger.debug("Created /root/.ssh directory");
  }

  if (existsSync(authorizedKeysPath)) {
    logger.info("Using existing authorized_keys file");
    return;
  }

  const sshKeysEnv = process.env.SSH_AUTHORIZED_KEYS;
  if (!sshKeysEnv) {
    logger.warn("No SSH keys configured (SSH_AUTHORIZED_KEYS not set and no mounted file)");
    return;
  }

  logger.info("Processing SSH_AUTHORIZED_KEYS");
  const lines = sshKeysEnv.split("\n").map((line) => line.trim());
  const allKeys: string[] = [];

  for (const line of lines) {
    if (!line) continue;

    const githubMatch = line.match(/^github\.com\/([a-zA-Z0-9-]+)$/);
    if (githubMatch && githubMatch[1]) {
      const username = githubMatch[1];
      const keys = await fetchGitHubKeys(username);
      allKeys.push(...keys);
    } else {
      allKeys.push(line);
    }
  }

  if (allKeys.length === 0) {
    logger.warn("No valid SSH keys found after processing");
    return;
  }

  const uniqueKeys = [...new Set(allKeys)];
  logger.info(`Writing ${uniqueKeys.length} unique SSH keys to authorized_keys`);

  writeFileSync(authorizedKeysPath, uniqueKeys.join("\n") + "\n", {
    mode: 0o600,
  });
  logger.debug("authorized_keys written successfully");
}

async function main(): Promise<void> {
  const startTime = performance.now();
  const timings: Record<string, number> = {};

  logger.info("PodPilot Agent boot script starting");

  // Step 1: Load and validate configuration
  logger.debug("Loading configuration from environment");
  const configResult = loadConfig();

  if (configResult.isErr) {
    logger.error("Configuration validation failed", {
      error: configResult.error,
      field: configResult.error.field,
    });
    process.exit(1);
  }

  const config = configResult.value;
  logger.info("Configuration loaded successfully", {
    appType: config.appType,
    tailscaleHostname: config.tailscale.hostname,
    hasAuthKey: !!config.tailscale.authKey,
  });

  // Shutdown state and promise for signal handling
  let shuttingDown = false;
  let shutdownResolver: (() => void) | undefined;
  const shutdownPromise = new Promise<void>((resolve) => {
    shutdownResolver = resolve;
  });

  // Store process references for cleanup
  let tailscaleProc: Bun.Subprocess | undefined;
  let sshdProc: Bun.Subprocess | undefined;
  let appProc: Bun.Subprocess | undefined;
  let agentProc: Bun.Subprocess | undefined;

  // Handle graceful shutdown
  const shutdown = async (signal: string) => {
    const shutdownStart = performance.now();
    logger.info("initiating shutdown", { signal });

    // Track per-process shutdown times
    const shutdownTimings: Record<string, number> = {};

    // Helper to kill process with timeout and fallback to SIGKILL
    const killWithTimeout = async (
      proc: Bun.Subprocess,
      name: string,
      timeoutMs: number = 5000
    ): Promise<void> => {
      const killStart = performance.now();
      logger.debug("terminating process", {
        process: name,
        pid: proc.pid,
      });
      proc.kill(); // Send SIGTERM

      // Wait for process to exit with timeout
      const exitPromise = proc.exited;
      const timeoutPromise = new Promise<void>((resolve) =>
        setTimeout(() => resolve(), timeoutMs)
      );

      const result = await Promise.race([
        exitPromise.then(() => "exited"),
        timeoutPromise.then(() => "timeout"),
      ]);

      const killDuration = performance.now() - killStart;
      shutdownTimings[name] = Math.round(killDuration);

      if (result === "timeout") {
        logger.warn("process did not exit gracefully, forcing termination", {
          process: name,
          pid: proc.pid,
          signal: "SIGKILL",
          durationMs: Math.round(killDuration),
        });
        proc.kill(9); // SIGKILL
        await proc.exited;
      } else {
        logger.info("process terminated", {
          process: name,
          pid: proc.pid,
          graceful: true,
          durationMs: Math.round(killDuration),
        });
      }
    };

    if (agentProc) await killWithTimeout(agentProc, "agent");
    if (appProc) await killWithTimeout(appProc, "application");
    if (sshdProc) await killWithTimeout(sshdProc, "sshd");
    if (tailscaleProc) await killWithTimeout(tailscaleProc, "tailscale");

    const totalShutdownDuration = performance.now() - shutdownStart;
    logger.info("shutdown complete", {
      totalDurationMs: Math.round(totalShutdownDuration),
      breakdown: shutdownTimings,
      signal,
      graceful: true,
    });
    process.exit(0);
  };

  // Signal handlers for graceful shutdown
  process.on("SIGTERM", () => {
    if (!shuttingDown) {
      shuttingDown = true;
      shutdownResolver?.();
      shutdown("SIGTERM").catch((err) => {
        logger.error("shutdown error", {
          error: err instanceof Error ? err.message : String(err),
          errorType: err instanceof Error ? err.name : "unknown",
        });
        process.exit(1);
      });
    }
  });

  process.on("SIGINT", () => {
    if (!shuttingDown) {
      shuttingDown = true;
      shutdownResolver?.();
      shutdown("SIGINT").catch((err) => {
        logger.error("shutdown error", {
          error: err instanceof Error ? err.message : String(err),
          errorType: err instanceof Error ? err.name : "unknown",
        });
        process.exit(1);
      });
    }
  });

  // Step 2-4: Initialize Tailscale (daemon + optional network connection)
  const tailscaleResult = await initializeTailscale(
    config.tailscale.authKey,
    config.tailscale.hostname,
    config.tailscale.tags
  );

  if (tailscaleResult.isErr) {
    logger.error("Tailscale initialization failed", {
      error: tailscaleResult.error,
    });
    process.exit(1);
  }

  const { process: tailscaleProcess, ip: tailscaleIp } = tailscaleResult.value;
  tailscaleProc = tailscaleProcess;
  timings.tailscale = performance.now() - startTime;
  logger.debug("Tailscale initialization complete", {
    durationMs: Math.round(timings.tailscale),
  });

  // Check if shutdown was initiated during Tailscale initialization
  if (shuttingDown) {
    logger.info("Shutdown initiated, exiting early");
    return;
  }

  // Start SSH daemon for remote access over Tailscale
  logger.debug("Starting SSH daemon");
  const sshStartTime = performance.now();

  try {
    await setupAuthorizedKeys();
  } catch (error) {
    logger.warn("Error setting up authorized_keys", {
      error: error instanceof Error ? error.message : String(error),
    });
  }

  const sshdResult = spawnBackground(["/usr/sbin/sshd", "-D", "-e"], {
    stdout: "pipe",
    stderr: "pipe",
  });

  if (sshdResult.isErr) {
    logger.warn("Failed to start SSH daemon (SSH will be unavailable)", {
      error: sshdResult.error,
    });
  } else {
    sshdProc = sshdResult.value;
    forwardProcessLogs(sshdProc, "sshd");
    timings.ssh = performance.now() - sshStartTime;
    logger.debug("SSH daemon started", { durationMs: Math.round(timings.ssh) });
  }

  // Step 5-6: Launch application and wait for readiness
  const appStartTime = performance.now();
  const appResult = await launchApp(config.appType);

  if (appResult.isErr) {
    logger.error("Application launch failed", {
      appType: config.appType,
      error: appResult.error,
    });
    process.exit(1);
  }

  appProc = appResult.value;
  timings.app = performance.now() - appStartTime;
  logger.debug("Application launch complete", {
    durationMs: Math.round(timings.app),
  });

  // Check if shutdown was initiated during app launch
  if (shuttingDown) {
    logger.info("Shutdown initiated, exiting early");
    return;
  }

  // Step 7: Start PodPilot agent
  const agentStartTime = performance.now();
  const agentResult = startAgent(config.agentBin, tailscaleIp);

  if (agentResult.isErr) {
    logger.error("Agent startup failed", {
      error: agentResult.error,
    });
    process.exit(1);
  }

  agentProc = agentResult.value;
  timings.agent = performance.now() - agentStartTime;
  const totalDuration = performance.now() - startTime;

  // Step 8: Keep processes running and handle signals
  logger.info("Boot sequence complete", {
    totalDurationMs: Math.round(totalDuration),
    phases: {
      tailscaleMs: Math.round(timings.tailscale),
      sshMs: timings.ssh ? Math.round(timings.ssh) : undefined,
      appMs: Math.round(timings.app),
      agentMs: Math.round(timings.agent),
    },
    processes: {
      tailscale: { pid: tailscaleProc.pid, ip: tailscaleIp },
      sshd: sshdProc ? { pid: sshdProc.pid } : "not running",
      app: { type: config.appType, pid: appProc.pid },
      agent: { pid: agentProc.pid },
    },
  });

  // Wait for either agent exit or shutdown signal
  const result = await Promise.race([
    agentProc.exited.then((code) => ({ type: "exit" as const, code })),
    shutdownPromise.then(() => ({ type: "shutdown" as const })),
  ]);

  if (result.type === "exit") {
    logger.error("Agent process exited unexpectedly", {
      exitCode: result.code,
    });
    process.exit(result.code);
  }

  // If we reach here, shutdown was initiated via signal
  // The signal handler will handle cleanup
  logger.debug("Main execution complete, waiting for shutdown to finish");
}

main().catch((error) => {
  logger.error("Unhandled error in boot script", {
    error: error instanceof Error ? error : new Error(String(error)),
  });
  process.exit(1);
});
