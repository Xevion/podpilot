/**
 * Log forwarding utility for capturing and forwarding process output.
 * Reads from process stdout/stderr and logs via structured logger.
 */

import { getLogger } from "./logger";

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
    forwardStream(proc.stdout, serviceName, "stdout", proc.pid);
  }

  if (options.stderr && proc.stderr && typeof proc.stderr !== "number") {
    forwardStream(proc.stderr, serviceName, "stderr", proc.pid);
  }
}

/**
 * Read from a stream and forward each line to the logger.
 */
async function forwardStream(
  stream: ReadableStream<Uint8Array<ArrayBufferLike>>,
  serviceName: string,
  streamType: "stdout" | "stderr",
  pid: number | undefined
): Promise<void> {
  // Create independent service-specific logger
  const serviceLogger = getLogger([serviceName.toLowerCase()]);

  try {
    const reader = stream.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();

      if (done) {
        // Log any remaining content in buffer
        if (buffer.trim()) {
          logLine(buffer, serviceName, streamType, pid, serviceLogger);
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
          logLine(line, serviceName, streamType, pid, serviceLogger);
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
  // Low-level networking details (moved from INFO)
  /^magicsock: SetPrivateKey/,
  /^magicsock: home DERP/,
  /^magicsock: home is now/,
  /^magicsock: adding connection/,
  /^magicsock: \d+ active derp conns/,
  /^derphttp\.Client\.Connect:/,
  /^magicsock: derp-\d+ connected/,
  /^magicsock: disco: node/,
  // Connection lifecycle (moved from INFO)
  /client -> backend close connection:/,
  /backend -> client close connection:/,
  // Shutdown/cleanup (moved from INFO)
  /^flushing log/,
  /^logger closing down/,
  // Internal operations (moved from INFO)
  /^EditPrefs:/,
  /^resetDialPlan:/,
];

// Pattern matchers for Tailscale info-level messages (to avoid defaulting to warn on stderr)
const TAILSCALED_INFO_PATTERNS = [
  /^Program starting:/,
  /^link state:/,
  /^Bringing WireGuard device/,
  /^Bringing router/,
  /^Clearing router/,
  /^Starting network monitor/,
  /^Start$/,
  /^ipnext: active extensions:/,
  /^Backend: logs:/,
  /^Switching ipn state/,
  /^active login:/,
  /^generating new machine key/,
  /^machine key written/,
  /^StartLoginInteractiveAs/,
  /^ssh-conn-\d+/,
  /^ssh-session\(/,
  /^logged out ephemeral node/,
  /^canceling captive portal/,
  /^tailscaled got signal/,
  // Health warnings that are informational
  /^health\(warnable=/,
];

const A1111_DEBUG_PATTERNS = [
  /Calculating sha256/,
  /^[a-f0-9]{64}$/, // Hex hashes
  /fatal: not a git repository/,
];

// Pattern matchers for progress bars to filter
const PROGRESS_BAR_PATTERNS = [
  /^\s*\d+%\|[█\s▌▏▎▍▋▊▉]+\|/, // Standard progress bar with block characters
  /\d+\/\d+\s+\[[\d:]+<[\d:]+,.*it\/s\]/, // tqdm format
  /^Total progress:/, // A1111 specific progress messages
];

// Compiled combined regexes for performance (avoids O(n*m) pattern matching)
const TAILSCALED_DEBUG_COMBINED = new RegExp(
  TAILSCALED_DEBUG_PATTERNS.map((p) => `(${p.source})`).join("|")
);

const TAILSCALED_INFO_COMBINED = new RegExp(
  TAILSCALED_INFO_PATTERNS.map((p) => `(${p.source})`).join("|")
);

const A1111_DEBUG_COMBINED = new RegExp(A1111_DEBUG_PATTERNS.map((p) => `(${p.source})`).join("|"));

const PROGRESS_BAR_COMBINED = new RegExp(
  PROGRESS_BAR_PATTERNS.map((p) => `(${p.source})`).join("|")
);

/**
 * Strip ANSI escape codes from text (colors, cursor movement, etc.).
 */
function stripAnsi(text: string): string {
  // Remove ANSI escape sequences and carriage returns
  return text.replace(/\x1B\[[0-9;]*[A-Za-z]/g, "").replace(/\r/g, "");
}

/**
 * Sanitize log line to prevent log injection attacks.
 * Escapes newlines and removes control characters while preserving readability.
 */
function sanitizeLogLine(text: string): string {
  return (
    text
      // Escape newlines to prevent log injection
      .replace(/\n/g, "\\n")
      .replace(/\r/g, "\\r")
      // Remove control characters (except tab which is useful)
      .replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, "")
  );
}

// State tracking for multi-line Python tracebacks
let inTraceback = false;
let tracebackService = "";

/**
 * Log a single line with appropriate level using service-specific logger.
 */
function logLine(
  line: string,
  serviceName: string,
  streamType: "stdout" | "stderr",
  pid: number | undefined,
  serviceLogger: ReturnType<typeof getLogger>
): void {
  // Strip tailscaled timestamp prefix (YYYY/MM/DD HH:MM:SS format)
  const timestampPattern = /^\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}:\d{2} /;
  let processedLine = line.replace(timestampPattern, "");

  // Strip ANSI escape codes (colors, cursor movement, progress bars)
  processedLine = stripAnsi(processedLine);

  // Filter out progress bars entirely (too noisy)
  if (PROGRESS_BAR_COMBINED.test(processedLine)) {
    return; // Skip logging progress bars
  }

  // Determine log level based on patterns
  let level: "debug" | "info" | "warn" | "error" = "info";

  // Handle multi-line Python tracebacks
  if (/^\*\*\* Error|^Traceback \(most recent call last\):/.test(processedLine)) {
    inTraceback = true;
    tracebackService = serviceName;
    level = "error";
  } else if (inTraceback && serviceName === tracebackService) {
    // Continue traceback at error level until blank line
    if (processedLine.trim() === "") {
      inTraceback = false;
      tracebackService = "";
      return; // Skip blank line that ends traceback
    } else {
      level = "error";
    }
  } else {
    // Check for debug patterns using combined regexes for better performance
    if (serviceName === "tailscaled") {
      if (TAILSCALED_DEBUG_COMBINED.test(processedLine)) {
        level = "debug";
      } else if (TAILSCALED_INFO_COMBINED.test(processedLine)) {
        level = "info";
      }
    } else if (serviceName === "A1111") {
      if (A1111_DEBUG_COMBINED.test(processedLine)) {
        level = "debug";
      }
    }

    // Override level based on stream type and content (if not already debug/info)
    if (level !== "debug" && level !== "info" && streamType === "stderr") {
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
  }

  // Sanitize the line to prevent log injection
  const sanitizedLine = sanitizeLogLine(processedLine);

  // Log with determined level, including stream type and PID in context
  const logContext: Record<string, string | number> = {
    stream: streamType,
  };
  if (pid !== undefined) {
    logContext.pid = pid;
  }

  serviceLogger[level](sanitizedLine, logContext);
}
