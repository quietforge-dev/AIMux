---
name: aimux-github-actions
description: AIMux GitHub Actions 跨平台 Rust/Tauri 构建规范。配置、触发、查看或排查 Windows/macOS 构建和 GitHub Release 时使用。
---

# AIMux GitHub Actions

workflow 位于 `.github/workflows/build-release.yml`，构建 Windows x64/arm64 NSIS 安装包和 macOS arm64 App ZIP，不再构建 macOS x64。构建使用 Node.js、Rust stable、npm 和 Tauri CLI。

Windows/macOS 的每个架构都缓存 Cargo registry 和 Cargo git 依赖，但不缓存 `src-tauri/target`。每次发布都从当前源码重新生成 Rust 二进制和安装包，避免陈旧构建产物进入 Release；首次下载依赖后，后续构建仍会复用已缓存的依赖。

Windows x64 构建会在上传前将 NSIS 安装包静默安装到 runner 临时目录，启动应用并检查 `http://127.0.0.1:7789/health`。该检查失败时，Release 不会创建。

## 构建触发

- 推送 `v*` tag 自动构建，并在全部平台成功后创建同名 GitHub Release。
- `workflow_dispatch` 只构建并上传 Actions Artifact，不自动创建正式 Release。
- 通过浏览器操作 GitHub Actions 前，确认当前浏览器已登录；遇到登录页时停止并提醒用户登录。
- GitHub Release 正文来自 `docs/releases/v<版本号>.md`，每次发版必须根据上一个 `v*` tag 与当前代码差异创建对应文件；缺少文件时发布 job 必须失败。

## 产物

- `AIMux-Windows-x64.exe`
- `AIMux-Windows-arm64.exe`
- `AIMux-macOS-arm64.zip`

workflow 不读取或上传用户数据目录中的数据库、配置和密钥。当前没有 Apple 公证或 Windows 代码签名，构建成功不等于获得系统信任。

## 应用内更新

- Tauri updater 使用 GitHub Actions Secret `TAURI_SIGNING_PRIVATE_KEY` 签名更新包。私钥只能保存在安全位置，绝不提交到仓库或 Release。
- tag 发布会额外上传 Windows 安装程序的 `.exe.sig`、macOS 的 `.app.tar.gz` 与 `.sig` 以及 `latest.json`。客户端从 `releases/latest/download/latest.json` 读取清单，不调用 GitHub Releases REST API。
- 正式发布构建缺少 `TAURI_SIGNING_PRIVATE_KEY` 必须失败，避免生成不可验证的更新。
- 更换签名密钥会导致旧客户端无法验证新包；除非放弃已有自动更新链路，否则不可随意重建密钥。

## 排查顺序

1. 检查 workflow 使用的 runner 架构和 Tauri/Rust 工具链。
2. 检查 `npm ci`、`npm run build` 和 `npm run tauri build` 的原始日志。
3. 检查 `src-tauri/migrations`、图标和前端构建产物是否进入 bundle。
4. 检查上传路径和 Release tag 是否与矩阵架构一致。

不要替用户执行 `git push`、创建 tag 或上传外部发布物，除非用户明确要求。
