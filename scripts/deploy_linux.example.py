#!/usr/bin/env python3
"""Build and deploy AIMux to a Linux host over SSH."""

from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
# Copy this file to deploy_linux.py, then fill in the deployment values below.
# Keep deploy_linux.py local: it may contain credentials and is gitignored.
DEFAULT_HOST = "YOUR_SERVER_IP_OR_HOST"
REMOTE_DIR = "/opt/AIMux"
RELEASES_DIR = f"{REMOTE_DIR}/releases"

# Set the initial web admin credentials and local API token here, or provide
# AIMUX_ADMIN_USERNAME, AIMUX_ADMIN_PASSWORD, and AIMUX_LOCAL_TOKEN in env.
# These values are written to the server's protected /opt/AIMux/.env file.
DEPLOY_USERNAME = ""
DEPLOY_PASSWORD = ""
LOCAL_TOKEN = ""

# Set the public hostname covered by your DNS and TLS certificate.
DOMAIN = "example.com"

# Optional SSH ProxyCommand, for example a locally configured SOCKS5 proxy.
# Leave empty to connect directly. AIMUX_SSH_PROXY_COMMAND can override this.
SSH_PROXY_COMMAND = ""


def systemd_env_value(value: str) -> str:
    if "\n" in value or "\r" in value:
        raise ValueError("环境变量值不能包含换行符")
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def run(command: list[str], *, cwd: Path | None = None) -> None:
    sensitive = any("base64 -d" in item for item in command)
    print("+", "写入远端配置（内容省略）" if sensitive else " ".join(command), flush=True)
    result = subprocess.run(command, cwd=cwd)
    if result.returncode:
        raise RuntimeError(f"部署步骤失败（退出码 {result.returncode}），请查看上方错误信息")


