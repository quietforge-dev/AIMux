DROP TABLE IF EXISTS multiplier_monitor_configs;

CREATE TABLE multiplier_monitor_configs (
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

CREATE INDEX idx_multiplier_monitor_configs_due
ON multiplier_monitor_configs(enabled, last_started_at, lease_until);

CREATE INDEX ix_multiplier_monitor_configs_updated_at
ON multiplier_monitor_configs(updated_at);
