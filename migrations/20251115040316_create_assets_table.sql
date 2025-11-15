-- Create assets table for generated files (images, videos, etc.)

CREATE TABLE assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    r2_key TEXT UNIQUE NOT NULL,
    filename TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    content_type TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    synced_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for listing agent's recent assets
CREATE INDEX idx_assets_agent_created ON assets (agent_id, created_at DESC);

-- Comment on table
COMMENT ON TABLE assets IS 'Generated assets (images, videos) uploaded to R2 storage';
COMMENT ON COLUMN assets.agent_id IS 'Agent that generated this asset (nullable after agent deletion)';
COMMENT ON COLUMN assets.r2_key IS 'S3/R2 object key (unique across all assets)';
COMMENT ON COLUMN assets.synced_at IS 'When the asset was uploaded to R2';
COMMENT ON COLUMN assets.metadata IS 'Generation parameters, prompts, model info, etc.';
