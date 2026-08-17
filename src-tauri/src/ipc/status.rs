use crate::domain::state::AppState;
use crate::domain::status::{snapshot, KeeperStatus};
use tauri::State;

#[tauri::command]
pub async fn status_get(state: State<'_, AppState>) -> Result<KeeperStatus, String> {
    Ok(snapshot(state.inner()).await)
}
