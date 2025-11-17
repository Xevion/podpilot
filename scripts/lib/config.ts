/**
 * Configuration management with strict validation.
 * Uses true-myth Result types for error handling.
 */

import { Result } from "true-myth";

export type AppType = "a1111" | "comfyui" | "fooocus" | "kohya";

export interface Config {
  appType: AppType;
  tailscale: {
    authKey?: string;
    hostname: string;
    tags: string;
  };
  agentBin: string;
  logLevel: string;
}

export class ConfigError extends Error {
  constructor(
    message: string,
    public readonly field?: string,
  ) {
    super(message);
    this.name = "ConfigError";
  }
}

function getEnv(key: string): string | undefined {
  return process.env[key];
}

function getRequiredEnv(key: string): Result<string, ConfigError> {
  const value = getEnv(key);
  if (!value) {
    return Result.err(new ConfigError(`Missing required environment variable: ${key}`, key));
  }
  return Result.ok(value);
}

function validateAppType(value: string): Result<AppType, ConfigError> {
  const validTypes: AppType[] = ["a1111", "comfyui", "fooocus", "kohya"];
  if (!validTypes.includes(value as AppType)) {
    return Result.err(
      new ConfigError(
        `Invalid APP_TYPE: ${value}. Must be one of: ${validTypes.join(", ")}`,
        "APP_TYPE"
      )
    );
  }
  return Result.ok(value as AppType);
}

/**
 * Load and validate configuration from environment variables.
 * Fails early with clear error messages if misconfigured.
 */
export function loadConfig(): Result<Config, ConfigError> {
  const appTypeResult = getRequiredEnv("APP_TYPE").andThen(validateAppType);

  if (appTypeResult.isErr) {
    return Result.err(appTypeResult.error);
  }

  const authKey = getEnv("AGENT_AUTHKEY");

  const config: Config = {
    appType: appTypeResult.value,
    tailscale: {
      ...(authKey ? { authKey } : {}),
      hostname: require("os").hostname(),
      tags: "tag:podpilot-agent",
    },
    agentBin: getEnv("AGENT_BIN") || "/app/podpilot-agent",
    logLevel: getEnv("LOG_LEVEL") || "info",
  };

  return Result.ok(config);
}
