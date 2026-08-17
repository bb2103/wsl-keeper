use crate::domain::config::DiskRule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static DISK_CACHE: Mutex<Option<Vec<DiskInfo>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionInfo {
    pub number: u32,
    pub size: u64,
    #[serde(alias = "partition_type")]
    pub partition_type: String,
    #[serde(alias = "drive_letter")]
    pub drive_letter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub number: u32,
    #[serde(alias = "friendly_name")]
    pub friendly_name: String,
    pub size: u64,
    #[serde(alias = "partition_style")]
    pub partition_style: String,
    #[serde(alias = "health_status")]
    pub health_status: String,
    #[serde(alias = "operational_status")]
    pub operational_status: String,
    pub serial: Option<String>,
    pub partitions: Vec<PartitionInfo>,
}

fn powershell_exe() -> PathBuf {
    let bundled = crate::platform::system32_dir()
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if bundled.is_file() {
        bundled
    } else {
        crate::platform::system32_dir().join("powershell.exe")
    }
}

fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

fn powershell(script: &str) -> anyhow::Result<String> {
    let wrapped = format!(
        "$ProgressPreference='SilentlyContinue'; [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false; {script}"
    );
    let exe = powershell_exe();
    let mut cmd = Command::new(exe);
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &wrapped,
    ])
    .stdin(Stdio::null());
    apply_no_window(&mut cmd);
    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!(
            "PowerShell failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn cached_disks() -> Option<Vec<DiskInfo>> {
    DISK_CACHE.lock().ok().and_then(|guard| guard.clone())
}

pub fn list_physical_disks() -> anyhow::Result<Vec<DiskInfo>> {
    let script = r#"
Get-Disk | ForEach-Object {
  $d = $_
  $parts = @(Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue | ForEach-Object {
    [pscustomobject]@{
      Number = $_.PartitionNumber
      Size = [int64]$_.Size
      Type = [string]$_.Type
      DriveLetter = if ($_.DriveLetter) { [string]$_.DriveLetter } else { $null }
    }
  })
  [pscustomobject]@{
    Number = $d.Number
    FriendlyName = $d.FriendlyName
    Size = [int64]$d.Size
    PartitionStyle = [string]$d.PartitionStyle
    HealthStatus = [string]$d.HealthStatus
    OperationalStatus = [string]$d.OperationalStatus
    SerialNumber = [string]$d.SerialNumber
    Partitions = $parts
  }
} | ConvertTo-Json -Depth 6 -Compress
"#;

    let json_str = powershell(script)?.trim().to_string();
    if json_str.is_empty() || json_str == "null" {
        return Ok(store_disk_cache(Vec::new()));
    }

    let value: Value = serde_json::from_str(&json_str)?;
    let mut result = Vec::new();
    if let Some(array) = value.as_array() {
        for disk in array {
            result.push(parse_disk_info(disk)?);
        }
    } else if value.is_object() {
        result.push(parse_disk_info(&value)?);
    }
    Ok(store_disk_cache(result))
}

fn store_disk_cache(disks: Vec<DiskInfo>) -> Vec<DiskInfo> {
    if let Ok(mut guard) = DISK_CACHE.lock() {
        *guard = Some(disks.clone());
    }
    disks
}

fn parse_disk_info(value: &Value) -> anyhow::Result<DiskInfo> {
    let mut partitions = Vec::new();
    if let Some(array) = value["Partitions"].as_array() {
        for p in array {
            partitions.push(PartitionInfo {
                number: json_u32(&p["Number"]),
                size: json_u64(&p["Size"]),
                partition_type: json_string(&p["Type"], "Unknown"),
                drive_letter: p["DriveLetter"].as_str().map(|s| s.to_string()),
            });
        }
    } else if value["Partitions"].is_object() {
        let p = &value["Partitions"];
        partitions.push(PartitionInfo {
            number: json_u32(&p["Number"]),
            size: json_u64(&p["Size"]),
            partition_type: json_string(&p["Type"], "Unknown"),
            drive_letter: p["DriveLetter"].as_str().map(|s| s.to_string()),
        });
    }

    let serial = value["SerialNumber"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(DiskInfo {
        number: json_u32(&value["Number"]),
        friendly_name: json_string(&value["FriendlyName"], "Unknown"),
        size: json_u64(&value["Size"]),
        partition_style: json_string(&value["PartitionStyle"], "Unknown"),
        health_status: json_string(&value["HealthStatus"], "Unknown"),
        operational_status: json_string(&value["OperationalStatus"], "Unknown"),
        serial,
        partitions,
    })
}

fn json_u32(v: &Value) -> u32 {
    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)).unwrap_or(0) as u32
}

