-- Create custom enum types for the database

-- Provider types for agent instances
CREATE TYPE provider_type AS ENUM (
    'vastai',
    'runpod',
    'local'
);

-- Agent status values
CREATE TYPE agent_status AS ENUM (
    'registering',
    'ready',
    'running',
    'idle',
    'error',
    'terminated'
);

-- Model types
CREATE TYPE model_type AS ENUM (
    'checkpoint',
    'lora',
    'embedding',
    'vae'
);
