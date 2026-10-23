-- 日期按汇总时的本机日历划分，持久化 UTC 边界，保留已有明细。
CREATE TABLE daily_usage_statistics (
    stat_date TEXT PRIMARY KEY NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_daily_usage_started ON daily_usage_statistics(started_at);

-- 延迟完成/写入的旧请求按差额更新已归档日期。
-- 明细删除不扣减汇总，确保用户手动清理明细后累计值仍保留。
CREATE TRIGGER daily_usage_after_insert AFTER INSERT ON usage_records
BEGIN
    UPDATE daily_usage_statistics
    SET input_tokens = input_tokens + COALESCE(NEW.input_tokens, 0),
        output_tokens = output_tokens + COALESCE(NEW.output_tokens, 0),
        cached_tokens = cached_tokens + COALESCE(NEW.cached_tokens, 0),
        total_tokens = total_tokens + COALESCE(NEW.total_tokens, 0),
        updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE NEW.started_at >= started_at AND NEW.started_at < ended_at;
END;

CREATE TRIGGER daily_usage_after_update
AFTER UPDATE OF input_tokens, output_tokens, cached_tokens, total_tokens ON usage_records
BEGIN
    UPDATE daily_usage_statistics
    SET input_tokens = input_tokens + COALESCE(NEW.input_tokens, 0) - COALESCE(OLD.input_tokens, 0),
        output_tokens = output_tokens + COALESCE(NEW.output_tokens, 0) - COALESCE(OLD.output_tokens, 0),
        cached_tokens = cached_tokens + COALESCE(NEW.cached_tokens, 0) - COALESCE(OLD.cached_tokens, 0),
        total_tokens = total_tokens + COALESCE(NEW.total_tokens, 0) - COALESCE(OLD.total_tokens, 0),
        updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE NEW.started_at >= started_at AND NEW.started_at < ended_at;
END;
