use crate::i18n;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("Failed to show notification: {e}");
    }
}

pub fn notify_wsl_stopped(app: &AppHandle, distro: &str) {
    notify(
        app,
        "WSL Keeper",
        &i18n::tf("notify.wslStopped", &[("distro", distro)]),
    );
}

pub fn notify_wsl_circuit(app: &AppHandle, distro: &str) {
    notify(
        app,
        "WSL Keeper",
        &i18n::tf("notify.wslCircuit", &[("distro", distro)]),
    );
}

pub fn notify_disk_mount_failed(app: &AppHandle, disk_name: &str) {
    notify(
        app,
        "WSL Keeper",
        &i18n::tf("notify.diskMountFailed", &[("disk", disk_name)]),
    );
}

pub fn notify_pause_expired(app: &AppHandle) {
    notify(app, "WSL Keeper", i18n::t("notify.pauseExpired"));
}

pub fn notify_wsl_recovered(app: &AppHandle, distro: &str) {
    notify(
        app,
        "WSL Keeper",
        &i18n::tf("notify.wslRecovered", &[("distro", distro)]),
    );
}
