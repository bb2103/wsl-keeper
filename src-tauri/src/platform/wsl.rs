use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static DISTRO_CACHE: Mutex<Option<Vec<DistroInfo>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistroInfo {
    pub name: String,
    pub state: String,
    pub version: u32,
    #[serde(alias = "is_default")]
    pub is_default: bool,
}

fn wsl_exe() -> PathBuf {
    crate::platform::system32_dir().join("wsl.exe")
}

fn wsl_command() -> Command {
    let path = wsl_exe();
    let mut cmd = if path.is_file() {
        Command::new(path)
    } else {
        Command::new("wsl.exe")
    };
    apply_no_window(&mut cmd);
    cmd
}

pub fn wsl_installed() -> bool {
    if wsl_exe().is_file() {
        return true;
    }
    let mut cmd = wsl_command();
    cmd.arg("-l")
        .arg("-q")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.status().is_ok()
}

fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

pub fn cached_distros() -> Option<Vec<DistroInfo>> {
    DISTRO_CACHE.lock().ok().and_then(|guard| guard.clone())
}

pub fn list_distros() -> anyhow::Result<Vec<DistroInfo>> {
    let mut cmd = wsl_command();
    cmd.args(["-l", "-v"]);
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {} -l -v", wsl_exe().display()))?;
    let stdout = decode_wsl_output(&output.stdout);
    if !output.status.success() && stdout.trim().is_empty() {
        let stderr = decode_wsl_output(&output.stderr);
        anyhow::bail!("wsl -l -v failed: {}", stderr.trim());
    }
    let list = parse_distro_list(&stdout);
    if let Ok(mut guard) = DISTRO_CACHE.lock() {
        *guard = Some(list.clone());
    }
    Ok(list)
}

pub fn resolve_distro(configured: &str, distros: &[DistroInfo]) -> Option<String> {
    if distros.is_empty() {
        return None;
    }

    let want = configured.trim();
    if !want.is_empty() {
        if let Some(found) = distros
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(want))
        {
            return Some(found.name.clone());
        }

        let want_l = want.to_ascii_lowercase();
        let fuzzy: Vec<&DistroInfo> = distros
            .iter()
            .filter(|d| {
                let name = d.name.to_ascii_lowercase();
                name.starts_with(&format!("{want_l}-")) || name.starts_with(&format!("{want_l}."))
            })
            .collect();
        if fuzzy.len() == 1 {
            return Some(fuzzy[0].name.clone());
        }
        if let Some(found) = fuzzy.iter().find(|d| d.is_default) {
            return Some(found.name.clone());
        }
    }

    distros
        .iter()
        .find(|d| d.is_default)
        .or_else(|| (distros.len() == 1).then_some(&distros[0]))
        .map(|d| d.name.clone())
}

pub fn spawn_keep_alive(name: &str) -> anyhow::Result<tokio::process::Child> {
    tracing::info!("Starting WSL keep-alive: {name}");
    let path = wsl_exe();
    let mut cmd = if path.is_file() {
        tokio::process::Command::new(path)
    } else {
        tokio::process::Command::new("wsl.exe")
    };
    cmd.args([
        "-d",
        name,
        "--exec",
        "/bin/sh",
        "-c",
        "while :; do sleep 86400; done",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
        .with_context(|| format!("failed to spawn keep-alive for {name}"))
}

pub fn exec_in_distro(name: &str, command: &str) -> anyhow::Result<String> {
    exec_in_distro_inner(name, command, false, true)
}

#[allow(dead_code)]
pub fn exec_in_distro_as_root(name: &str, command: &str) -> anyhow::Result<String> {
    exec_in_distro_inner(name, command, true, true)
}

/// Non-login `/bin/sh -c`. Avoids bashrc/profile side effects when probing mounts.
pub fn exec_in_distro_sh(name: &str, command: &str, as_root: bool) -> anyhow::Result<String> {
    exec_in_distro_inner(name, command, as_root, false)
}

fn exec_in_distro_inner(
    name: &str,
    command: &str,
    as_root: bool,
    login_bash: bool,
) -> anyhow::Result<String> {
    let mut cmd = wsl_command();
    cmd.arg("-d").arg(name);
    if as_root {
        cmd.args(["-u", "root"]);
    }
    if login_bash {
        cmd.args(["--", "bash", "-lc", command]);
    } else {
        cmd.args(["--exec", "/bin/sh", "-c", command]);
    }
    apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .with_context(|| format!("failed to exec in {name}"))?;
    let stdout = decode_wsl_output(&output.stdout);
    let stderr = decode_wsl_output(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "WSL exec failed ({}): {}",
            output.status,
            stderr.trim().if_empty(stdout.trim())
        );
    }
    Ok(stdout)
}

trait IfEmpty {
    fn if_empty(self, other: Self) -> Self;
}

impl IfEmpty for &str {
    fn if_empty(self, other: Self) -> Self {
        if self.is_empty() {
            other
        } else {
            self
        }
    }
}

pub fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return utf16_le(&bytes[2..]);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return utf16_be(&bytes[2..]);
    }
    if looks_like_utf16_le(bytes) {
        return utf16_le(bytes);
    }
    String::from_utf8_lossy(bytes).to_string()
}

fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    if bytes.len() < 8 || bytes.len() % 2 != 0 {
        return false;
    }
    let zeros = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
    zeros * 2 >= bytes.len() / 2
}

fn utf16_le(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn utf16_be(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn parse_distro_list(output: &str) -> Vec<DistroInfo> {
    output
        .lines()
        .filter_map(|line| {
            let raw = line.trim();
            if raw.is_empty() {
                return None;
            }
            let lower = raw.to_ascii_lowercase();
            if lower.contains("windows subsystem") || lower.starts_with("name") {
                return None;
            }
            let is_default = raw.starts_with('*');
            let line = raw.trim_start_matches('*').trim();
            let mut parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return None;
            }
            let version = parts.pop()?.parse().unwrap_or(2);
            let state = parts.pop()?.to_string();
            let name = parts.join(" ");
            if name.is_empty() {
                return None;
            }
            Some(DistroInfo {
                name,
                state,
                version,
                is_default,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_distro_list() {
        let sample = "  NAME            STATE           VERSION\n* Ubuntu-24.04    Running         2\n  Debian          Stopped         2\n";
        let list = parse_distro_list(sample);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Ubuntu-24.04");
        assert!(list[0].is_default);
        assert_eq!(list[1].name, "Debian");
    }

    #[test]
    fn resolves_ubuntu_prefix_to_unique_versioned_distro() {
        let distros = vec![DistroInfo {
            name: "Ubuntu-22.04".into(),
            state: "Stopped".into(),
            version: 2,
            is_default: true,
        }];
        assert_eq!(
            resolve_distro("Ubuntu", &distros).as_deref(),
            Some("Ubuntu-22.04")
        );
        assert_eq!(
            resolve_distro("", &distros).as_deref(),
            Some("Ubuntu-22.04")
        );
    }

    #[test]
    fn prefers_exact_ubuntu_when_several_exist() {
        let distros = vec![
            DistroInfo {
                name: "Ubuntu".into(),
                state: "Stopped".into(),
                version: 2,
                is_default: false,
            },
            DistroInfo {
                name: "Ubuntu-22.04".into(),
                state: "Stopped".into(),
                version: 2,
                is_default: true,
            },
        ];
        assert_eq!(resolve_distro("Ubuntu", &distros).as_deref(), Some("Ubuntu"));
        assert_eq!(
            resolve_distro("Ubuntu-22.04", &distros).as_deref(),
            Some("Ubuntu-22.04")
        );
    }
}
