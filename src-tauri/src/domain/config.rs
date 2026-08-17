use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn app_dir() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "lea", "wsl-keeper")
        .ok_or_else(|| anyhow::anyhow!("Failed to determine app data directory"))?;
    let dir = dirs.config_dir().to_path_buf();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskRule {
    pub id: String,
    #[serde(alias = "disk_number")]
    pub disk_number: u32,
    #[serde(alias = "friendly_name")]
    pub friendly_name: String,
    pub partition: u32,
    #[serde(alias = "fs_type")]
    pub fs_type: String,
    #[serde(alias = "mount_name")]
    pub mount_name: String,
    pub enabled: bool,
}

impl DiskRule {
    pub fn physical_drive_path(&self) -> String {
        format!(r"\\.\PHYSICALDRIVE{}", self.disk_number)
    }

    pub fn mount_point(&self) -> String {
        format!("/mnt/wsl/{}", self.mount_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PauseState {
    pub until: DateTime<Utc>,
}

impl PauseState {
    pub fn is_active(&self) -> bool {
        Utc::now() < self.until
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default = "default_schema_version", alias = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default, alias = "start_minimized")]
    pub start_minimized: bool,
    #[serde(default)]
    pub distro: String,
    #[serde(default, alias = "guardian_enabled")]
    pub guardian_enabled: bool,
    #[serde(default, alias = "init_command")]
    pub init_command: Option<String>,
    #[serde(default, alias = "pause_state")]
    pub pause_state: Option<PauseState>,
    #[serde(default, alias = "disk_rules")]
    pub disk_rules: Vec<DiskRule>,
    #[serde(default = "default_log_level", alias = "log_level")]
    pub log_level: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_schema_version() -> u32 {
    1
}
fn default_log_level() -> String {
    "INFO".to_string()
}
fn default_locale() -> String {
    "system".to_string()
}
fn default_theme() -> String {
    "system".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            autostart: false,
            start_minimized: false,
            distro: String::new(),
            guardian_enabled: false,
            init_command: None,
            pause_state: None,
            disk_rules: Vec::new(),
            log_level: default_log_level(),
            locale: default_locale(),
            theme: default_theme(),
        }
    }
}

impl AppConfig {
    pub fn validate(&mut self) -> anyhow::Result<()> {
        self.schema_version = 1;
        self.distro = self.distro.trim().to_string();
        match self.locale.as_str() {
            "en" | "zh" | "system" => {}
            "zh-CN" | "zh-Hans" | "zh-TW" | "zh-Hant" => self.locale = "zh".into(),
            "en-US" | "en-GB" => self.locale = "en".into(),
            _ => self.locale = default_locale(),
        }
        match self.theme.as_str() {
            "light" | "dark" | "system" => {}
            _ => self.theme = default_theme(),
        }
        crate::i18n::set_preference(&self.locale);
        let mount_re = regex::Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,31}$").unwrap();
        let allowed_fs = ["ext4", "xfs", "btrfs"];
        let mut names = std::collections::HashSet::new();
        for rule in &mut self.disk_rules {
            if rule.id.trim().is_empty() {
                rule.id = uuid::Uuid::new_v4().to_string();
            }
            if rule.partition == 0 {
                anyhow::bail!("{}", crate::i18n::t("error.partitionMin"));
            }
            if !allowed_fs.contains(&rule.fs_type.as_str()) {
                anyhow::bail!(
                    "{}",
                    crate::i18n::tf("error.unsupportedFs", &[("fs", &rule.fs_type)])
                );
            }
            if !mount_re.is_match(&rule.mount_name) {
                anyhow::bail!(
                    "{}",
                    crate::i18n::tf("error.mountName", &[("name", &rule.mount_name)])
                );
            }
            if !names.insert(rule.mount_name.clone()) {
                anyhow::bail!(
                    "{}",
                    crate::i18n::tf("error.duplicateMount", &[("name", &rule.mount_name)])
                );
            }
        }
        match self.log_level.to_uppercase().as_str() {
            "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {
                self.log_level = self.log_level.to_uppercase();
            }
            _ => self.log_level = "INFO".into(),
        }
        Ok(())
    }
}

pub struct ConfigManager {
    path: PathBuf,
    config: Arc<RwLock<AppConfig>>,
}

impl ConfigManager {
    pub fn new() -> anyhow::Result<Self> {
        let dir = app_dir()?;
        let path = dir.join("config.json");
        let mut config = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        };
        if let Some(ref pause) = config.pause_state {
            if !pause.is_active() {
                config.pause_state = None;
            }
        }
        let _ = config.validate();
        Ok(Self {
            path,
            config: Arc::new(RwLock::new(config)),
        })
    }

    pub fn load_sync(path: &Path) -> AppConfig {
        path.parent()
            .and_then(|dir| fs::read_to_string(dir.join("config.json")).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub async fn get(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    pub async fn set(&self, mut config: AppConfig) -> anyhow::Result<()> {
        config.validate()?;
        self.persist(&config)?;
        *self.config.write().await = config;
        Ok(())
    }

    pub async fn update<F>(&self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut config = self.config.write().await;
        f(&mut config);
        self.persist(&config)?;
        Ok(())
    }

    pub async fn pause(&self, minutes: Option<u32>) -> anyhow::Result<()> {
        let until = match minutes {
            Some(mins) => Utc::now() + Duration::minutes(mins as i64),
            None => Utc::now() + Duration::days(365 * 100),
        };
        self.update(|c| c.pause_state = Some(PauseState { until }))
            .await
    }

    pub async fn resume(&self) -> anyhow::Result<()> {
        self.update(|c| {
            c.pause_state = None;
            c.guardian_enabled = true;
        })
        .await
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn persist(&self, config: &AppConfig) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(config)?)?;
        Ok(())
    }
}
