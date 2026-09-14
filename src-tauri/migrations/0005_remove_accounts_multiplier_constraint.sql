-- SQLite 不支持直接删除 CHECK 约束；重建 accounts 表以移除倍率数据库约束。
CREATE TABLE accounts_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key_encrypted TEXT NOT NULL,
    status TEXT NOT NULL,
    priority INTEGER NOT NULL,
    multiplier NUMERIC(4, 2) NOT NULL,
    supported_models TEXT,
    tags TEXT,
    notes TEXT,
    last_error_code TEXT,
    last_error_message TEXT,
    last_successful_test_model TEXT,
    last_used_at TEXT,
    total_requests INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    test_default_model TEXT,
    model_mappings TEXT,
    monitor_average_duration_ms INTEGER,
    CONSTRAINT ck_accounts_type CHECK (type IN ('openai', 'anthropic')),
    CONSTRAINT ck_accounts_status CHECK (status IN ('active', 'disabled')),
    CONSTRAINT ck_accounts_priority CHECK (priority BETWEEN 0 AND 9)
);

INSERT INTO accounts_new (
    id, name, type, base_url, api_key_encrypted, status, priority, multiplier,
    supported_models, tags, notes, last_error_code, last_error_message,
    last_successful_test_model, last_used_at, total_requests, total_tokens,
    created_at, updated_at, test_default_model, model_mappings,
    monitor_average_duration_ms
)
SELECT
    id, name, type, base_url, api_key_encrypted, status, priority, multiplier,
    supported_models, tags, notes, last_error_code, last_error_message,
    last_successful_test_model, last_used_at, total_requests, total_tokens,
    created_at, updated_at, test_default_model, model_mappings,
    monitor_average_duration_ms
FROM accounts;

DROP TABLE accounts;
ALTER TABLE accounts_new RENAME TO accounts;

CREATE INDEX idx_accounts_dispatch ON accounts(status, priority, id);
CREATE INDEX ix_accounts_priority ON accounts(priority);
CREATE INDEX ix_accounts_status ON accounts(status);
CREATE INDEX ix_accounts_type ON accounts(type);
