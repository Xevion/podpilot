-- Create agent_models junction table for tracking which models are on which agents

CREATE TABLE agent_models (
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    model_id UUID NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    downloaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (agent_id, model_id)
);

-- Comment on table
COMMENT ON TABLE agent_models IS 'Many-to-many relationship tracking which models each agent has downloaded';
COMMENT ON COLUMN agent_models.downloaded_at IS 'When the agent successfully downloaded this model';
