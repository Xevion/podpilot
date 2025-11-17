-- Add tailscale_ip column to agents table for agent identity tracking
ALTER TABLE agents ADD COLUMN IF NOT EXISTS tailscale_ip INET;

-- Create unique index for agent identity (tailscale_ip, provider_instance_id)
-- Only enforced for non-terminated agents with both fields set
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_identity
ON agents (tailscale_ip, provider_instance_id)
WHERE terminated_at IS NULL
  AND tailscale_ip IS NOT NULL
  AND provider_instance_id IS NOT NULL;

-- Add index on tailscale_ip for faster lookups
CREATE INDEX IF NOT EXISTS idx_agents_tailscale_ip
ON agents (tailscale_ip)
WHERE tailscale_ip IS NOT NULL;
