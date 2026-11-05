-- Existing installations become the initial owner's data. Credentials are initialized once in a transaction.
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin','user')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
    api_token_hash TEXT UNIQUE,
    api_token_prefix TEXT NOT NULL DEFAULT '',
    api_token_updated_at TEXT,
    initialized INTEGER NOT NULL DEFAULT 0 CHECK(initialized IN (0,1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(role <> 'admin' OR enabled = 1)
);
CREATE UNIQUE INDEX users_one_admin ON users(role) WHERE role='admin';
INSERT INTO users(id,username,password_hash,role,created_at,updated_at)
VALUES('local','admin','','admin',strftime('%Y-%m-%dT%H:%M:%SZ','now'),strftime('%Y-%m-%dT%H:%M:%SZ','now'));
CREATE TRIGGER users_keep_admin_delete BEFORE DELETE ON users WHEN OLD.role='admin'
BEGIN SELECT RAISE(ABORT,'cannot delete the only administrator'); END;
CREATE TRIGGER users_keep_admin_role BEFORE UPDATE OF role ON users WHEN OLD.role='admin' AND NEW.role<>'admin'
BEGIN SELECT RAISE(ABORT,'cannot demote the only administrator'); END;
CREATE TABLE web_sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX web_sessions_user ON web_sessions(user_id);
CREATE TABLE user_settings (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(user_id,key)
);
DROP TRIGGER daily_usage_after_insert;
DROP TRIGGER daily_usage_after_update;
CREATE TABLE accounts_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
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
    monitor_average_duration_ms INTEGER, disabled_until INTEGER, provider TEXT, super_priority INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT ck_accounts_type CHECK (type IN ('openai', 'anthropic')),
    CONSTRAINT ck_accounts_status CHECK (status IN ('active', 'disabled')),
    CONSTRAINT ck_accounts_priority CHECK (priority BETWEEN 0 AND 9)
);
INSERT INTO accounts_scoped(user_id,id,name,type,base_url,api_key_encrypted,status,priority,multiplier,supported_models,tags,notes,last_error_code,last_error_message,last_successful_test_model,last_used_at,total_requests,total_tokens,created_at,updated_at,test_default_model,model_mappings,monitor_average_duration_ms,disabled_until,provider,super_priority) SELECT 'local',id,name,type,base_url,api_key_encrypted,status,priority,multiplier,supported_models,tags,notes,last_error_code,last_error_message,last_successful_test_model,last_used_at,total_requests,total_tokens,created_at,updated_at,test_default_model,model_mappings,monitor_average_duration_ms,disabled_until,provider,super_priority FROM accounts;
DROP TABLE accounts;
ALTER TABLE accounts_scoped RENAME TO accounts;
CREATE INDEX idx_accounts_dispatch ON accounts(status, priority, id);
CREATE INDEX ix_accounts_priority ON accounts(priority);
CREATE INDEX ix_accounts_status ON accounts(status);
CREATE INDEX ix_accounts_type ON accounts(type);
CREATE INDEX idx_accounts_provider_type ON accounts(provider, type);

CREATE TABLE usage_records_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
    id TEXT PRIMARY KEY NOT NULL,
    trace_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    duration_ms INTEGER,
    first_token_ms INTEGER,
    account_id TEXT,
    account_name TEXT,
    account_type TEXT,
    model TEXT,
    reasoning_effort TEXT,
    endpoint TEXT,
    stream BOOLEAN NOT NULL,
    success BOOLEAN NOT NULL,
    status_code INTEGER,
    error_code TEXT,
    error_message TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    cached_tokens INTEGER,
    client_ip TEXT,
    attempts INTEGER NOT NULL
);
INSERT INTO usage_records_scoped(user_id,id,trace_id,started_at,ended_at,duration_ms,first_token_ms,account_id,account_name,account_type,model,reasoning_effort,endpoint,stream,success,status_code,error_code,error_message,input_tokens,output_tokens,total_tokens,cached_tokens,client_ip,attempts) SELECT 'local',id,trace_id,started_at,ended_at,duration_ms,first_token_ms,account_id,account_name,account_type,model,reasoning_effort,endpoint,stream,success,status_code,error_code,error_message,input_tokens,output_tokens,total_tokens,cached_tokens,client_ip,attempts FROM usage_records;
DROP TABLE usage_records;
ALTER TABLE usage_records_scoped RENAME TO usage_records;
CREATE INDEX idx_usage_started ON usage_records(started_at, id);
CREATE INDEX idx_usage_account ON usage_records(account_id, started_at);
CREATE INDEX idx_usage_model ON usage_records(model, started_at);
CREATE INDEX ix_usage_records_account_id ON usage_records(account_id);
CREATE INDEX ix_usage_records_model ON usage_records(model);
CREATE INDEX ix_usage_records_started_at ON usage_records(started_at);
CREATE INDEX ix_usage_records_trace_id ON usage_records(trace_id);

