# AIMux 项目规范

本文件适用于整个 AIMux 仓库。项目正式实现为 Rust/Tauri 2/Vue 3。

## 分支约定

- `main` 用于桌面端主线；2026-11-05 分支拆分时保留在 `v0.3.25`，后续可继续演进。
- `main-web` 用于 Web 版，包含 2026-11-05 的全部开发提交；Web 功能继续在此分支开发。
- 只允许从 `main` 向 `main-web` 合并公共更新，不将 `main-web` 合并回 `main`。
- 提交或发布前确认目标分支，避免将 Web 专属改动带入桌面端主线。

进行代码修改前，先根据改动范围阅读对应 skill：

- 修改 `src/` 下的 Vue 页面、组件、Pinia store、API 客户端或样式时，阅读
  [`skills/aimux-frontend/SKILL.md`](skills/aimux-frontend/SKILL.md)。
- 修改 `src-tauri/src/controller/`、`service/`、`dao/`、`model/`、`schema/`、`gateway/` 或数据库业务时，阅读
  [`skills/aimux-backend/SKILL.md`](skills/aimux-backend/SKILL.md)。
- 修改 SQLx migration、SQLite 表结构、字段、约束、索引，或需要转换/回填已有持久化数据时，额外阅读
  [`skills/aimux-database-migrations/SKILL.md`](skills/aimux-database-migrations/SKILL.md)，并同步维护 `src-tauri/migrations/`。
  普通 DAO/Service 增删改查、查询、排序和分页不需要新增 migration。
- 执行 Windows 打包、安装包生成、版本升级、正式发版、产物验证或排查发布脚本时，阅读
  [`skills/aimux-windows-release/SKILL.md`](skills/aimux-windows-release/SKILL.md)。
- 配置、触发、查看或排查 GitHub Actions 的 Windows/macOS 构建，或通过浏览器操作 GitHub Actions 页面时，阅读
  [`skills/aimux-github-actions/SKILL.md`](skills/aimux-github-actions/SKILL.md)。
- 同时涉及多个范围时，对应 skill 都要阅读并遵守。

## 通用要求

- 保持 `controller -> service -> dao` 后端分层和 `api -> store -> page/component` 前端分层，避免无关重构。
- Rust 代码使用 `cargo fmt` 格式化，并补充明确的错误处理和类型定义；前端 TypeScript/Vue 代码使用项目 Prettier 配置。
- 数据库永远使用用户数据目录中的 `aimux.db`。修改表结构必须追加 SQLx migration，不修改已发布的基线 migration，也不通过代码启动时临时 `ALTER TABLE`。
- 修改后运行与影响范围匹配的聚焦检查；跨模块、共享基础设施、生命周期/全局配置改动或发布前再运行完整检查：

  ```powershell
  npm run format:check
  npm run build
  cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
  cargo check --manifest-path src-tauri/Cargo.toml
  cargo test --manifest-path src-tauri/Cargo.toml --lib
  ```

- 每次改动不默认执行全量检查。先测试受影响的访问路径；只有跨模块或发布前才扩展到完整检查。
- 改动完成后默认保留工作区改动，不自动执行 `git commit` 或 `git push`；只有用户明确要求时才创建中文说明的本地 commit 或推送远程。

## 命令执行

如果 PowerShell 命令没有输出，改用：

`C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.4.0_x64__8wekyb3d8bbwe\pwsh.exe`
