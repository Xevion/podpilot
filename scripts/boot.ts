#!/usr/bin/env bun

import { logger } from "./lib/logger";
import { loadConfig } from "./lib/config";
import { initializeTailscale } from "./lib/tailscale";
import { launchApp } from "./lib/apps";
import { ensureAgent, startAgent } from "./lib/agent";

async function main(): Promise<void> {
  logger.info("PodPilot Agent boot script starting");

  // Load and validate configuration
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

  // Initialize Tailscale (daemon + optional network connection)
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

  const tailscaleProc = tailscaleResult.value;
  logger.info("Tailscale initialized successfully", { pid: tailscaleProc.pid });

  // Launch application and wait for readiness
  const appResult = await launchApp(config.appType);

  if (appResult.isErr) {
    logger.error("Application launch failed", {
      appType: config.appType,
      error: appResult.error,
    });
    process.exit(1);
  }

  const appProc = appResult.value;
  logger.info("Application launched successfully", {
    appType: config.appType,
    pid: appProc.pid,
  });

  // Ensure agent binary is available
  const agentPathResult = await ensureAgent(config);

  if (agentPathResult.isErr) {
    logger.error("Agent acquisition failed", {
      error: agentPathResult.error,
      source: config.agent.source,
    });
    process.exit(1);
  }

  const agentPath = agentPathResult.value;

  // Start PodPilot agent
  const agentResult = startAgent(agentPath);

  if (agentResult.isErr) {
    logger.error("Agent startup failed", {
      error: agentResult.error,
    });
    process.exit(1);
  }

  const agentProc = agentResult.value;
  logger.info("Agent started successfully", { pid: agentProc.pid });

  // Keep processes running and handle signals
  logger.info("Boot sequence complete - all processes running");
  logger.info("Process IDs", {
    tailscale: tailscaleProc.pid,
    app: appProc.pid,
    agent: agentProc.pid,
  });

  // Handle graceful shutdown
  const shutdown = async (signal: string) => {
    logger.info(`Received ${signal}, shutting down gracefully`);

    // Helper to kill process with timeout and fallback to SIGKILL
    const killWithTimeout = async (
      proc: Bun.Subprocess,
      name: string,
      timeoutMs: number = 5000
    ) => {
      logger.info(`Terminating ${name}`);
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

      if (result === "timeout") {
        logger.warn(`${name} did not exit gracefully, sending SIGKILL`);
        proc.kill(9); // SIGKILL
        await proc.exited;
      }

      logger.info(`${name} terminated`);
    };

    await killWithTimeout(agentProc, "agent");
    await killWithTimeout(appProc, "application");
    await killWithTimeout(tailscaleProc, "Tailscale");

    logger.info("Shutdown complete");
    process.exit(0);
  };

  // Track shutdown state to prevent double-shutdown
  let shuttingDown = false;

  process.on("SIGTERM", () => {
    if (!shuttingDown) {
      shuttingDown = true;
      shutdown("SIGTERM").catch((err) => {
        logger.error("Error during shutdown", { error: err });
        process.exit(1);
      });
    }
  });

  process.on("SIGINT", () => {
    if (!shuttingDown) {
      shuttingDown = true;
      shutdown("SIGINT").catch((err) => {
        logger.error("Error during shutdown", { error: err });
        process.exit(1);
      });
    }
  });

  // Wait for agent process to exit (should run indefinitely)
  const agentExitCode = await agentProc.exited;
  logger.error("Agent process exited unexpectedly", {
    exitCode: agentExitCode,
  });
  process.exit(agentExitCode);
}

main().catch((error) => {
  logger.error("Unhandled error in boot script", {
    error: error instanceof Error ? error : new Error(String(error)),
  });
  process.exit(1);
});