CREATE TABLE monitor_records_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    account_name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    model TEXT,
    checked_at TEXT NOT NULL,
    duration_ms INTEGER,
    success BOOLEAN NOT NULL,
    status_code INTEGER,
    error_code TEXT,
    error_message TEXT
);
INSERT INTO monitor_records_scoped(user_id,id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message) SELECT 'local',id,account_id,account_name,account_type,model,checked_at,duration_ms,success,status_code,error_code,error_message FROM monitor_records;
DROP TABLE monitor_records;
ALTER TABLE monitor_records_scoped RENAME TO monitor_records;
CREATE INDEX idx_monitor_account_checked ON monitor_records(account_id, checked_at);
CREATE INDEX idx_monitor_checked ON monitor_records(checked_at, id);
CREATE INDEX ix_monitor_records_account_id ON monitor_records(account_id);
CREATE INDEX ix_monitor_records_checked_at ON monitor_records(checked_at);

CREATE TABLE multiplier_monitor_configs_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    platform_type TEXT NOT NULL DEFAULT 'sub2api',
    new_api_group TEXT,
    sync_upstream_multiplier INTEGER NOT NULL DEFAULT 0,
    multiplier_divisor INTEGER NOT NULL DEFAULT 1,
    enabled INTEGER NOT NULL DEFAULT 0,
    last_started_at TEXT,
    last_finished_at TEXT,
    lease_owner TEXT,
    lease_until TEXT,
    last_success_at TEXT,
    last_error TEXT,
    last_remote_multiplier NUMERIC,
    last_result TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
INSERT INTO multiplier_monitor_configs_scoped(user_id,id,account_id,platform_type,new_api_group,sync_upstream_multiplier,multiplier_divisor,enabled,last_started_at,last_finished_at,lease_owner,lease_until,last_success_at,last_error,last_remote_multiplier,last_result,created_at,updated_at) SELECT 'local',id,account_id,platform_type,new_api_group,sync_upstream_multiplier,multiplier_divisor,enabled,last_started_at,last_finished_at,lease_owner,lease_until,last_success_at,last_error,last_remote_multiplier,last_result,created_at,updated_at FROM multiplier_monitor_configs;
DROP TABLE multiplier_monitor_configs;
ALTER TABLE multiplier_monitor_configs_scoped RENAME TO multiplier_monitor_configs;
CREATE INDEX idx_multiplier_monitor_configs_due
ON multiplier_monitor_configs(enabled, last_started_at, lease_until);
CREATE INDEX ix_multiplier_monitor_configs_updated_at
ON multiplier_monitor_configs(updated_at);

CREATE TABLE multiplier_monitor_logs_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL,
    config_id TEXT NOT NULL,
    config_name TEXT NOT NULL,
    account_id TEXT,
    account_name TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    duration_ms INTEGER,
    result TEXT NOT NULL,
    old_multiplier NUMERIC,
    remote_multiplier NUMERIC,
    error_code TEXT,
    error_message TEXT,
    http_status INTEGER
);
INSERT INTO multiplier_monitor_logs_scoped(user_id,id,run_id,config_id,config_name,account_id,account_name,started_at,finished_at,duration_ms,result,old_multiplier,remote_multiplier,error_code,error_message,http_status) SELECT 'local',id,run_id,config_id,config_name,account_id,account_name,started_at,finished_at,duration_ms,result,old_multiplier,remote_multiplier,error_code,error_message,http_status FROM multiplier_monitor_logs;
DROP TABLE multiplier_monitor_logs;
ALTER TABLE multiplier_monitor_logs_scoped RENAME TO multiplier_monitor_logs;
CREATE INDEX idx_multiplier_monitor_logs_config_time
ON multiplier_monitor_logs(config_id, started_at DESC, id DESC);
CREATE INDEX idx_multiplier_monitor_logs_run_id
ON multiplier_monitor_logs(run_id);
CREATE INDEX idx_multiplier_monitor_logs_account_time
ON multiplier_monitor_logs(account_id, started_at DESC, id DESC);


CREATE TABLE models_scoped (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('openai','anthropic')),
    provider TEXT NOT NULL,
    is_default INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(user_id,type,name)
);
INSERT INTO models_scoped SELECT id,'local',name,type,provider,is_default,created_at,updated_at FROM models;
DROP TABLE models;
ALTER TABLE models_scoped RENAME TO models;
CREATE UNIQUE INDEX uq_models_one_default_per_user_type ON models(user_id,type) WHERE is_default=1;
CREATE INDEX idx_models_user_provider ON models(user_id,provider,type);

