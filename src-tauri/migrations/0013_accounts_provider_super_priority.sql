-- 账号供应商是可选元数据；不对历史账号的模型列表做修改。
ALTER TABLE accounts ADD COLUMN provider TEXT;
ALTER TABLE accounts ADD COLUMN super_priority INTEGER NOT NULL DEFAULT 0;

-- 仅在支持模型全部能在同协议模型目录中找到，且供应商一致时回填。
-- 混合供应商、自定义模型、空列表和无效 JSON 均保留为未指定。
WITH resolved AS (
    SELECT a.id, MIN(m.provider) AS provider
    FROM accounts a
    JOIN json_each(CASE WHEN json_valid(a.supported_models)
                        THEN a.supported_models ELSE '[]' END) j
    LEFT JOIN models m ON m.type = a.type AND m.name = j.value
    WHERE json_type(CASE WHEN json_valid(a.supported_models)
                        THEN a.supported_models ELSE '[]' END) = 'array'
    GROUP BY a.id
    HAVING COUNT(*) > 0
       AND COUNT(m.id) = COUNT(*)
       AND COUNT(DISTINCT m.provider) = 1
       AND MIN(m.provider) <> ''
)
UPDATE accounts
SET provider = (SELECT provider FROM resolved WHERE resolved.id = accounts.id)
WHERE id IN (SELECT id FROM resolved);

CREATE INDEX idx_accounts_provider_type ON accounts(provider, type);
