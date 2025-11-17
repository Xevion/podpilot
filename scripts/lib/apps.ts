/**
 * Application launching logic with pattern matching.
 * Uses ts-pattern for exhaustive matching on APP_TYPE.
 */

import { Result } from "true-myth";
import { match } from "ts-pattern";
import { logger } from "./logger";
import { spawnBackground } from "./process";
import { waitForPort } from "./wait";
import { forwardProcessLogs } from "./log-forward";
import type { AppType } from "./config";

export class AppError extends Error {
  constructor(
    message: string,
    public readonly appType: AppType,
    public readonly cause?: Error
  ) {
    super(message);
    this.name = "AppError";
  }
}

interface AppConfig {
  name: string;
  cwd: string;
  command: string[];
  port: number;
}

/**
 * Get application configuration based on APP_TYPE.
 * Uses ts-pattern for exhaustive type-safe matching.
 */
function getAppConfig(appType: AppType): AppConfig {
  return match(appType)
    .with("a1111", () => ({
      name: "A1111",
      cwd: "/app/stable-diffusion-webui",
      command: [
        "python3",
        "launch.py",
        "--listen",
        "--xformers",
        "--enable-insecure-extension-access",
        "--skip-prepare-environment",
        "--skip-install",
      ],
      port: 7860,
    }))
    .with("comfyui", () => ({
      name: "ComfyUI",
      cwd: "/workspace/ComfyUI",
      command: ["python3", "main.py", "--listen", "0.0.0.0", "--port", "7860"],
      port: 7860,
    }))
    .with("fooocus", () => ({
      name: "Fooocus",
      cwd: "/workspace/Fooocus",
      command: ["python3", "entry_with_update.py", "--listen", "0.0.0.0", "--port", "7860"],
      port: 7860,
    }))
    .with("kohya", () => ({
      name: "Kohya",
      cwd: "/workspace/kohya_ss",
      command: ["python3", "kohya_gui.py", "--listen", "0.0.0.0", "--server_port", "7860"],
      port: 7860,
    }))
    .exhaustive();
}

/**
 * Launch the application based on APP_TYPE.
 * Spawns the process in the background and waits for the HTTP port to be ready.
 */
export async function launchApp(appType: AppType): Promise<Result<Bun.Subprocess, AppError>> {
  const config = getAppConfig(appType);

  logger.debug(`Launching ${config.name}`, { appType, cwd: config.cwd, port: config.port });

  const spawnResult = spawnBackground(config.command, {
    cwd: config.cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

  if (spawnResult.isErr) {
    return Result.err(new AppError(`Failed to spawn ${config.name}`, appType, spawnResult.error));
  }

  const proc = spawnResult.value;
  logger.debug(`${config.name} process started`, { appType, pid: proc.pid });

  // Forward process logs to structured logger
  forwardProcessLogs(proc, config.name);

  logger.debug(`Waiting for ${config.name} to be ready on port ${config.port}`, { appType });

  const waitResult = await waitForPort(config.port, "0.0.0.0", {
    timeoutMs: 120000, // 2 minutes - some apps take time to load models
    intervalMs: 1000, // Check every second
  });

  if (waitResult.isErr) {
    return Result.err(
      new AppError(
        `${config.name} did not become ready on port ${config.port}`,
        appType,
        waitResult.error
      )
    );
  }

  logger.info(`${config.name} is ready and listening on port ${config.port}`, { appType });
  return Result.ok(proc);
}
