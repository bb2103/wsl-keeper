use std::sync::Arc;
use tauri::{AppHandle, Manager, RunEvent};

mod domain;
mod i18n;
mod ipc;
mod platform;
mod runtime;

use domain::config::{AppConfig, ConfigManager};
use domain::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_manager = match ConfigManager::new() {
        Ok(cm) => Arc::new(cm),
        Err(e) => {
            eprintln!("Failed to initialize config: {e}");
            std::process::exit(1);
        }
    };

    let initial_config: AppConfig = ConfigManager::load_sync(config_manager.path());
    crate::i18n::set_preference(&initial_config.locale);
    if let Err(e) = runtime::log::init_logging(&initial_config.log_level) {
        eprintln!("Failed to initialize logging: {e}");
    }
    runtime::log::start_log_cleanup();

    let app_state = AppState::new(config_manager);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = runtime::tray::show_window(app);
        }))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            ipc::config_get,
            ipc::config_save,
            ipc::config_import,
            ipc::config_export,
            ipc::status_get,
            ipc::guardian_pause,
            ipc::guardian_resume,
            ipc::guardian_check,
            ipc::guardian_reset_wsl,
            ipc::guardian_reset_disk,
            ipc::wsl_list,
            ipc::disk_list,
            ipc::app_paths,
            ipc::app_open_logs,
            ipc::app_read_file,
            ipc::app_write_file,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            runtime::tray::setup_tray(app.handle())?;
            setup_autostart(app.handle(), &initial_config);

            let state_arc = Arc::new(handle.state::<AppState>().inner().clone());
            runtime::wsl::start_wsl_guardian(handle.clone(), state_arc.clone());
            runtime::disk::start_disk_guardian(handle.clone(), state_arc.clone());
            prefetch_inventory(handle.clone(), state_arc);

            let background = std::env::args().any(|arg| arg == "--background");
            let hide = background || (!cfg!(debug_assertions) && initial_config.start_minimized);
            if !hide {
                let _ = runtime::tray::show_window(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::WindowEvent {
                label,
                event: window_event,
                ..
            } = event
            {
                if label == "main" {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = window_event {
                        api.prevent_close();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                }
            }
        });
}

fn prefetch_inventory(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let distros = tauri::async_runtime::spawn_blocking(platform::wsl::list_distros)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        if !distros.is_empty() {
            let cfg = state.config.get().await;
            if let Some(resolved) = platform::wsl::resolve_distro(&cfg.distro, &distros) {
                if resolved != cfg.distro {
                    let _ = state.config.update(|c| c.distro = resolved).await;
                }
            }
        }
        let _ = tauri::async_runtime::spawn_blocking(platform::disk::list_physical_disks).await;
        let _ = domain::status::publish(&app, &state).await;
    });
}

fn setup_autostart(app: &AppHandle, cfg: &AppConfig) {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let _ = if cfg.autostart {
        manager.enable()
    } else {
        manager.disable()
    };
}
