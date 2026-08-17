use crate::domain::config::ConfigManager;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, RwLock};

#[derive(Debug, Clone, Default)]
pub struct DiskRuntime {
    pub mounted: bool,
    pub device: Option<String>,
    pub last_error: Option<String>,
    pub failures: u32,
    pub circuit_open: bool,
    pub circuit_opened_at: Option<std::time::Instant>,
    pub next_retry: Option<DateTime<Utc>>,
    pub interval: std::time::Duration,
}

#[derive(Debug, Default)]
pub struct RuntimeInner {
    pub distro_running: bool,
    pub distro_version: Option<u32>,
    pub wsl_available: bool,
    pub mount_supported: bool,
    pub running_since: Option<DateTime<Utc>>,
    pub last_check: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub wsl_failures: u32,
    pub wsl_circuit_open: bool,
    pub wsl_circuit_opened_at: Option<std::time::Instant>,
    pub mount_task_exists: bool,
    pub disks: HashMap<String, DiskRuntime>,
}

pub struct Wakers {
    pub wsl: Notify,
    pub disks: Notify,
}

pub struct ResetFlags {
    pub disks: Mutex<HashSet<String>>,
    pub wsl: AtomicBool,
}

impl ResetFlags {
    pub fn take_wsl(&self) -> bool {
        self.wsl.swap(false, Ordering::SeqCst)
    }

    pub fn take_disks(&self) -> HashSet<String> {
        let mut g = self.disks.lock().unwrap();
        std::mem::take(&mut *g)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ConfigManager>,
    pub runtime: Arc<RwLock<RuntimeInner>>,
    pub wakers: Arc<Wakers>,
    pub resets: Arc<ResetFlags>,
}

impl AppState {
    pub fn new(config: Arc<ConfigManager>) -> Self {
        Self {
            config,
            runtime: Arc::new(RwLock::new(RuntimeInner::default())),
            wakers: Arc::new(Wakers {
                wsl: Notify::new(),
                disks: Notify::new(),
            }),
            resets: Arc::new(ResetFlags {
                disks: Mutex::new(HashSet::new()),
                wsl: AtomicBool::new(false),
            }),
        }
    }

    pub fn wake_all(&self) {
        self.wakers.wsl.notify_waiters();
        self.wakers.disks.notify_waiters();
    }
}
