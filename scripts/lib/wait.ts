/**
 * Smart waiting and polling utilities.
 * Replaces hardcoded sleep delays with readiness checks.
 */

import { Result } from "true-myth";
import { logger } from "./logger";

export class TimeoutError extends Error {
  constructor(
    message: string,
    public readonly timeoutMs: number,
  ) {
    super(message);
    this.name = "TimeoutError";
  }
}

export interface WaitOptions {
  timeoutMs?: number;
  intervalMs?: number;
}

const DEFAULT_TIMEOUT_MS = 30000; // 30 seconds
const DEFAULT_INTERVAL_MS = 500; // 500ms

/**
 * Poll a condition function until it returns true or timeout is reached.
 */
export async function waitFor(
  condition: () => Promise<boolean> | boolean,
  description: string,
  options?: WaitOptions
): Promise<Result<void, TimeoutError>> {
  const timeoutMs = options?.timeoutMs || DEFAULT_TIMEOUT_MS;
  const intervalMs = options?.intervalMs || DEFAULT_INTERVAL_MS;
  const startTime = Date.now();

  logger.debug("Waiting for condition", { description, timeoutMs, intervalMs });

  while (true) {
    const elapsed = Date.now() - startTime;

    if (elapsed >= timeoutMs) {
      logger.error("Wait timeout exceeded", { description, timeoutMs, elapsed });
      return Result.err(new TimeoutError(`Timeout waiting for: ${description}`, timeoutMs));
    }

    try {
      const result = await condition();
      if (result) {
        logger.debug("Condition met", { description, elapsed });
        return Result.ok(undefined);
      }
    } catch (error) {
      logger.debug("Condition check threw error", {
        description,
        error: error instanceof Error ? error.message : String(error),
      });
    }

    await Bun.sleep(intervalMs);
  }
}

/**
 * Wait for a TCP port to be listening.
 */
export async function waitForPort(
  port: number,
  host: string = "127.0.0.1",
  options?: WaitOptions
): Promise<Result<void, TimeoutError>> {
  return waitFor(
    async () => {
      try {
        void await Bun.connect({
          hostname: host,
          port,
          socket: {
            data() {},
            open(socket) {
              socket.end();
            },
          },
        });
        return true;
      } catch {
        return false;
      }
    },
    `Port ${port} on ${host} to be listening`,
    options
  );
}

/**
 * Wait for an HTTP endpoint to respond successfully (2xx status).
 */
export async function waitForHttp(
  url: string,
  options?: WaitOptions
): Promise<Result<void, TimeoutError>> {
  return waitFor(
    async () => {
      try {
        const response = await fetch(url, { method: "GET" });
        return response.ok;
      } catch {
        return false;
      }
    },
    `HTTP endpoint ${url} to respond`,
    options
  );
}

/**
 * Wait for a file to exist.
 */
export async function waitForFile(
  path: string,
  options?: WaitOptions
): Promise<Result<void, TimeoutError>> {
  return waitFor(
    async () => {
      try {
        const file = Bun.file(path);
        return await file.exists();
      } catch {
        return false;
      }
    },
    `File ${path} to exist`,
    options
  );
}

/**
 * Simple sleep utility (for when you actually need a fixed delay).
 */
export async function sleep(ms: number): Promise<void> {
  await Bun.sleep(ms);
}
