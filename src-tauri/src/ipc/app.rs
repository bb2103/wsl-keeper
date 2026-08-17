use crate::domain::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub config: String,
    pub logs: String,
}

#[tauri::command]
pub fn app_paths(state: State<'_, AppState>) -> Result<AppPaths, String> {
    Ok(AppPaths {
        config: state.config.path().to_string_lossy().to_string(),
        logs: crate::runtime::log::log_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub fn app_open_logs() -> Result<(), String> {
    let log_dir = crate::runtime::log::log_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("explorer.exe")
        .arg(log_dir)
        .spawn();
    Ok(())
}

#[tauri::command]
pub fn app_read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| e.to_string())
}