fn json_u64(v: &Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
        .or_else(|| v.as_f64().map(|f| f.max(0.0) as u64))
        .unwrap_or(0)
}

fn json_string(v: &Value, fallback: &str) -> String {
    v.as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Debug, Clone, Default)]
pub struct PartitionProbe {
    pub mounted: bool,
    pub device: Option<String>,
}

pub struct PartitionPresence {
    pub device: String,
    pub mountpoints: Vec<String>,
}

impl PartitionPresence {
    pub fn mounted_at(&self, path: &str) -> bool {
        self.mountpoints.iter().any(|p| paths_equal(p, path))
    }
}

fn paths_equal(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

pub fn probe_partition(distro: &str, rule: &DiskRule) -> PartitionProbe {
    let mount_point = rule.mount_point();
    match probe_mountpoint(distro, &mount_point) {
        Ok(Some(device)) => PartitionProbe {
            mounted: true,
            device: Some(device),
        },
        _ => PartitionProbe::default(),
    }
}

fn probe_mountpoint(distro: &str, mount_point: &str) -> anyhow::Result<Option<String>> {
    let mp = mount_point.replace('\'', "'\\''");
    let cmd = r#"
mp='__MP__'
src=""
if command -v findmnt >/dev/null 2>&1; then
  src=$(findmnt -n -o SOURCE --mountpoint "$mp" 2>/dev/null || true)
fi
read_src() {
  if [ -n "$src" ] || [ ! -r "$1" ]; then
    return
  fi
  while read -r dev point _rest; do
    if [ "$point" = "$mp" ]; then
      src=$dev
      break
    fi
  done < "$1"
}
read_src /proc/self/mounts
read_src /proc/mounts
if [ -z "$src" ] && [ -d "$mp" ]; then
  d1=$(stat -c %d "$mp" 2>/dev/null || true)
  d0=$(stat -c %d "${mp%/*}" 2>/dev/null || true)
  if [ -n "$d1" ] && [ -n "$d0" ] && [ "$d1" != "$d0" ]; then
    src=$(df -P "$mp" 2>/dev/null | awk 'NR==2 { print $1 }')
    [ -n "$src" ] || src=unknown
  fi
fi
if [ -n "$src" ]; then
  printf 'MOUNTED %s\n' "$src"
else
  printf 'NOTMOUNTED\n'
fi
"#
    .replace("__MP__", &mp);

    let output = crate::platform::wsl::exec_in_distro_sh(distro, cmd.trim(), true)?;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("MOUNTED") {
            let device = rest.trim();
            if !device.is_empty() {
                return Ok(Some(device.to_string()));
            }
        }
    }
    Ok(None)
}

pub fn find_partition(distro: &str, rule: &DiskRule) -> anyhow::Result<PartitionPresence> {
    let output = crate::platform::wsl::exec_in_distro_sh(
        distro,
        "lsblk -J -b -p -o NAME,SIZE,TYPE,MOUNTPOINT,MODEL,SERIAL 2>/dev/null",
        true,
    )?;
    let json: Value = serde_json::from_str(output.trim())
        .map_err(|e| anyhow::anyhow!("Invalid lsblk output: {e}"))?;
    let devices = json["blockdevices"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid lsblk output"))?;

    let expected_size = get_disk_size(rule.disk_number).ok();
    let expected_serial = get_disk_serial(rule.disk_number);

    let mut matched: Option<&Value> = None;
    for device in devices {
        if device["type"].as_str() != Some("disk") {
            continue;
        }
        let serial = device["serial"].as_str().map(|s| s.trim().to_string());
        if let (Some(want), Some(have)) = (expected_serial.as_ref(), serial.as_ref()) {
            if !want.is_empty() && want.eq_ignore_ascii_case(have) {
                matched = Some(device);
                break;
            }
        }
        if let Some(size) = expected_size {
            let device_size = json_u64(&device["size"]);
            if sizes_match(size, device_size) {
                matched = Some(device);
            }
        }
    }

    let device = matched.ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find PHYSICALDRIVE{} inside WSL. Is the disk attached with --bare?",
            rule.disk_number
        )
    })?;

    let children = device["children"].as_array();
    if let Some(children) = children {
        for child in children {
            let name = child["name"].as_str().unwrap_or_default();
            if partition_index(name) == Some(rule.partition) {
                return Ok(PartitionPresence {
                    device: name.to_string(),
                    mountpoints: json_mountpoints(child),
                });
            }
        }
        if let Some(child) = children.get(rule.partition.saturating_sub(1) as usize) {
            if let Some(name) = child["name"].as_str() {
                return Ok(PartitionPresence {
                    device: name.to_string(),
                    mountpoints: json_mountpoints(child),
                });
            }
        }
    }

    anyhow::bail!(
        "Found disk PHYSICALDRIVE{} but partition {} is missing",
        rule.disk_number,
        rule.partition
    )
}

