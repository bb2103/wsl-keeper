use crate::platform::{disk, wsl};
use tauri::async_runtime::spawn_blocking;

#[tauri::command]
pub async fn wsl_list() -> Result<Vec<wsl::DistroInfo>, String> {
    if let Some(cached) = wsl::cached_distros() {
        return Ok(cached);
    }
    spawn_blocking(wsl::list_distros)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disk_list() -> Result<Vec<disk::DiskInfo>, String> {
    if let Some(cached) = disk::cached_disks() {
        return Ok(cached);
    }
    spawn_blocking(disk::list_physical_disks)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
