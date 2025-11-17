/**
 * Process spawning utilities with proper error handling.
 * Uses true-myth Result types for error handling.
 */

import { Result } from "true-myth";
import { logger } from "./logger";

export class ProcessError extends Error {
  constructor(
    message: string,
    public readonly command: string,
    public readonly exitCode?: number,
  ) {
    super(message);
    this.name = "ProcessError";
  }
}

export interface SpawnOptions {
  cwd?: string;
  env?: Record<string, string>;
  stdin?: "inherit" | "pipe" | "ignore";
  stdout?: "inherit" | "pipe" | "ignore";
  stderr?: "inherit" | "pipe" | "ignore";
}

/**
 * Helper to create a ProcessError from a caught exception.
 */
function handleSpawnError(error: unknown, commandStr: string): ProcessError {
  const errorMessage = error instanceof Error ? error.message : String(error);
  logger.error("Failed to spawn process", {
    command: commandStr,
    error: errorMessage,
  });
  return new ProcessError(`Failed to spawn process: ${errorMessage}`, commandStr);
}

/**
 * Spawn a process in the background and return immediately.
 * The process continues running after this function returns.
 */
export function spawnBackground(
  command: string[],
  options?: SpawnOptions
): Result<Bun.Subprocess, ProcessError> {
  const commandStr = command.join(" ");

  try {
    const [cmd, ...args] = command;
    if (!cmd) {
      return Result.err(new ProcessError("Empty command array", commandStr));
    }

    logger.debug("Spawning background process", { command: commandStr, cwd: options?.cwd });

    const proc = Bun.spawn([cmd, ...args], {
      ...(options?.cwd ? { cwd: options.cwd } : {}),
      env: { ...process.env, ...options?.env },
      stdin: options?.stdin || "ignore",
      stdout: options?.stdout || "inherit",
      stderr: options?.stderr || "inherit",
    });

    logger.debug("Background process spawned", { command: commandStr, pid: proc.pid });

    return Result.ok(proc);
  } catch (error) {
    return Result.err(handleSpawnError(error, commandStr));
  }
}

/**
 * Spawn a process and wait for it to complete.
 * Returns Result with stdout/stderr or error.
 */
export async function spawnSync(
  command: string[],
  options?: SpawnOptions
): Promise<Result<{ stdout: string; stderr: string; exitCode: number }, ProcessError>> {
  const commandStr = command.join(" ");

  try {
    const [cmd, ...args] = command;
    if (!cmd) {
      return Result.err(new ProcessError("Empty command array", commandStr));
    }

    logger.debug("Spawning synchronous process", { command: commandStr, cwd: options?.cwd });

    const proc = Bun.spawn([cmd, ...args], {
      ...(options?.cwd ? { cwd: options.cwd } : {}),
      env: { ...process.env, ...options?.env },
      stdin: options?.stdin || "ignore",
      stdout: "pipe",
      stderr: "pipe",
    });

    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    if (exitCode !== 0) {
      const stderrPreview = stderr.slice(0, 500);
      const truncated = stderr.length > 500;

      logger.warn("Process exited with non-zero code", {
        command: commandStr,
        exitCode,
        stderrPreview,
        stderrLength: stderr.length,
        truncated,
      });

      const errorMsg = truncated
        ? `Process exited with code ${exitCode}: ${stderrPreview}... (${stderr.length - 500} more chars)`
        : `Process exited with code ${exitCode}: ${stderrPreview}`;

      return Result.err(new ProcessError(errorMsg, commandStr, exitCode));
    }

    logger.debug("Process completed successfully", { command: commandStr, exitCode });

    return Result.ok({ stdout, stderr, exitCode });
  } catch (error) {
    return Result.err(handleSpawnError(error, commandStr));
  }
}
