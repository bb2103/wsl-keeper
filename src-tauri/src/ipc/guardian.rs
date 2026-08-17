use crate::domain::actions;
use crate::domain::state::AppState;
use crate::domain::status;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn guardian_pause(
    app: AppHandle,
    state: State<'_, AppState>,
    minutes: Option<u32>,
) -> Result<(), String> {
    actions::pause(&app, state.inner(), minutes).await
}

#[tauri::command]
pub async fn guardian_resume(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    actions::resume(&app, state.inner()).await
}

#[tauri::command]
pub async fn guardian_check(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.wake_all();
    let _ = status::publish(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn guardian_reset_wsl(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state
        .resets
        .wsl
        .store(true, std::sync::atomic::Ordering::SeqCst);
    state.wake_all();
    let _ = status::publish(&app, state.inner()).await;
    Ok(())
}

#[tauri::command]
pub async fn guardian_reset_disk(
    app: AppHandle,
    state: State<'_, AppState>,
    rule_id: String,
) -> Result<(), String> {
    state.resets.disks.lock().unwrap().insert(rule_id);
    state.wake_all();
    let _ = status::publish(&app, state.inner()).await;
    Ok(())
}
