#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$project_root"

if ! command -v npm >/dev/null 2>&1; then
    echo "[AIMux] 未找到 npm，请先安装 Node.js LTS。"
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "[AIMux] 未找到 cargo，请先安装 Rust stable 工具链。"
    exit 1
fi

dev_backend_port="$(node scripts/runtime-ports.mjs development backend)"
dev_frontend_port="$(node scripts/runtime-ports.mjs development frontend)"

echo "[AIMux] 正在启动 Tauri 桌面端开发环境..."
echo "[AIMux] Vite 前端端口：${dev_frontend_port}"
echo "[AIMux] Rust 开发后端端口：${dev_backend_port}"
echo "[AIMux] 稳定 Rust 后端端口：不占用"

exec npm run dev:desktop
