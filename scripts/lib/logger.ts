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

  await configure({
    sinks: {
      stdout: getStreamSink(stdoutStream, {
        formatter: getJsonLinesFormatter(),
      }),
    },
    loggers: [
      {
        category: ["logtape", "meta"],
        lowestLevel: "warning",
      },
      {
        category: "podpilot",
        lowestLevel: logLevelMap[minLevel],
        sinks: ["stdout"],
      },
    ],
  });
};

// Initialize logging immediately
await configureLogging();

export const logger = getLogtapeLogger(["podpilot"]);
