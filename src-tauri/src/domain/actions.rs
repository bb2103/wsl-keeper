use crate::domain::config::AppConfig;
use crate::domain::state::AppState;
use crate::domain::status;
use crate::platform::mount;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

pub async fn save_config(app: &AppHandle, state: &AppState, config: AppConfig) -> Result<(), String> {
    let prev = state.config.get().await;
    state
        .config
        .set(config.clone())
        .await
        .map_err(|e| e.to_string())?;

    if prev.autostart != config.autostart {
        let manager = app.autolaunch();
        let result = if config.autostart {
            manager.enable()
        } else {
            manager.disable()
        };
        if let Err(e) = result {
            tracing::warn!("Failed to update autostart: {e}");
        }
    }

    let any_enabled = config.disk_rules.iter().any(|r| r.enabled);
    if any_enabled {
        mount::ensure_mount_task_exists().map_err(|e| e.to_string())?;
    } else if let Err(e) = mount::delete_mount_task() {
        tracing::warn!("Failed to remove mount task: {e}");
    }

    state.wake_all();
    let _ = status::publish(app, state).await;
    Ok(())
}

pub async fn pause(app: &AppHandle, state: &AppState, minutes: Option<u32>) -> Result<(), String> {
    state.config.pause(minutes).await.map_err(|e| e.to_string())?;
    tracing::info!("Guardian paused ({minutes:?} minutes)");
    let _ = status::publish(app, state).await;
    Ok(())
}

pub async fn resume(app: &AppHandle, state: &AppState) -> Result<(), String> {
    state.config.resume().await.map_err(|e| e.to_string())?;
    state.wake_all();
    tracing::info!("Guardian resumed");
    let _ = status::publish(app, state).await;
    Ok(())
}

pub fn pause_from_tray(app: &AppHandle, minutes: Option<u32>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(e) = pause(&app, state.inner(), minutes).await {
            tracing::error!("Failed to pause guardian: {e}");
        }
    });
}

pub fn resume_from_tray(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(e) = resume(&app, state.inner()).await {
            tracing::error!("Failed to resume guardian: {e}");
        }
    });
}
