use crate::domain::state::{AppState, DiskRuntime};
use crate::platform::{disk, mount};
use crate::runtime::notify;
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::sleep;

const CHECK_TICK: Duration = Duration::from_secs(15);
const INITIAL_RETRY: Duration = Duration::from_secs(30);
const MAX_RETRY: Duration = Duration::from_secs(600);
const SUCCESS_INTERVAL: Duration = Duration::from_secs(300);
const FIRST_UNMOUNTED_INTERVAL: Duration = Duration::from_secs(60);
const CIRCUIT_THRESHOLD: u32 = 10;
const CIRCUIT_RESET: Duration = Duration::from_secs(1800);

pub fn start_disk_guardian(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            run_once(&app, &state).await;
            let _ = crate::domain::status::publish(&app, &state).await;
            tokio::select! {
                _ = sleep(CHECK_TICK) => {}
                _ = state.wakers.disks.notified() => {}
            }
        }
    });
}

async fn run_once(app: &AppHandle, state: &AppState) {
    let cfg = state.config.get().await;
    if !cfg.guardian_enabled || cfg.pause_state.as_ref().is_some_and(|p| p.is_active()) {
        return;
    }
    if cfg.distro.trim().is_empty() {
        return;
    }

    let resets = state.resets.take_disks();
    {
        let mut rt = state.runtime.write().await;
        for id in resets {
            rt.disks.insert(id, DiskRuntime::default());
        }
        if let Some(opened) = rt.wsl_circuit_opened_at {
            let _ = opened;
        }
    }

    if !state.runtime.read().await.distro_running {
        return;
    }

    for rule in cfg.disk_rules.iter().filter(|r| r.enabled) {
        let mut current = {
            let rt = state.runtime.read().await;
            rt.disks.get(&rule.id).cloned().unwrap_or_default()
        };

        if current.circuit_open {
            if let Some(opened_at) = current.circuit_opened_at {
                if opened_at.elapsed() >= CIRCUIT_RESET {
                    tracing::info!("Auto-resetting circuit breaker for {}", rule.mount_name);
                    current = DiskRuntime::default();
                } else {
                    let mut rt = state.runtime.write().await;
                    rt.disks.insert(rule.id.clone(), current);
                    continue;
                }
            } else {
                continue;
            }
        }

        let rule_clone = rule.clone();
        let distro = cfg.distro.clone();
        let probe = tauri::async_runtime::spawn_blocking({
            let distro = distro.clone();
            let rule = rule_clone.clone();
            move || disk::probe_partition(&distro, &rule)
        })
        .await
        .unwrap_or_default();

        if probe.mounted {
            current.mounted = true;
            if let Some(device) = probe.device {
                current.device = Some(device);
            }
            current.failures = 0;
            current.circuit_open = false;
            current.circuit_opened_at = None;
            current.last_error = None;
            current.interval = SUCCESS_INTERVAL;
            current.next_retry = None;
            let mut rt = state.runtime.write().await;
            rt.disks.insert(rule.id.clone(), current);
            continue;
        }

        if current.failures > 0 {
            if let Some(next) = current.next_retry {
                if Utc::now() < next {
                    current.mounted = false;
                    let mut rt = state.runtime.write().await;
                    rt.disks.insert(rule.id.clone(), current);
                    continue;
                }
            }
        }

        if current.interval == Duration::ZERO {
            current.interval = FIRST_UNMOUNTED_INTERVAL;
        }

        match attempt_mount(&distro, &rule_clone).await {
            Ok(device) => {
                tracing::info!("Disk {} is attached and mounted", rule.mount_name);
                current.mounted = true;
                current.device = Some(device);
                current.failures = 0;
                current.circuit_open = false;
                current.circuit_opened_at = None;
                current.last_error = None;
                current.interval = SUCCESS_INTERVAL;
                current.next_retry = None;
            }
            Err(e) => {
                tracing::warn!("Failed to mount {}: {e}", rule.mount_name);
                current.mounted = false;
                current.failures += 1;
                current.last_error = Some(e.to_string());
                if current.failures >= CIRCUIT_THRESHOLD {
                    current.circuit_open = true;
                    current.circuit_opened_at = Some(Instant::now());
                    notify::notify_disk_mount_failed(app, &rule.mount_name);
                    tracing::error!("Circuit breaker opened for {}", rule.mount_name);
                } else {
                    let next_secs = if current.failures == 1 {
                        FIRST_UNMOUNTED_INTERVAL.as_secs()
                    } else {
                        let grown = current.interval.saturating_mul(2);
                        grown.min(MAX_RETRY).max(INITIAL_RETRY).as_secs()
                    };
                    current.interval = Duration::from_secs(next_secs);
                    current.next_retry = Some(Utc::now() + ChronoDuration::seconds(next_secs as i64));
                }
            }
        }

        let mut rt = state.runtime.write().await;
        rt.disks.insert(rule.id.clone(), current);
    }
}

async fn attempt_mount(distro: &str, rule: &crate::domain::config::DiskRule) -> anyhow::Result<String> {
    let mount_point = rule.mount_point();
    if let Ok(part) = lookup_partition(distro, rule).await {
        if part.mounted_at(&mount_point) {
            tracing::info!(
                "{} already mounted at {mount_point} via {}, skipping",
                rule.mount_name,
                part.device
            );
            return Ok(part.device);
        }
        mount_device(distro, rule, &part.device).await?;
        return Ok(part.device);
    }

    let drive = rule.physical_drive_path();
    tracing::info!("Attaching {} to WSL after restart or detach", drive);
    tauri::async_runtime::spawn_blocking({
        let drive = drive.clone();
        move || mount::trigger_mount_task(&drive)
    })
    .await??;

    let mut last_err = anyhow::anyhow!("Disk did not appear in WSL");
    for attempt in 0..8 {
        if attempt > 0 {
            sleep(Duration::from_secs(2)).await;
        }
        match lookup_partition(distro, rule).await {
            Ok(part) => {
                if !part.mounted_at(&mount_point) {
                    mount_device(distro, rule, &part.device).await?;
                }
                return Ok(part.device);
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

async fn lookup_partition(
    distro: &str,
    rule: &crate::domain::config::DiskRule,
) -> anyhow::Result<disk::PartitionPresence> {
    let distro = distro.to_string();
    let rule = rule.clone();
    tauri::async_runtime::spawn_blocking(move || disk::find_partition(&distro, &rule)).await?
}

async fn mount_device(
    distro: &str,
    rule: &crate::domain::config::DiskRule,
    device: &str,
) -> anyhow::Result<()> {
    let distro = distro.to_string();
    let rule = rule.clone();
    let device = device.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        disk::mount_partition_in_wsl(&distro, &rule, &device)
    })
    .await?
}
