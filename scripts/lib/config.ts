/**
 * Configuration management with strict validation.
 * Uses true-myth Result types for error handling.
 */

import { Result } from "true-myth";

export type AppType = "a1111" | "comfyui" | "fooocus" | "kohya";
export type AgentSource = "embedded" | "download" | "local";

export interface Config {
  appType: AppType;
  tailscale: {
    authKey?: string;
    hostname: string;
    tags: string;
  };
  agent: {
    binPath: string;
    source: AgentSource;
    downloadUrl?: string;
  };
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

function validateAgentSource(value: string): Result<AgentSource, ConfigError> {
  const validSources: AgentSource[] = ["embedded", "download", "local"];
  if (!validSources.includes(value as AgentSource)) {
    return Result.err(
      new ConfigError(
        `Invalid AGENT_SOURCE: ${value}. Must be one of: ${validSources.join(", ")}`,
        "AGENT_SOURCE"
      )
    );
  }
  return Result.ok(value as AgentSource);
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

  // Validate agent source (defaults to "embedded" for backward compatibility)
  const agentSourceStr = getEnv("AGENT_SOURCE") || "embedded";
  const agentSourceResult = validateAgentSource(agentSourceStr);

  if (agentSourceResult.isErr) {
    return Result.err(agentSourceResult.error);
  }

  const agentSource = agentSourceResult.value;
  const agentBinPath = getEnv("AGENT_BIN") || "/app/podpilot-agent";
  const agentDownloadUrl = getEnv("AGENT_DOWNLOAD_URL");

  // Validate that download URL is provided when source is "download"
  if (agentSource === "download" && !agentDownloadUrl) {
    return Result.err(
      new ConfigError(
        "AGENT_DOWNLOAD_URL is required when AGENT_SOURCE is 'download'",
        "AGENT_DOWNLOAD_URL"
      )
    );
  }

  const authKey = getEnv("TAILSCALE_AUTHKEY");

  const config: Config = {
    appType: appTypeResult.value,
    tailscale: {
      ...(authKey ? { authKey } : {}),
      hostname: getEnv("TAILSCALE_HOSTNAME") || "podpilot-agent",
      tags: getEnv("TAILSCALE_TAGS") || "tag:podpilot-agent",
    },
    agent: {
      binPath: agentBinPath,
      source: agentSource,
      ...(agentDownloadUrl ? { downloadUrl: agentDownloadUrl } : {}),
    },
    logLevel: getEnv("LOG_LEVEL") || "info",
  };

  return Result.ok(config);
}
