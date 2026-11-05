use tauri::State;

use crate::app_state::ServerState;

const GITHUB_REPOSITORY_URL: &str = "https://github.com/quietforge-dev/AIMux";

#[tauri::command]
pub fn open_data_directory() -> Result<(), String> {
    let path = crate::config::Settings::data_dir();
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    if url != GITHUB_REPOSITORY_URL && !url.starts_with("https://github.com/quietforge-dev/AIMux/")
    {
        return Err("只允许打开 AIMux 的 GitHub 地址".into());
    }
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn app_version(_state: State<'_, std::sync::Arc<ServerState>>) -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[tauri::command]
pub async fn gateway_url(state: State<'_, std::sync::Arc<ServerState>>) -> Result<String, String> {
    let app_state = state.inner().clone();
    let settings = &app_state.runtime_settings;
    let host = match settings.host.trim() {
        "" | "0.0.0.0" | "::" => "127.0.0.1",
        value => value,
    };
    Ok(format!("http://{host}:{}", settings.port))
}

#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) -> Result<(), String> {
    window.open_devtools();
    Ok(())
}

#[tauri::command]
pub fn minimize_to_tray(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn desktop_token(
    state: State<'_, std::sync::Arc<ServerState>>,
) -> Result<String, String> {
    let owner = crate::dao::user_dao::get(&state.pool, "local")
        .await
        .map_err(|e| e.to_string())?;
    let runtime = state.user_state(&owner).await.map_err(|e| e.to_string())?;
    runtime
        .desktop_secret
        .clone()
        .ok_or_else(|| "非桌面运行模式".to_owned())
}
