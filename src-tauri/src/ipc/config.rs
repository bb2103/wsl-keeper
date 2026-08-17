use crate::domain::actions;
use crate::domain::config::AppConfig;
use crate::domain::state::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn config_get(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.get().await)
}

#[tauri::command]
pub async fn config_save(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    actions::save_config(&app, state.inner(), config).await?;
    crate::runtime::tray::apply_labels(&app);
    Ok(())
}

#[tauri::command]
pub async fn config_import(
    app: AppHandle,
    state: State<'_, AppState>,
    content: String,
) -> Result<(), String> {
    let config: AppConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    actions::save_config(&app, state.inner(), config).await?;
    crate::runtime::tray::apply_labels(&app);
    Ok(())
}

#[tauri::command]
pub async fn config_export(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.get().await;
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}
