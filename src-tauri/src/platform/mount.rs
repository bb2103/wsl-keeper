use crate::domain::config::app_dir;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const TASK_NAME: &str = "WSLKeeperMountTask";

fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

pub fn mount_request_path() -> PathBuf {
    app_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("mount_request.txt")
}

pub fn helper_script_path() -> PathBuf {
    app_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("mount-helper.ps1")
}

pub fn create_task_script_path() -> PathBuf {
    app_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("create-mount-task.ps1")
}

pub fn ensure_scripts() -> anyhow::Result<()> {
    let dir = app_dir()?;
    fs::create_dir_all(&dir)?;

    let helper = r#"$ErrorActionPreference = 'Stop'
$dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$requestPath = Join-Path $dir 'mount_request.txt'
$logPath = Join-Path $dir 'mount_task.log'

function Write-Log([string]$Message) {
  "$(Get-Date -Format o) $Message" | Out-File -FilePath $logPath -Append -Encoding utf8
}

try {
  if (-not (Test-Path $requestPath)) { throw "Missing mount request file: $requestPath" }
  $drive = (Get-Content -Path $requestPath -Raw -Encoding utf8).Trim()
  if ([string]::IsNullOrWhiteSpace($drive)) { throw 'Empty drive path in request file' }
  Write-Log "Mounting $drive --bare"
  $wsl = Join-Path $env:SystemRoot 'System32\wsl.exe'
  $prevEa = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  $outputLines = & $wsl --mount $drive --bare 2>&1
  $code = $LASTEXITCODE
  $output = $outputLines | Out-String
  $ErrorActionPreference = $prevEa
  Write-Log "wsl --mount exit $code $output"
  if ($code -eq 0) { exit 0 }
  if ($output -match 'already (mounted|attached)|WSL_E_DISK_ALREADY_ATTACHED') {
    Write-Log "Disk already attached, treating as success"
    exit 0
  }
  exit $code
} catch {
  Write-Log "ERROR: $_"
  exit 1
}
"#;
    fs::write(helper_script_path(), helper)?;

    let helper_path = helper_script_path();
    let create = format!(
        r#"$ErrorActionPreference = 'Stop'
$taskName = '{task}'
$helper = '{helper}'
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $helper + '"')
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -DontStopOnIdleEnd -Hidden
Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings -Force | Out-Null
"#,
        task = TASK_NAME,
        helper = helper_path.display().to_string().replace('\'', "''"),
    );
    fs::write(create_task_script_path(), create)?;
    Ok(())
}

pub fn task_exists() -> anyhow::Result<bool> {
    let mut cmd = Command::new("schtasks.exe");
    cmd.args(["/Query", "/TN", TASK_NAME, "/FO", "LIST"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_no_window(&mut cmd);
    Ok(cmd.status()?.success())
}

pub fn ensure_mount_task_exists() -> anyhow::Result<()> {
    ensure_scripts()?;
    if task_exists()? {
        return Ok(());
    }
    create_mount_task()?;
    Ok(())
}

fn create_mount_task() -> anyhow::Result<()> {
    tracing::info!("Creating elevated scheduled task for WSL disk mounting");
    ensure_scripts()?;
    let script = create_task_script_path();
    let arg_list = format!(
        "-NoProfile -ExecutionPolicy Bypass -File {}",
        quote_cmd(&script.display().to_string())
    );

    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-Command",
        &format!(
            "Start-Process -FilePath powershell.exe -Verb RunAs -Wait -ArgumentList {}",
            ps_single_quote(&arg_list)
        ),
    ]);
    apply_no_window(&mut cmd);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Failed to create scheduled task. UAC may have been declined.");
    }

    for _ in 0..20 {
        if task_exists()? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    anyhow::bail!("Scheduled task was not created after UAC")
}

pub fn trigger_mount_task(drive_path: &str) -> anyhow::Result<()> {
    ensure_mount_task_exists()?;
    fs::write(mount_request_path(), drive_path)?;

    let mut cmd = Command::new("schtasks.exe");
    cmd.args(["/Run", "/TN", TASK_NAME]);
    apply_no_window(&mut cmd);
    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to trigger scheduled task: {}", stderr.trim());
    }
    Ok(())
}

pub fn delete_mount_task() -> anyhow::Result<()> {
    if !task_exists()? {
        return Ok(());
    }
    let mut cmd = Command::new("schtasks.exe");
    cmd.args(["/Delete", "/TN", TASK_NAME, "/F"]);
    apply_no_window(&mut cmd);
    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!(
            "Failed to delete scheduled task: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn quote_cmd(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn ps_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
