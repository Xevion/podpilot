/**
 * Log forwarding utility for capturing and forwarding process output.
 * Reads from process stdout/stderr and logs via structured logger.
 */

import { logger } from "./logger";

interface ForwardOptions {
  stdout?: boolean;
  stderr?: boolean;
}

/**
 * Forward process stdout/stderr to structured logger with service tagging.
 * Runs asynchronously in the background without blocking.
 */
export function forwardProcessLogs(
  proc: Bun.Subprocess,
  serviceName: string,
  options: ForwardOptions = { stdout: true, stderr: true }
): void {
  if (options.stdout && proc.stdout && typeof proc.stdout !== "number") {
    forwardStream(proc.stdout, serviceName, "stdout");
  }

  if (options.stderr && proc.stderr && typeof proc.stderr !== "number") {
    forwardStream(proc.stderr, serviceName, "stderr");
  }
}

/**
 * Read from a stream and forward each line to the logger.
 */
async function forwardStream(
  stream: ReadableStream<Uint8Array<ArrayBufferLike>>,
  serviceName: string,
  streamType: "stdout" | "stderr"
): Promise<void> {
  // Create service-specific logger once for reuse
  const serviceCategory = serviceToCategory(serviceName);
  const serviceLogger = logger.getChild(serviceCategory);

  try {
    const reader = stream.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();

      if (done) {
        // Log any remaining content in buffer
        if (buffer.trim()) {
          logLine(buffer, serviceName, streamType, serviceLogger);
        }
        break;
      }

      // Decode chunk and add to buffer
      buffer += decoder.decode(value, { stream: true });

      // Split by newlines and process complete lines
      const lines = buffer.split("\n");
      // Keep the last incomplete line in buffer
      buffer = lines.pop() || "";

      // Log each complete line
      for (const line of lines) {
        if (line.trim()) {
          logLine(line, serviceName, streamType, serviceLogger);
        }
      }
    }
  } catch (error) {
    serviceLogger.error("Error forwarding process logs", {
      stream: streamType,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

// Pattern matchers for debug-level messages
const TAILSCALED_DEBUG_PATTERNS = [
  /logtail/,
  /LogID:/,
  /logpolicy:/,
  /dns:/,
  /wgengine/,
  /blockEngineUpdates/,
  /control:/,
  /taildrop:/,
  /network-lock/,
  /peerapi:/,
  /tsdial:/,
  /monitor:/,
  /pm: migrating/,
  /got LocalBackend/,
  /magicsock: disco key/,
  /Creating\/Bringing WireGuard device/,
  /Engine created/,
  /endpoints changed:/,
  /\[RATELIMIT\]/,
];

const A1111_DEBUG_PATTERNS = [
  /Calculating sha256/,
  /^[a-f0-9]{64}$/, // Hex hashes
  /fatal: not a git repository/,
];

/**
 * Convert service name to kebab-case for logger category.
 */
function serviceToCategory(serviceName: string): string {
  return serviceName.toLowerCase();
}

/**
 * Log a single line with appropriate level using service-specific sublogger.
 */
function logLine(
  line: string,
  serviceName: string,
  streamType: "stdout" | "stderr",
  serviceLogger: ReturnType<typeof logger.getChild>
): void {
  // Strip tailscaled timestamp prefix (YYYY/MM/DD HH:MM:SS format)
  // TODO: Consider parsing and reusing the timestamp instead of just clipping
  const timestampPattern = /^\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}:\d{2} /;
  const processedLine = line.replace(timestampPattern, "");

  // Determine log level based on patterns
  let level: "debug" | "info" | "warn" | "error" = "info";

  // Check for debug patterns
  if (serviceName === "tailscaled") {
    if (
      TAILSCALED_DEBUG_PATTERNS.some((pattern) => pattern.test(processedLine))
    ) {
      level = "debug";
    }
  } else if (serviceName === "A1111") {
    if (A1111_DEBUG_PATTERNS.some((pattern) => pattern.test(processedLine))) {
      level = "debug";
    }
  }

  // Override level based on stream type and content (if not already debug)
  if (level !== "debug" && streamType === "stderr") {
    const lowerLine = processedLine.toLowerCase();
    if (
      lowerLine.includes("error") ||
      lowerLine.includes("fatal") ||
      lowerLine.includes("panic") ||
      lowerLine.includes("exception")
    ) {
      level = "error";
    } else {
      level = "warn";
    }
  }

  // Log with determined level (service is in logger category)
  serviceLogger[level](processedLine);
}
