#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
pub mod background;
mod commands;
pub mod config;
mod controller;
pub mod dao;
pub mod database;
mod error;
pub mod gateway;
mod logging;
pub mod model;
pub mod schema;
pub mod service;
pub mod upstream;
pub mod utils;

use std::sync::Arc;

use app_state::AppState;
use config::Settings;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

#[cfg(not(debug_assertions))]
const STABLE_GATEWAY_PORT: u16 = 7789;
#[cfg(not(debug_assertions))]
const DEVELOPMENT_DESKTOP_PORT: u16 = 7790;

#[cfg(debug_assertions)]
fn load_runtime_settings() -> Settings {
    Settings::load().unwrap_or_default()
}

#[cfg(not(debug_assertions))]
fn load_runtime_settings() -> Settings {
    let mut settings = Settings::load().unwrap_or_default();
    if settings.port == DEVELOPMENT_DESKTOP_PORT {
        settings.port = STABLE_GATEWAY_PORT;
        if let Err(error) = settings.save() {
            tracing::warn!(%error, "无法保存恢复后的稳定网关端口");
        } else {
            tracing::info!(port = STABLE_GATEWAY_PORT, "正式版已恢复稳定网关端口");
        }
    }
    settings
}

pub fn run() {
    logging::init();
    let settings = load_runtime_settings();
    let runtime = tokio::runtime::Runtime::new().expect("创建 Tokio runtime 失败");
    let state = match runtime.block_on(AppState::initialize(settings)) {
        Ok(state) => state,
        Err(error) => {
            tracing::error!(%error, "初始化 AIMux 失败");
            return;
        }
    };
    let shared = Arc::new(state);
    let monitor_state = Arc::clone(&shared);
    runtime.spawn(async move {
        background::monitor_task::run(monitor_state).await;
    });
    let server_state = Arc::clone(&shared);
    runtime.spawn(async move {
        if let Err(error) = controller::serve(server_state).await {
            tracing::error!(%error, "AIMux HTTP 服务退出");
        }
    });
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            commands::open_data_directory,
            commands::open_external_url,
            commands::app_version,
            commands::gateway_url,
            commands::open_devtools,
            commands::minimize_to_tray,
            commands::exit_app
        ])
        .setup(|app| {
            let launched_from_autostart = std::env::args().any(|arg| arg == "--autostart");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("AIMux");
                if launched_from_autostart {
                    let _ = window.hide();
                    tracing::info!("AIMux 已通过开机自启动，当前窗口已隐藏到托盘");
                }
                let close_window = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Err(error) = close_window
                            .eval("window.dispatchEvent(new CustomEvent('aimux-close-requested'));")
                        {
                            tracing::error!(%error, "无法通知前端显示关闭确认框");
                        }
                    }
                });
            }

            let show = MenuItem::with_id(app, "show", "显示 AIMux", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| "未找到托盘图标".to_string())?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .tooltip("AIMux")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}

pub fn run_gateway() -> Result<(), String> {
    logging::init();
    let settings = Settings::load().map_err(|error| format!("读取设置失败: {error}"))?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let state = runtime
        .block_on(async { AppState::initialize(settings).await })
        .map_err(|error| format!("初始化 AIMux 失败: {error}"))?;
    let shared = Arc::new(state);
    let monitor_state = Arc::clone(&shared);
    runtime.spawn(async move {
        background::monitor_task::run(monitor_state).await;
    });
    runtime
        .block_on(controller::serve(shared))
        .map_err(|error| format!("AIMux HTTP 服务退出: {error}"))
}
