CREATE TABLE multiplier_monitor_configs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    token TEXT NOT NULL,
    account_ids TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_started_at TEXT,
    last_finished_at TEXT,
    lease_owner TEXT,
    lease_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_multiplier_monitor_configs_due
ON multiplier_monitor_configs(enabled, last_started_at, lease_until);

CREATE INDEX ix_multiplier_monitor_configs_updated_at
ON multiplier_monitor_configs(updated_at);

CREATE TABLE multiplier_monitor_logs (
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

CREATE INDEX idx_multiplier_monitor_logs_config_time
ON multiplier_monitor_logs(config_id, started_at DESC, id DESC);

CREATE INDEX idx_multiplier_monitor_logs_run_id
ON multiplier_monitor_logs(run_id);

CREATE INDEX idx_multiplier_monitor_logs_account_time
ON multiplier_monitor_logs(account_id, started_at DESC, id DESC);
