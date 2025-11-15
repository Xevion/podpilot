-- Create models table for Stable Diffusion checkpoints, LoRAs, embeddings, VAEs

CREATE TABLE models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    type model_type NOT NULL,
    r2_key TEXT UNIQUE NOT NULL,
    file_size BIGINT NOT NULL,
    hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for browsing models by type
CREATE INDEX idx_models_type_name ON models (type, name);

-- Comment on table
COMMENT ON TABLE models IS 'Model files stored in R2 (checkpoints, LoRAs, embeddings, VAEs)';
COMMENT ON COLUMN models.r2_key IS 'S3/R2 object key (unique across all models)';
COMMENT ON COLUMN models.hash IS 'SHA256 hash for deduplication and integrity verification';
