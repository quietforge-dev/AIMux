-- 为模型目录增加供应商元数据；历史模型按协议类型回填供应商。
CREATE TABLE models_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    provider TEXT NOT NULL,
    is_default INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT ck_models_type CHECK (type IN ('openai', 'anthropic')),
    CONSTRAINT uq_models_type_name UNIQUE (type, name)
);

INSERT INTO models_new (
    id, name, type, provider, is_default, created_at, updated_at
)
SELECT
    id,
    name,
    type,
    CASE
        WHEN type = 'anthropic' THEN 'anthropic'
        ELSE 'openai'
    END,
    is_default,
    created_at,
    updated_at
FROM models;

DROP TABLE models;
ALTER TABLE models_new RENAME TO models;

CREATE INDEX idx_models_type_name ON models(type, name);
CREATE INDEX ix_models_name ON models(name);
CREATE INDEX ix_models_type ON models(type);
CREATE INDEX ix_models_provider_type ON models(provider, type);
CREATE UNIQUE INDEX uq_models_one_default_per_type
ON models(type)
WHERE is_default = 1;
