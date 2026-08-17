use crate::domain::config::AppConfig;
use crate::domain::state::{AppState, RuntimeInner};
use crate::i18n;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub const STATUS_EVENT: &str = "status";
pub const NAVIGATE_EVENT: &str = "navigate";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OverallKind {
    Ok,
    Starting,
    Paused,
    Circuit,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskStatus {
    pub rule_id: String,
    pub name: String,
    pub disk_number: u32,
    pub partition: u32,
    pub fs_type: String,
    pub mount_name: String,
    pub enabled: bool,
    pub mounted: bool,
    pub device: Option<String>,
    pub last_error: Option<String>,
    pub failures: u32,
    pub circuit_open: bool,
    pub next_retry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeeperStatus {
    pub overall: OverallKind,
    pub distro: String,
    pub distro_running: bool,
    pub distro_version: Option<u32>,
    pub wsl_available: bool,
    pub mount_supported: bool,
    pub paused: bool,
    pub pause_until: Option<DateTime<Utc>>,
    pub running_since: Option<DateTime<Utc>>,
    pub last_check: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub wsl_failures: u32,
    pub wsl_circuit_open: bool,
    pub mount_task_exists: bool,
    pub disks: Vec<DiskStatus>,
}

impl KeeperStatus {
    pub fn tooltip(&self) -> String {
        match self.overall {
            OverallKind::Ok => {
                let count = self.disks.iter().filter(|d| d.mounted).count().to_string();
                i18n::tf("tray.tooltip.ok", &[("count", &count)])
            }
            OverallKind::Starting => i18n::t("tray.tooltip.starting").into(),
            OverallKind::Paused => match self.pause_until {
                Some(until) => {
                    let time = until
                        .with_timezone(&chrono::Local)
                        .format("%H:%M")
                        .to_string();
                    i18n::tf("tray.tooltip.pausedUntil", &[("time", &time)])
                }
                None => i18n::t("tray.tooltip.paused").into(),
            },
            OverallKind::Circuit => i18n::t("tray.tooltip.circuit").into(),
            OverallKind::Stopped => i18n::t("tray.tooltip.stopped").into(),
            OverallKind::Error => i18n::t("tray.tooltip.error").into(),
        }
    }

    pub fn tray_label(&self) -> String {
        let state = match self.overall {
            OverallKind::Ok => i18n::t("overall.ok"),
            OverallKind::Starting => i18n::t("overall.starting"),
            OverallKind::Paused => i18n::t("overall.paused"),
            OverallKind::Circuit => i18n::t("overall.circuit"),
            OverallKind::Stopped => i18n::t("overall.stopped"),
            OverallKind::Error => i18n::t("overall.error"),
        };
        i18n::tf("tray.status", &[("state", state)])
    }

    pub fn recompute_overall(&mut self) {
        if self.paused {
            self.overall = OverallKind::Paused;
            return;
        }
        if !self.wsl_available {
            self.overall = OverallKind::Error;
            return;
        }
        if self.wsl_circuit_open || self.disks.iter().any(|d| d.enabled && d.circuit_open) {
            self.overall = OverallKind::Circuit;
            return;
        }
        if self.distro_running {
            if self.disks.iter().any(|d| d.enabled && !d.mounted) {
                self.overall = OverallKind::Starting;
            } else {
                self.overall = OverallKind::Ok;
            }
        } else if self.wsl_failures > 0 {
            self.overall = OverallKind::Starting;
        } else {
            self.overall = OverallKind::Stopped;
        }
    }
}

pub async fn snapshot(state: &AppState) -> KeeperStatus {
    let cfg = state.config.get().await;
    let rt = state.runtime.read().await;
    build(&cfg, &rt)
}

pub fn build(cfg: &AppConfig, rt: &RuntimeInner) -> KeeperStatus {
    let paused = !cfg.guardian_enabled
        || cfg
            .pause_state
            .as_ref()
            .map(|p| p.is_active())
            .unwrap_or(false);
    let pause_until = cfg.pause_state.as_ref().and_then(|p| {
        if p.is_active() {
            Some(p.until)
        } else {
            None
        }
    });

    let disks = cfg
        .disk_rules
        .iter()
        .map(|rule| {
            let live = rt.disks.get(&rule.id);
            DiskStatus {
                rule_id: rule.id.clone(),
                name: rule.friendly_name.clone(),
                disk_number: rule.disk_number,
                partition: rule.partition,
                fs_type: rule.fs_type.clone(),
                mount_name: rule.mount_name.clone(),
                enabled: rule.enabled,
                mounted: live.map(|d| d.mounted).unwrap_or(false),
                device: live.and_then(|d| d.device.clone()),
                last_error: live.and_then(|d| d.last_error.clone()),
                failures: live.map(|d| d.failures).unwrap_or(0),
                circuit_open: live.map(|d| d.circuit_open).unwrap_or(false),
                next_retry: live.and_then(|d| d.next_retry),
            }
        })
        .collect();

    let mut status = KeeperStatus {
        overall: OverallKind::Stopped,
        distro: cfg.distro.clone(),
        distro_running: rt.distro_running,
        distro_version: rt.distro_version,
        wsl_available: rt.wsl_available,
        mount_supported: rt.mount_supported,
        paused,
        pause_until,
        running_since: rt.running_since,
        last_check: rt.last_check.or(Some(Utc::now())),
        last_error: rt.last_error.clone(),
        wsl_failures: rt.wsl_failures,
        wsl_circuit_open: rt.wsl_circuit_open,
        mount_task_exists: rt.mount_task_exists,
        disks,
    };
    status.recompute_overall();
    status
}

pub fn emit(app: &AppHandle, status: &KeeperStatus) {
    let _ = app.emit(STATUS_EVENT, status);
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(status.tooltip()));
        let _ = tray.set_icon(Some(crate::runtime::tray::icon_for(status.overall)));
    }
    crate::runtime::tray::set_status_label(status.tray_label());
}

pub async fn publish(app: &AppHandle, state: &AppState) -> KeeperStatus {
    let status = snapshot(state).await;
    emit(app, &status);
    status
}
