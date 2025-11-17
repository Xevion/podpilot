/**
 * File download utilities for fetching agent binaries and other assets.
 * Uses Bun's built-in fetch API with retry logic and proper error handling.
 */

import { Result } from "true-myth";
import { logger } from "./logger";
import { chmod, unlink } from "node:fs/promises";

export class DownloadError extends Error {
  constructor(
    message: string,
    public readonly url: string,
    public readonly statusCode?: number
  ) {
    super(message);
    this.name = "DownloadError";
  }
}

export interface DownloadOptions {
  timeoutMs?: number;
  retries?: number;
  retryDelayMs?: number;
}

const DEFAULT_TIMEOUT_MS = 60000; // 60 seconds
const DEFAULT_RETRIES = 3;
const DEFAULT_RETRY_DELAY_MS = 1000; // 1 second

/**
 * Download a file from URL to destination path.
 * Automatically retries on failure with exponential backoff.
 * Makes the downloaded file executable (chmod +x).
 */
export async function downloadFile(
  url: string,
  destination: string,
  options?: DownloadOptions
): Promise<Result<string, DownloadError>> {
  const timeoutMs = options?.timeoutMs || DEFAULT_TIMEOUT_MS;
  const maxRetries = options?.retries || DEFAULT_RETRIES;
  const baseRetryDelay = options?.retryDelayMs || DEFAULT_RETRY_DELAY_MS;

  logger.info("Downloading file", { url, destination, timeoutMs });

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    const result = await attemptDownload(url, destination, timeoutMs);

    if (result.isOk) {
      // Make the file executable
      const chmodResult = await makeExecutable(destination);
      if (chmodResult.isErr) {
        // Clean up the downloaded file if chmod fails
        await unlink(destination).catch(() => {});
        return Result.err(chmodResult.error);
      }

      logger.info("Download completed successfully", { destination, attempt });
      return Result.ok(destination);
    }

    // Log the error and retry if we have attempts left
    const error = result.error;
    logger.warn("Download attempt failed", {
      url,
      attempt,
      maxRetries,
      error: error.message,
      statusCode: error.statusCode,
    });

    if (attempt < maxRetries) {
      // Exponential backoff: 1s, 2s, 4s, etc.
      const delayMs = baseRetryDelay * Math.pow(2, attempt - 1);
      logger.debug("Retrying download after delay", { delayMs });
      await Bun.sleep(delayMs);
    } else {
      logger.error("Download failed after all retries", {
        url,
        attempts: maxRetries,
        lastError: error.message,
      });
      return Result.err(error);
    }
  }

  // Should never reach here, but TypeScript needs this
  return Result.err(new DownloadError(`Download failed after ${maxRetries} attempts`, url));
}

/**
 * Single download attempt without retries.
 */
async function attemptDownload(
  url: string,
  destination: string,
  timeoutMs: number
): Promise<Result<void, DownloadError>> {
  try {
    // Fetch with timeout
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

    const response = await fetch(url, { signal: controller.signal });
    clearTimeout(timeoutId);

    // Check HTTP status
    if (!response.ok) {
      return Result.err(
        new DownloadError(`HTTP ${response.status}: ${response.statusText}`, url, response.status)
      );
    }

    // Write to file using Bun.write
    const data = await response.arrayBuffer();
    await Bun.write(destination, data);

    return Result.ok(undefined);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    return Result.err(new DownloadError(`Download failed: ${errorMessage}`, url));
  }
}

/**
 * Make a file executable (chmod +x).
 */
async function makeExecutable(filePath: string): Promise<Result<void, DownloadError>> {
  try {
    await chmod(filePath, 0o755);
    logger.debug("Made file executable", { filePath });
    return Result.ok(undefined);
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error("Failed to make file executable", { filePath, error: errorMessage });
    return Result.err(new DownloadError(`Failed to chmod file: ${errorMessage}`, filePath));
  }
}

/**
 * Check if a file exists.
 */
export async function fileExists(filePath: string): Promise<boolean> {
  try {
    const file = Bun.file(filePath);
    return await file.exists();
  } catch {
    return false;
  }
}