fn json_mountpoints(value: &Value) -> Vec<String> {
    let mut points = Vec::new();
    if let Some(s) = value["mountpoint"].as_str() {
        if !s.is_empty() {
            points.push(s.to_string());
        }
    }
    match &value["mountpoints"] {
        Value::Array(items) => {
            for item in items {
                if let Some(s) = item.as_str() {
                    if !s.is_empty() {
                        points.push(s.to_string());
                    }
                }
            }
        }
        Value::String(s) if !s.is_empty() => points.push(s.clone()),
        _ => {}
    }
    points.sort();
    points.dedup();
    points
}

fn partition_index(name: &str) -> Option<u32> {
    let digits: String = name.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.chars().rev().collect::<String>().parse().ok()
}

fn get_disk_size(disk_number: u32) -> anyhow::Result<u64> {
    if let Some(disks) = cached_disks() {
        if let Some(disk) = disks.iter().find(|d| d.number == disk_number) {
            if disk.size > 0 {
                return Ok(disk.size);
            }
        }
    }
    let script = format!("[int64](Get-Disk -Number {disk_number}).Size");
    let size_str = powershell(&script)?.trim().to_string();
    size_str
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse disk size: {e}"))
}

fn get_disk_serial(disk_number: u32) -> Option<String> {
    if let Some(disks) = cached_disks() {
        if let Some(disk) = disks.iter().find(|d| d.number == disk_number) {
            if let Some(serial) = disk.serial.as_ref().filter(|s| !s.is_empty()) {
                return Some(serial.clone());
            }
        }
    }
    let script = format!("(Get-Disk -Number {disk_number}).SerialNumber");
    powershell(&script)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn sizes_match(a: u64, b: u64) -> bool {
    if a == 0 || b == 0 {
        return false;
    }
    let diff = a.abs_diff(b);
    let max = a.max(b);
    (diff as f64 / max as f64) < 0.02
}

pub fn mount_partition_in_wsl(distro: &str, rule: &DiskRule, device: &str) -> anyhow::Result<()> {
    let mount_point = rule.mount_point();
    let fs_type = &rule.fs_type;
    let cmd = format!(
        "mkdir -p '{mp}' && mount -t {fs} '{dev}' '{mp}'",
        mp = mount_point.replace('\'', "'\\''"),
        fs = fs_type,
        dev = device.replace('\'', "'\\''")
    );
    match crate::platform::wsl::exec_in_distro_sh(distro, &cmd, true) {
        Ok(_) => {
            tracing::info!("Mounted {device} to {mount_point} in {distro}");
            Ok(())
        }
        Err(e) if already_mounted_error(&e.to_string(), &mount_point) => {
            tracing::info!("{device} already mounted at {mount_point}, skipping");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn already_mounted_error(err: &str, mount_point: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    if !lower.contains("already mounted") {
        return false;
    }
    err.contains(&format!("on {mount_point}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treats_same_path_already_mounted_as_success() {
        let err = "WSL exec failed (exit code: 32): mount: /mnt/wsl/disk2-1: /dev/sdd1 already mounted on /mnt/wsl/disk2-1.";
        assert!(already_mounted_error(err, "/mnt/wsl/disk2-1"));
    }

    #[test]
    fn ignores_already_mounted_on_a_different_path() {
        let err = "mount: /mnt/wsl/disk2-1: /dev/sdd1 already mounted on /mnt/other.";
        assert!(!already_mounted_error(err, "/mnt/wsl/disk2-1"));
    }

    #[test]
    fn parses_lsblk_mountpoint_and_mountpoints() {
        let json = serde_json::json!({
            "mountpoint": "/mnt/wsl/disk2-1",
            "mountpoints": ["/mnt/wsl/disk2-1", ""]
        });
        assert_eq!(json_mountpoints(&json), vec!["/mnt/wsl/disk2-1"]);
    }
}