def release_id() -> str:
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    try:
        revision = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, text=True
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        revision = "local"
    return f"{timestamp}-{revision}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=os.getenv("AIMUX_DEPLOY_HOST", DEFAULT_HOST))
    parser.add_argument("--user", default=os.getenv("AIMUX_DEPLOY_USER", "root"))
    parser.add_argument("--port", type=int, default=int(os.getenv("AIMUX_DEPLOY_SSH_PORT", "22")))
    parser.add_argument("--username", default=os.getenv("AIMUX_ADMIN_USERNAME", DEPLOY_USERNAME))
    parser.add_argument("--password", default=os.getenv("AIMUX_ADMIN_PASSWORD", DEPLOY_PASSWORD))
    parser.add_argument("--domain", default=os.getenv("AIMUX_DOMAIN", DOMAIN))
    parser.add_argument("--proxy-command", default=os.getenv("AIMUX_SSH_PROXY_COMMAND", SSH_PROXY_COMMAND))
    parser.add_argument("--local-token", default=os.getenv("AIMUX_LOCAL_TOKEN", LOCAL_TOKEN))
    args = parser.parse_args()
    if args.host == "YOUR_SERVER_IP_OR_HOST":
        parser.error("请修改 DEFAULT_HOST 或设置 AIMUX_DEPLOY_HOST")
    if args.domain == "example.com":
        parser.error("请修改 DOMAIN 或设置 AIMUX_DOMAIN")
    if not args.username or not args.password or not args.local_token:
        parser.error(
            "请通过参数或环境变量提供网页登录账号密码，以及 AIMUX_LOCAL_TOKEN（供 Codex 等客户端使用）"
        )

    destination = f"{args.user}@{args.host}" if args.user else args.host
    version = release_id()
    release_dir = f"{RELEASES_DIR}/{version}"
    ssh_options = ["-p", str(args.port)] if args.port != 22 else []
    scp_options = ["-P", str(args.port)] if args.port != 22 else []
    if args.proxy_command:
        ssh_options += ["-o", f"ProxyCommand={args.proxy_command}"]
        scp_options += ["-o", f"ProxyCommand={args.proxy_command}"]
    remote = lambda command: run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ConnectionAttempts=1",
            *ssh_options,
            destination,
            command,
        ]
    )

    run(["npm", "run", "build"], cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="aimux-deploy-") as temp:
        archive = Path(temp) / "aimux"
        staging = Path(temp) / "payload"
        staging.mkdir()
        for name in ["dist", "src-tauri", "package.json", "package-lock.json", "config"]:
            source = ROOT / name
            target = staging / name
            if source.is_dir():
                shutil.copytree(source, target, ignore=shutil.ignore_patterns("target", "node_modules", "keys", ".DS_Store"))
            else:
                shutil.copy2(source, target)
        archive_path = Path(shutil.make_archive(str(archive), "gztar", staging))
        remote(
            "if ! test -x /root/.cargo/bin/cargo; then "
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/aimux-rustup.sh && "
            "sh /tmp/aimux-rustup.sh -y --profile minimal && rm /tmp/aimux-rustup.sh; "
            "fi"
        )
        remote(f"sudo -n mkdir -p {release_dir}")
        run(
            [
                "scp",
                "-o",
                "ConnectTimeout=10",
                *scp_options,
                str(archive_path),
                f"{destination}:/tmp/aimux-{version}.tar.gz",
            ]
        )
        remote(
            f"if ! sudo -n tar -xzf /tmp/aimux-{version}.tar.gz -C {release_dir}; then "
            f"sudo -n rm -f /tmp/aimux-{version}.tar.gz; "
            f"sudo -n rm -rf {release_dir}; exit 1; fi; "
            f"sudo -n rm -f /tmp/aimux-{version}.tar.gz"
        )

    remote(
        f"sudo -n mkdir -p {RELEASES_DIR} && "
        # Preserve the currently running binary and static files on first use.
        f"if ! sudo -n test -s {REMOTE_DIR}/active-release; then "
        f"legacy=legacy-$(date -u +%Y%m%dT%H%M%SZ); "
        f"sudo -n mkdir -p {RELEASES_DIR}/$legacy && "
        f"sudo -n install -m 755 {REMOTE_DIR}/src-tauri/target/release/aimux-gateway {RELEASES_DIR}/$legacy/aimux-gateway && "
        f"sudo -n cp -a {REMOTE_DIR}/dist {RELEASES_DIR}/$legacy/dist && "
        f"echo $legacy | sudo -n tee {REMOTE_DIR}/active-release >/dev/null; fi"
    )
    previous = subprocess.check_output(
        [
            "ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
            *ssh_options, destination,
            f"sudo -n cat {REMOTE_DIR}/active-release",
        ],
        text=True,
    ).strip()
    if not re.fullmatch(r"[A-Za-z0-9._-]+", previous):
        raise RuntimeError("服务器当前版本标识无效，已停止发布")

    # Keep Cargo's incremental cache outside releases. The old service keeps
    # running while this immutable release is compiled.
    try:
        remote(
            f"cd {release_dir}/src-tauri && "
            f"sudo -n env CARGO_TARGET_DIR={REMOTE_DIR}/src-tauri/target "
            "/root/.cargo/bin/cargo build --release --locked --no-default-features --bin aimux-gateway -j 1 && "
            f"sudo -n install -m 755 {REMOTE_DIR}/src-tauri/target/release/aimux-gateway "
            f"{release_dir}/aimux-gateway && "
            f"sudo -n rm -rf {release_dir}/src-tauri {release_dir}/config && "
            f"sudo -n rm -f {release_dir}/package.json {release_dir}/package-lock.json"
        )
    except RuntimeError:
        remote(f"sudo -n rm -rf {release_dir}")
        raise

    def activate(target_id: str) -> None:
        target_dir = f"{RELEASES_DIR}/{target_id}"
        target_env = "\n".join(
            [
                "TZ=Asia/Shanghai",
                "AIMUX_HOST=127.0.0.1",
                "AIMUX_PORT=7789",
                f"AIMUX_WEB_ROOT={target_dir}/dist",
                f"AIMUX_ADMIN_USERNAME={systemd_env_value(args.username)}",
                f"AIMUX_ADMIN_PASSWORD={systemd_env_value(args.password)}",
                f"AIMUX_LOCAL_TOKEN={systemd_env_value(args.local_token)}",
                "",
            ]
        )
        target_service = f"""[Unit]
Description=AIMux Gateway
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory={target_dir}
EnvironmentFile={REMOTE_DIR}/.env
ExecStart={target_dir}/aimux-gateway
Restart=on-failure
RestartSec=5
KillSignal=SIGTERM
TimeoutStopSec=40s
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
"""
        env_b64 = base64.b64encode(target_env.encode()).decode()
        service_b64 = base64.b64encode(target_service.encode()).decode()
        remote(
            f"echo {env_b64} | base64 -d | sudo -n tee {REMOTE_DIR}/.env >/dev/null && "
            f"sudo -n chmod 600 {REMOTE_DIR}/.env && "
            f"echo {service_b64} | base64 -d | sudo -n tee /etc/systemd/system/aimux.service >/dev/null && "
            "sudo -n systemctl daemon-reload"
        )

    def restart_and_check() -> None:
        remote(
            "sudo -n systemctl restart aimux.service && "
            "for i in $(seq 1 20); do "
            "if curl --fail --silent http://127.0.0.1:7789/health >/dev/null; then exit 0; fi; "
            "sleep 1; done; exit 1"
        )

    try:
        activate(version)
        restart_and_check()
    except RuntimeError as deploy_error:
        print("新版本健康检查失败，正在恢复上一版本。", file=sys.stderr, flush=True)
        try:
            activate(previous)
            restart_and_check()
        except RuntimeError as rollback_error:
            raise RuntimeError(
                f"新版本启动及自动回滚均失败；请检查服务器 aimux.service。"
                f"发布错误：{deploy_error}；回滚错误：{rollback_error}"
            ) from rollback_error
        remote(f"sudo -n rm -rf {release_dir}")
        raise RuntimeError(f"新版未通过健康检查，已恢复上一版本 {previous}") from deploy_error

    remote(
        f"echo {version} | sudo -n tee {REMOTE_DIR}/active-release >/dev/null && "
        f"sudo -n rm -rf {REMOTE_DIR}/dist && "
        # Keep only the active and immediately previous release.
        f"cd {RELEASES_DIR} && "
        f"for d in *; do [ -d \"$d\" ] || continue; "
        f"case \"$d\" in {version}|{previous}) ;; *) sudo -n rm -rf -- \"$d\" ;; esac; done"
    )
    nginx_config = f"""server {{
    listen 80;
    listen [::]:80;
    server_name {args.domain};

    location ^~ /.well-known/acme-challenge/ {{
        root /var/www/letsencrypt;
        default_type text/plain;
    }}

    location / {{
        proxy_pass http://127.0.0.1:7789;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection \"close\";
        proxy_read_timeout 300s;
    }}
}}
"""
    encoded_nginx = base64.b64encode(nginx_config.encode()).decode()
    remote(
        f"echo {encoded_nginx} | base64 -d | sudo -n tee /etc/nginx/conf.d/aimux.conf >/dev/null && "
        "sudo -n mkdir -p /var/www/letsencrypt && sudo -n nginx -t && sudo -n systemctl reload nginx && "
        f"sudo -n certbot certonly --webroot -w /var/www/letsencrypt -d {args.domain} --non-interactive --keep-until-expiring"
    )
    tls_config = nginx_config.replace(
        "    listen 80;\n    listen [::]:80;",
        "    listen 8443 ssl http2;\n    listen [::]:8443 ssl http2;\n    client_max_body_size 512m;\n"
        f"    ssl_certificate /etc/letsencrypt/live/{args.domain}/fullchain.pem;\n"
        f"    ssl_certificate_key /etc/letsencrypt/live/{args.domain}/privkey.pem;",
    ).replace("        proxy_read_timeout 300s;", "        proxy_buffering off;\n        proxy_request_buffering off;\n        proxy_read_timeout 3600s;")
    http_config = f"""server {{
    listen 80;
    listen [::]:80;
    server_name {args.domain};
    location ^~ /.well-known/acme-challenge/ {{ root /var/www/letsencrypt; }}
    location / {{ return 301 https://$host:8443$request_uri; }}
}}
"""
    encoded_nginx = base64.b64encode((http_config + tls_config).encode()).decode()
    remote(
        f"echo {encoded_nginx} | base64 -d | sudo -n tee /etc/nginx/conf.d/aimux.conf >/dev/null && "
        "sudo -n nginx -t && sudo -n systemctl reload nginx && "
        "sudo -n mkdir -p /var/www/letsencrypt && "
        "sudo -n systemctl enable aimux.service && "
        "sudo -n systemctl --no-pager --full status aimux.service && "
        "curl --fail --silent http://127.0.0.1:7789/health"
    )
    print(f"部署完成：https://{args.domain}:8443/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
