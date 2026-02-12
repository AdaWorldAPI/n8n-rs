-- n8n-rust PostgreSQL Schema
-- Faithful translation of n8n TypeORM entities
-- Migration: 001_initial_schema

-- =============================================================================
-- EXTENSIONS
-- =============================================================================
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- =============================================================================
-- SETTINGS (Global key-value store)
-- =============================================================================
CREATE TABLE IF NOT EXISTS settings (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    load_on_startup BOOLEAN NOT NULL DEFAULT false
);

-- =============================================================================
-- ROLE (User roles)
-- =============================================================================
CREATE TABLE IF NOT EXISTS role (
    id SERIAL PRIMARY KEY,
    name VARCHAR(32) NOT NULL UNIQUE,
    scope VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default roles
INSERT INTO role (name, scope) VALUES
    ('global:owner', 'global'),
    ('global:admin', 'global'),
    ('global:member', 'global')
ON CONFLICT (name) DO NOTHING;

-- =============================================================================
-- USER
-- =============================================================================
CREATE TABLE IF NOT EXISTS "user" (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(254) UNIQUE,
    first_name VARCHAR(32),
    last_name VARCHAR(32),
    password TEXT,
    personalization_answers JSONB,
    settings JSONB,
    disabled BOOLEAN NOT NULL DEFAULT false,
    mfa_enabled BOOLEAN NOT NULL DEFAULT false,
    mfa_secret TEXT,
    mfa_recovery_codes TEXT[] DEFAULT '{}',
    last_active_at DATE,
    role_id INTEGER REFERENCES role(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_email ON "user"(email);

-- =============================================================================
-- PROJECT (Team/personal projects)
-- =============================================================================
CREATE TABLE IF NOT EXISTS project (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    name VARCHAR(255) NOT NULL,
    type VARCHAR(36) NOT NULL DEFAULT 'personal' CHECK (type IN ('personal', 'team')),
    icon JSONB,
    description VARCHAR(512),
    creator_id UUID REFERENCES "user"(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- PROJECT_RELATION (Project membership)
-- =============================================================================
CREATE TABLE IF NOT EXISTS project_relation (
    project_id VARCHAR(36) NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    role VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

-- =============================================================================
-- FOLDER (Workflow organization)
-- =============================================================================
CREATE TABLE IF NOT EXISTS folder (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    name VARCHAR(128) NOT NULL,
    parent_folder_id VARCHAR(36) REFERENCES folder(id) ON DELETE CASCADE,
    project_id VARCHAR(36) REFERENCES project(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_folder_parent ON folder(parent_folder_id);
CREATE INDEX idx_folder_project ON folder(project_id);

-- =============================================================================
-- WORKFLOW_HISTORY (Version tracking)
-- =============================================================================
CREATE TABLE IF NOT EXISTS workflow_history (
    version_id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 36),
    workflow_id VARCHAR(36) NOT NULL,
    nodes JSONB NOT NULL DEFAULT '[]',
    connections JSONB NOT NULL DEFAULT '{}',
    authors TEXT,
    name TEXT,
    description TEXT,
    autosaved BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workflow_history_workflow ON workflow_history(workflow_id);

-- =============================================================================
-- WORKFLOW (Main workflow entity)
-- =============================================================================
CREATE TABLE IF NOT EXISTS workflow_entity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    name VARCHAR(128) NOT NULL UNIQUE,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT false,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    nodes JSONB NOT NULL DEFAULT '[]',
    connections JSONB NOT NULL DEFAULT '{}',
    settings JSONB,
    static_data JSONB,
    meta JSONB,
    pin_data JSONB,
    version_id VARCHAR(36) NOT NULL DEFAULT substring(md5(random()::text), 1, 36),
    active_version_id VARCHAR(36) REFERENCES workflow_history(version_id) ON DELETE SET NULL,
    version_counter INTEGER NOT NULL DEFAULT 1,
    trigger_count INTEGER NOT NULL DEFAULT 0,
    parent_folder_id VARCHAR(36) REFERENCES folder(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workflow_name ON workflow_entity(name);
CREATE INDEX idx_workflow_active ON workflow_entity(active);
CREATE INDEX idx_workflow_folder ON workflow_entity(parent_folder_id);

-- Add FK for workflow_history after workflow_entity exists
ALTER TABLE workflow_history
    ADD CONSTRAINT fk_workflow_history_workflow
    FOREIGN KEY (workflow_id) REFERENCES workflow_entity(id) ON DELETE CASCADE;

-- =============================================================================
-- TAG
-- =============================================================================
CREATE TABLE IF NOT EXISTS tag_entity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    name VARCHAR(24) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- WORKFLOW_TAG_MAPPING (Junction table)
-- =============================================================================
CREATE TABLE IF NOT EXISTS workflow_tag_mapping (
    workflow_id VARCHAR(36) NOT NULL REFERENCES workflow_entity(id) ON DELETE CASCADE,
    tag_id VARCHAR(36) NOT NULL REFERENCES tag_entity(id) ON DELETE CASCADE,
    PRIMARY KEY (workflow_id, tag_id)
);

CREATE INDEX idx_workflow_tag_workflow ON workflow_tag_mapping(workflow_id);
CREATE INDEX idx_workflow_tag_tag ON workflow_tag_mapping(tag_id);

-- =============================================================================
-- SHARED_WORKFLOW (Workflow access control)
-- =============================================================================
CREATE TABLE IF NOT EXISTS shared_workflow (
    workflow_id VARCHAR(36) NOT NULL REFERENCES workflow_entity(id) ON DELETE CASCADE,
    project_id VARCHAR(36) NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    role VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workflow_id, project_id)
);

-- =============================================================================
-- CREDENTIALS
-- =============================================================================
CREATE TABLE IF NOT EXISTS credentials_entity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    name VARCHAR(128) NOT NULL,
    type VARCHAR(128) NOT NULL,
    data TEXT NOT NULL,
    is_managed BOOLEAN NOT NULL DEFAULT false,
    is_global BOOLEAN NOT NULL DEFAULT false,
    is_resolvable BOOLEAN NOT NULL DEFAULT false,
    resolvable_allow_fallback BOOLEAN NOT NULL DEFAULT false,
    resolver_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_credentials_type ON credentials_entity(type);
CREATE INDEX idx_credentials_name ON credentials_entity(name);

-- =============================================================================
-- SHARED_CREDENTIALS (Credential access control)
-- =============================================================================
CREATE TABLE IF NOT EXISTS shared_credentials (
    credentials_id VARCHAR(36) NOT NULL REFERENCES credentials_entity(id) ON DELETE CASCADE,
    project_id VARCHAR(36) NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    role VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (credentials_id, project_id)
);

-- =============================================================================
-- EXECUTION (Workflow executions)
-- =============================================================================
CREATE TABLE IF NOT EXISTS execution_entity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    finished BOOLEAN NOT NULL DEFAULT false,
    mode VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    workflow_id VARCHAR(36) REFERENCES workflow_entity(id) ON DELETE SET NULL,
    retry_of VARCHAR(36),
    retry_success_id VARCHAR(36),
    wait_till TIMESTAMPTZ,
    stored_at VARCHAR(2) NOT NULL DEFAULT 'db'
);

CREATE INDEX idx_execution_workflow ON execution_entity(workflow_id);
CREATE INDEX idx_execution_workflow_id ON execution_entity(workflow_id, id);
CREATE INDEX idx_execution_wait_till ON execution_entity(wait_till, id);
CREATE INDEX idx_execution_finished ON execution_entity(finished, id);
CREATE INDEX idx_execution_workflow_finished ON execution_entity(workflow_id, finished, id);
CREATE INDEX idx_execution_workflow_wait_till ON execution_entity(workflow_id, wait_till, id);
CREATE INDEX idx_execution_stopped_at ON execution_entity(stopped_at);
CREATE INDEX idx_execution_status ON execution_entity(status);

-- =============================================================================
-- EXECUTION_DATA (Separate table for large execution data)
-- =============================================================================
CREATE TABLE IF NOT EXISTS execution_data (
    execution_id VARCHAR(36) PRIMARY KEY REFERENCES execution_entity(id) ON DELETE CASCADE,
    data TEXT NOT NULL,
    workflow_data JSONB NOT NULL,
    workflow_version_id VARCHAR(36)
);

-- =============================================================================
-- EXECUTION_METADATA (Key-value metadata)
-- =============================================================================
CREATE TABLE IF NOT EXISTS execution_metadata (
    id SERIAL PRIMARY KEY,
    execution_id VARCHAR(36) NOT NULL REFERENCES execution_entity(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL
);

CREATE INDEX idx_execution_metadata_execution ON execution_metadata(execution_id);

-- =============================================================================
-- VARIABLES (Global and project-scoped variables)
-- =============================================================================
CREATE TABLE IF NOT EXISTS variables (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    key TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'string',
    value TEXT NOT NULL,
    project_id VARCHAR(36) REFERENCES project(id) ON DELETE CASCADE
);

CREATE INDEX idx_variables_project ON variables(project_id);
CREATE INDEX idx_variables_key ON variables(key);

-- =============================================================================
-- WEBHOOK_ENTITY (Webhook configuration)
-- =============================================================================
CREATE TABLE IF NOT EXISTS webhook_entity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    workflow_id VARCHAR(36) NOT NULL REFERENCES workflow_entity(id) ON DELETE CASCADE,
    node VARCHAR(255) NOT NULL,
    method VARCHAR(16) NOT NULL,
    path VARCHAR(255) NOT NULL,
    webhook_id VARCHAR(255),
    path_length INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhook_workflow ON webhook_entity(workflow_id);
CREATE INDEX idx_webhook_path ON webhook_entity(path);
CREATE UNIQUE INDEX idx_webhook_unique ON webhook_entity(webhook_id, method, path);

-- =============================================================================
-- API_KEY (User API keys)
-- =============================================================================
CREATE TABLE IF NOT EXISTS api_key (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    label VARCHAR(255) NOT NULL,
    api_key VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_key_user ON api_key(user_id);

-- =============================================================================
-- AUTH_IDENTITY (External authentication)
-- =============================================================================
CREATE TABLE IF NOT EXISTS auth_identity (
    id VARCHAR(36) PRIMARY KEY DEFAULT substring(md5(random()::text), 1, 21),
    user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    provider_type VARCHAR(64) NOT NULL,
    provider_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider_type, provider_id)
);

CREATE INDEX idx_auth_identity_user ON auth_identity(user_id);

-- =============================================================================
-- TRIGGERS for updated_at
-- =============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply to all tables with updated_at
CREATE TRIGGER update_user_updated_at BEFORE UPDATE ON "user"
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_project_updated_at BEFORE UPDATE ON project
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_folder_updated_at BEFORE UPDATE ON folder
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_workflow_updated_at BEFORE UPDATE ON workflow_entity
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_workflow_history_updated_at BEFORE UPDATE ON workflow_history
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_tag_updated_at BEFORE UPDATE ON tag_entity
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_credentials_updated_at BEFORE UPDATE ON credentials_entity
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_webhook_updated_at BEFORE UPDATE ON webhook_entity
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_api_key_updated_at BEFORE UPDATE ON api_key
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_auth_identity_updated_at BEFORE UPDATE ON auth_identity
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
