use crate::domain::config::app_dir;
use std::path::PathBuf;
use std::time::Duration;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

const RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub fn init_logging(log_level: &str) -> anyhow::Result<()> {
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let level = normalize_level(log_level);
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("wsl-keeper")
        .filename_suffix("log")
        .build(&log_dir)?;

    let filter = format!("wsl_keeper_lib={level},wsl_keeper={level}");
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true)
        .with_filter(EnvFilter::new(filter.clone()));

    let console_layer = fmt::layer()
        .with_target(false)
        .with_filter(EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    tracing::info!("Logging initialized at level {level}");
    Ok(())
}

fn normalize_level(level: &str) -> &'static str {
    match level.to_uppercase().as_str() {
        "TRACE" => "trace",
        "DEBUG" => "debug",
        "WARN" => "warn",
        "ERROR" => "error",
        _ => "info",
    }
}

pub fn log_dir() -> anyhow::Result<PathBuf> {
    Ok(app_dir()?.join("logs"))
}

pub fn start_log_cleanup() {
    tauri::async_runtime::spawn(async {
        loop {
            if let Err(e) = cleanup_old_logs() {
                tracing::warn!("Failed to cleanup old logs: {e}");
            }
            tokio::time::sleep(CLEANUP_INTERVAL).await;
        }
    });
}

fn cleanup_old_logs() -> anyhow::Result<()> {
    let log_dir = log_dir()?;
    if !log_dir.exists() {
        return Ok(());
    }
    let cutoff = std::time::SystemTime::now() - RETENTION;
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
    }
    Ok(())
}