CREATE TABLE daily_usage_statistics_scoped (
    user_id TEXT NOT NULL REFERENCES users(id),
    stat_date TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(user_id,stat_date)
);
INSERT INTO daily_usage_statistics_scoped SELECT 'local',stat_date,started_at,ended_at,input_tokens,output_tokens,cached_tokens,total_tokens,updated_at FROM daily_usage_statistics;
DROP TABLE daily_usage_statistics;
ALTER TABLE daily_usage_statistics_scoped RENAME TO daily_usage_statistics;
CREATE INDEX idx_daily_usage_user_started ON daily_usage_statistics(user_id,started_at);
CREATE TRIGGER daily_usage_after_insert AFTER INSERT ON usage_records BEGIN
    UPDATE daily_usage_statistics
    SET input_tokens=input_tokens+COALESCE(NEW.input_tokens,0),output_tokens=output_tokens+COALESCE(NEW.output_tokens,0),
        cached_tokens=cached_tokens+COALESCE(NEW.cached_tokens,0),total_tokens=total_tokens+COALESCE(NEW.total_tokens,0),
        updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
    WHERE user_id=NEW.user_id AND NEW.started_at>=started_at AND NEW.started_at<ended_at;
END;
CREATE TRIGGER daily_usage_after_update AFTER UPDATE OF input_tokens,output_tokens,cached_tokens,total_tokens ON usage_records BEGIN
    UPDATE daily_usage_statistics
    SET input_tokens=input_tokens+COALESCE(NEW.input_tokens,0)-COALESCE(OLD.input_tokens,0),
        output_tokens=output_tokens+COALESCE(NEW.output_tokens,0)-COALESCE(OLD.output_tokens,0),
        cached_tokens=cached_tokens+COALESCE(NEW.cached_tokens,0)-COALESCE(OLD.cached_tokens,0),
        total_tokens=total_tokens+COALESCE(NEW.total_tokens,0)-COALESCE(OLD.total_tokens,0),
        updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
    WHERE user_id=NEW.user_id AND NEW.started_at>=started_at AND NEW.started_at<ended_at;
END;
CREATE INDEX idx_accounts_user_routing ON accounts(user_id,type,status,priority DESC,multiplier);
CREATE INDEX idx_usage_user_time ON usage_records(user_id,started_at DESC,id DESC);
CREATE INDEX idx_usage_user_account_time ON usage_records(user_id,account_id,started_at DESC,id DESC);
CREATE INDEX idx_monitor_user_account_time ON monitor_records(user_id,account_id,checked_at DESC,id DESC);
CREATE INDEX idx_multiplier_user_due ON multiplier_monitor_configs(user_id,enabled,last_started_at,lease_until);
CREATE INDEX idx_multiplier_logs_user_time ON multiplier_monitor_logs(user_id,account_id,started_at DESC,id DESC);
CREATE TRIGGER accounts_owner_immutable BEFORE UPDATE OF user_id ON accounts WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER models_owner_immutable BEFORE UPDATE OF user_id ON models WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER usage_records_owner_immutable BEFORE UPDATE OF user_id ON usage_records WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER monitor_records_owner_immutable BEFORE UPDATE OF user_id ON monitor_records WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER multiplier_monitor_configs_owner_immutable BEFORE UPDATE OF user_id ON multiplier_monitor_configs WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER multiplier_monitor_logs_owner_immutable BEFORE UPDATE OF user_id ON multiplier_monitor_logs WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER daily_usage_statistics_owner_immutable BEFORE UPDATE OF user_id ON daily_usage_statistics WHEN OLD.user_id<>NEW.user_id
BEGIN SELECT RAISE(ABORT,'owner cannot change'); END;
CREATE TRIGGER usage_records_account_owner_insert BEFORE INSERT ON usage_records
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER usage_records_account_owner_update BEFORE UPDATE ON usage_records
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER monitor_records_account_owner_insert BEFORE INSERT ON monitor_records
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER monitor_records_account_owner_update BEFORE UPDATE ON monitor_records
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER multiplier_monitor_configs_account_owner_insert BEFORE INSERT ON multiplier_monitor_configs
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER multiplier_monitor_configs_account_owner_update BEFORE UPDATE ON multiplier_monitor_configs
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER multiplier_monitor_logs_account_owner_insert BEFORE INSERT ON multiplier_monitor_logs
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER multiplier_monitor_logs_account_owner_update BEFORE UPDATE ON multiplier_monitor_logs
WHEN NEW.account_id IS NOT NULL AND EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account owner mismatch'); END;
CREATE TRIGGER multiplier_config_account_exists BEFORE INSERT ON multiplier_monitor_configs
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id=NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account does not exist for user'); END;

CREATE TRIGGER multiplier_config_account_exists_update BEFORE UPDATE OF account_id ON multiplier_monitor_configs
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND user_id=NEW.user_id)
BEGIN SELECT RAISE(ABORT,'account does not exist for user'); END;
CREATE TRIGGER multiplier_log_config_owner BEFORE INSERT ON multiplier_monitor_logs
WHEN EXISTS(SELECT 1 FROM multiplier_monitor_configs WHERE id=NEW.config_id AND user_id<>NEW.user_id)
BEGIN SELECT RAISE(ABORT,'config owner mismatch'); END;
