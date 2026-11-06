# Linux 发布流程

`scripts/deploy_linux.py` 采用单实例、双版本保留的发布方式。发布时不会覆盖当前正在运行的版本目录。仓库提交不含部署凭据的 [deploy_linux.example.py](../scripts/deploy_linux.example.py)；首次使用时复制为 `scripts/deploy_linux.py`，再填写服务器地址、域名和管理员初始凭据。真实配置文件已加入 `.gitignore`，不要强制加入版本控制。

也可以通过环境变量传入敏感信息，避免把密码和 API token 写入脚本：`AIMUX_ADMIN_USERNAME`、`AIMUX_ADMIN_PASSWORD`、`AIMUX_LOCAL_TOKEN`。服务器和域名分别可使用 `AIMUX_DEPLOY_HOST`、`AIMUX_DOMAIN` 配置；具体参数见 `python3 scripts/deploy_linux.example.py --help`。

## 服务器目录

```text
/opt/AIMux/
├── .env                 # 稳定路径，保存 systemd 环境变量
├── active-release       # 当前版本目录名
├── releases/
│   ├── <当前版本>/
│   │   ├── aimux-gateway
│   │   └── dist/
│   └── <上一版本>/
└── src-tauri/target/    # Cargo 编译缓存，不属于发布版本
```

每个版本名由 UTC 时间、当前 Git 短提交号和微秒组成。首次使用时，脚本会把旧的 `/opt/AIMux/src-tauri/target/release/aimux-gateway` 和 `/opt/AIMux/dist` 复制成 `legacy-*` 版本，然后再发布新版本。

服务器只保留当前版本和切换前的上一版本。新版本解压到独立目录后，脚本在该目录编译；编译失败只删除这个新目录，旧服务继续运行。编译成功后只把最终二进制复制到新版本目录，并删除发布目录中的源代码、配置和依赖清单，减少磁盘占用。Cargo 的 target 缓存留在稳定路径，避免每次发布重新下载和编译全部依赖。

## 切换顺序

1. 本机构建 `dist` 并上传压缩包。
2. 服务器创建新版本目录并解压，当前版本继续服务。
3. 在新版本目录编译网关，失败则清理新目录。
4. 写入新版本的 `AIMUX_WEB_ROOT` 和 systemd `ExecStart`。
5. systemd 重启网关；健康检查循环请求 `127.0.0.1:7789/health`，最多等待 20 秒。
6. 健康检查失败时自动恢复上一版本并再次检查；回滚也失败时停止并保留现场，不清理旧版本。
7. 新版本健康后写入 `active-release`，清理除当前版和上一版之外的目录。

Nginx 继续代理固定的 `127.0.0.1:7789`，因此域名和证书不随版本切换。Nginx 到网关的代理请求使用 `Connection: close`，避免空闲的上游 keep-alive 连接拖住 Axum 的优雅关闭。Nginx 只在配置需要时 reload，不参与每次版本切换。

## 优雅关闭

网关监听 SIGTERM。收到 systemd 重启信号后，Axum 停止接收新请求并等待已有请求完成，最多等待 30 秒；超时后进程退出，由 systemd 按配置处理。systemd 的 `TimeoutStopSec=40s` 给应用排空留出余量。

这能保护大多数普通请求和正在发送的流式响应，但单实例在释放 `7789` 到新进程监听之间仍可能有极短的新连接空窗。需要完全避免这个空窗时，应进一步使用 systemd socket activation 或蓝绿双网关；后者必须先把后台监控任务从网关进程中拆出，避免两个实例重复执行监控。

## 回滚

健康检查自动回滚只针对本次发布失败。手工回滚时，将 `active-release` 和 systemd 的 `ExecStart`、`AIMUX_WEB_ROOT` 指向上一版本，再执行：

```sh
systemctl daemon-reload
systemctl restart aimux.service
curl --fail http://127.0.0.1:7789/health
```

数据库迁移仍然是不可逆发布边界。包含 SQLx migration 的版本发布前必须备份 `aimux.db`，并确认旧版本不能继续访问新结构；代码或前端发布失败可以直接回滚版本目录。
