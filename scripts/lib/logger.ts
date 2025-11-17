/**
 * Structured JSON logging using LogTape.
 * Provides consistent, machine-parseable logs with timestamps and context.
 */

import {
  configure,
  getLogger as getLogtapeLogger,
  getStreamSink,
  getJsonLinesFormatter,
  type LogLevel as LogtapeLevel,
} from "@logtape/logtape";

export type LogLevel = "debug" | "info" | "warn" | "error";

// Mapping from our log levels to LogTape levels
const logLevelMap: Record<LogLevel, LogtapeLevel> = {
  debug: "debug",
  info: "info",
  warn: "warning",
  error: "error",
};

// Create a WritableStream wrapper for Bun.stdout
const stdoutStream = new WritableStream({
  write(chunk) {
    Bun.write(Bun.stdout, chunk);
  },
});

// Configure LogTape with JSON output to stdout
const configureLogging = async () => {
  const minLevel = (process.env.LOG_LEVEL as LogLevel) || "info";

  // All logger categories used in the system
  const categories = [
    "supervisor",
    "a1111",
    "comfyui",
    "fooocus",
    "kohya",
    "sshd",
    "tailscaled",
  ];

  await configure({
    sinks: {
      stdout: getStreamSink(stdoutStream, {
        formatter: getJsonLinesFormatter({ properties: "flatten" }),
      }),
    },
    loggers: [
      {
        category: ["logtape", "meta"],
        lowestLevel: "warning",
      },
      // Configure each category with the same settings
      ...categories.map((cat) => ({
        category: cat,
        lowestLevel: logLevelMap[minLevel],
        sinks: ["stdout"],
      })),
    ],
  });
};

// Initialize logging immediately
await configureLogging();

export const logger = getLogtapeLogger(["supervisor"]);
export const getLogger = getLogtapeLogger;
