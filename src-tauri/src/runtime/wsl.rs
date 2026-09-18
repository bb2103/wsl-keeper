use crate::domain::config::AppConfig;
use crate::domain::state::AppState;
use crate::platform::wsl;
use crate::runtime::notify;
use chrono::Utc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::{sleep, timeout};

const FAIL_INTERVAL: Duration = Duration::from_secs(30);
const STABLE_AFTER: Duration = Duration::from_secs(2);
const INIT_SETTLE: Duration = Duration::from_secs(3);
const CIRCUIT_THRESHOLD: u32 = 10;
const CIRCUIT_RESET: Duration = Duration::from_secs(1800);

struct KeepAlive {
    child: tokio::process::Child,
    distro: String,
    started: Instant,
}

impl KeepAlive {
    async fn stop(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

pub fn start_wsl_guardian(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut alive: Option<KeepAlive> = None;
        let mut notified_down = false;
        // Last init command executed against the live session; edits re-run it.
        let mut init_run: Option<String> = None;

        loop {
            let mut cfg = state.config.get().await;
            refresh_inventory(&state, &mut cfg).await;

            if !guardian_should_run(&cfg) {
                if let Some(session) = alive.take() {
                    session.stop().await;
                    mark_stopped(&state).await;
                    let _ = crate::domain::status::publish(&app, &state).await;
                }
                wait_while_idle(&app, &state).await;
                continue;
            }

            if let Some(session) = alive.take() {
                if session.distro != cfg.distro {
                    session.stop().await;
                    mark_stopped(&state).await;
                    init_run = None;
                } else {
                    match normalized_init(&cfg) {
                        // Run an edited command against the live session, but only
                        // once the settings UI's debounced saves settle, so a
                        // half-typed command never executes.
                        Some(cmd) if init_run.as_deref() != Some(cmd.as_str()) => {
                            wait_config_stable(&state, INIT_SETTLE).await;
                            let latest = state.config.get().await;
                            if guardian_should_run(&latest) && latest.distro == session.distro {
                                if let Some(cmd) = normalized_init(&latest)
                                    .filter(|c| init_run.as_deref() != Some(c.as_str()))
                                {
                                    let distro = session.distro.clone();
                                    run_init_command(&app, &state, &distro, &cmd).await;
                                    init_run = Some(cmd);
                                }
                            }
                        }
                        None => init_run = None,
                        _ => {}
                    }
                    watch_session(&app, &state, session, &mut notified_down, &mut alive).await;
                    continue;
                }
            }

            if !ensure_can_start(&app, &state, &cfg).await {
                wait_retry(&state, FAIL_INTERVAL).await;
                continue;
            }

            match start_session(&cfg.distro).await {
                Ok(session) => {
                    mark_running(&app, &state, &session.distro, &mut notified_down).await;
                    if let Some(cmd) = normalized_init(&cfg) {
                        let distro = session.distro.clone();
                        run_init_command(&app, &state, &distro, &cmd).await;
                        init_run = Some(cmd);
                    } else {
                        init_run = None;
                    }
                    state.wakers.disks.notify_waiters();
                    let _ = crate::domain::status::publish(&app, &state).await;
                    alive = Some(session);
                }
                Err(error) => {
                    record_start_failure(&app, &state, &cfg.distro, error.to_string()).await;
                    let _ = crate::domain::status::publish(&app, &state).await;
                    wait_retry(&state, FAIL_INTERVAL).await;
                }
            }
        }
    });
}

fn guardian_should_run(cfg: &AppConfig) -> bool {
    cfg.guardian_enabled
        && !cfg.distro.trim().is_empty()
        && !cfg
            .pause_state
            .as_ref()
            .is_some_and(|pause| pause.is_active())
}

async fn refresh_inventory(state: &AppState, cfg: &mut AppConfig) {
    let (wsl_ok, distros) = tauri::async_runtime::spawn_blocking(|| {
        let installed = wsl::wsl_installed();
        let distros = wsl::list_distros().unwrap_or_default();
        (installed, distros)
    })
    .await
    .unwrap_or((false, Vec::new()));

    if let Some(resolved) = wsl::resolve_distro(&cfg.distro, &distros) {
        if resolved != cfg.distro {
            tracing::info!("Resolved WSL distro '{}' → '{resolved}'", cfg.distro);
            let name = resolved.clone();
            let _ = state.config.update(|c| c.distro = name).await;
            cfg.distro = resolved;
        }
    }

    let distro_meta = distros
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(&cfg.distro));
    let version = distro_meta.map(|d| d.version);

    let mut rt = state.runtime.write().await;
    rt.wsl_available = wsl_ok;
    rt.distro_version = version;
    rt.mount_supported = version.unwrap_or(0) >= 2;
    rt.last_check = Some(Utc::now());
    rt.mount_task_exists = crate::platform::mount::task_exists().unwrap_or(false);
}

async fn watch_session(
    app: &AppHandle,
    state: &AppState,
    mut session: KeepAlive,
    notified_down: &mut bool,
    alive: &mut Option<KeepAlive>,
) {
    tokio::select! {
        waited = session.child.wait() => {
            let lived = session.started.elapsed();
            tracing::warn!(
                "WSL keep-alive for {} exited after {:?}: {waited:?}",
                session.distro,
                lived
            );
            mark_stopped(state).await;
            if !*notified_down {
                notify::notify_wsl_stopped(app, &session.distro);
                *notified_down = true;
            }
            let _ = crate::domain::status::publish(app, state).await;
            if lived < Duration::from_secs(60) {
                record_start_failure(app, state, &session.distro, crate::i18n::t("error.keepAliveExited").into()).await;
                wait_retry(state, FAIL_INTERVAL).await;
            }
        }
        _ = state.wakers.wsl.notified() => {
            *alive = Some(session);
        }
    }
}

async fn start_session(distro: &str) -> anyhow::Result<KeepAlive> {
    let mut child = wsl::spawn_keep_alive(distro)?;
    match timeout(STABLE_AFTER, child.wait()).await {
        Ok(Ok(status)) => anyhow::bail!("keep-alive exited immediately ({status})"),
        Ok(Err(error)) => anyhow::bail!("keep-alive wait failed: {error}"),
        Err(_) => Ok(KeepAlive {
            child,
            distro: distro.to_string(),
            started: Instant::now(),
        }),
    }
}

fn normalized_init(cfg: &AppConfig) -> Option<String> {
    cfg.init_command
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

async fn run_init_command(app: &AppHandle, state: &AppState, distro: &str, cmd: &str) {
    tracing::info!("Running init command in {distro}: {cmd}");
    let distro = distro.to_string();
    let cmd = cmd.to_string();
    let result = tauri::async_runtime::spawn_blocking(move || wsl::exec_in_distro(&distro, &cmd))
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!("init command task failed: {e}")));
    match result {
        Ok(output) => {
            let output = output.trim();
            if !output.is_empty() {
                tracing::info!("Init command output: {output}");
            }
        }
        Err(e) => {
            tracing::warn!("Init command failed: {e}");
            let message =
                crate::i18n::tf("error.initCommandFailed", &[("error", &e.to_string())]);
            state.runtime.write().await.last_error = Some(message);
        }
    }
    let _ = crate::domain::status::publish(app, state).await;
}

/// Waits until the config has gone untouched for `quiet`, so the settings
/// UI's debounced saves settle before we act on a changed command.
async fn wait_config_stable(state: &AppState, quiet: Duration) {
    loop {
        let elapsed = state.config.untouched_for();
        if elapsed >= quiet {
            return;
        }
        sleep(quiet - elapsed).await;
    }
}

async fn ensure_can_start(app: &AppHandle, state: &AppState, cfg: &AppConfig) -> bool {
    if state.resets.take_wsl() {
        let mut rt = state.runtime.write().await;
        rt.wsl_circuit_open = false;
        rt.wsl_circuit_opened_at = None;
        rt.wsl_failures = 0;
        rt.last_error = None;
        tracing::info!("WSL circuit breaker reset");
    }

    let mut start_error: Option<String> = None;
    {
        let mut rt = state.runtime.write().await;
        if rt.wsl_circuit_open {
            if let Some(opened) = rt.wsl_circuit_opened_at {
                if opened.elapsed() >= CIRCUIT_RESET {
                    tracing::info!("Auto-resetting WSL circuit breaker");
                    rt.wsl_circuit_open = false;
                    rt.wsl_circuit_opened_at = None;
                    rt.wsl_failures = 0;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
        if !rt.wsl_available {
            rt.distro_running = false;
            start_error = Some(crate::i18n::t("error.wslMissing").into());
            rt.last_error = start_error.clone();
        }
    }
    if start_error.is_some() {
        let _ = crate::domain::status::publish(app, state).await;
        return false;
    }

    let distros = wsl::cached_distros().unwrap_or_default();
    if !distros
        .iter()
        .any(|d| d.name.eq_ignore_ascii_case(&cfg.distro))
    {
        {
            let mut rt = state.runtime.write().await;
            rt.distro_running = false;
            rt.last_error = Some(crate::i18n::tf(
                "error.distroNotFound",
                &[("name", &cfg.distro)],
            ));
        }
        let _ = crate::domain::status::publish(app, state).await;
        return false;
    }

    true
}

async fn mark_running(
    app: &AppHandle,
    state: &AppState,
    distro: &str,
    notified_down: &mut bool,
) {
    let mut rt = state.runtime.write().await;
    if *notified_down {
        notify::notify_wsl_recovered(app, distro);
    }
    if rt.running_since.is_none() {
        rt.running_since = Some(Utc::now());
    }
    rt.distro_running = true;
    rt.wsl_failures = 0;
    rt.last_error = None;
    rt.last_check = Some(Utc::now());
    for disk in rt.disks.values_mut() {
        disk.next_retry = None;
    }
    *notified_down = false;
}

async fn mark_stopped(state: &AppState) {
    let mut rt = state.runtime.write().await;
    rt.distro_running = false;
    rt.running_since = None;
    rt.last_check = Some(Utc::now());
    for disk in rt.disks.values_mut() {
        disk.mounted = false;
        disk.device = None;
        disk.next_retry = None;
        if !disk.circuit_open {
            disk.last_error = None;
        }
    }
}

async fn wait_while_idle(app: &AppHandle, state: &AppState) {
    let cfg = state.config.get().await;
    if let Some(pause) = cfg.pause_state.as_ref().filter(|pause| pause.is_active()) {
        let wait = pause
            .until
            .signed_duration_since(Utc::now())
            .to_std()
            .unwrap_or(Duration::ZERO);
        tokio::select! {
            _ = sleep(wait) => {
                let _ = state.config.resume().await;
                notify::notify_pause_expired(app);
                tracing::info!("Pause expired, guardian resumed");
            }
            _ = state.wakers.wsl.notified() => {}
        }
        return;
    }
    state.wakers.wsl.notified().await;
}

async fn wait_retry(state: &AppState, delay: Duration) {
    tokio::select! {
        _ = sleep(delay) => {}
        _ = state.wakers.wsl.notified() => {}
    }
}

async fn record_start_failure(app: &AppHandle, state: &AppState, distro: &str, error: String) {
    tracing::error!("Failed to start WSL distro {distro}: {error}");
    let mut rt = state.runtime.write().await;
    rt.distro_running = false;
    rt.last_error = Some(error);
    rt.wsl_failures += 1;
    if rt.wsl_failures >= CIRCUIT_THRESHOLD {
        rt.wsl_circuit_open = true;
        rt.wsl_circuit_opened_at = Some(Instant::now());
        notify::notify_wsl_circuit(app, distro);
        tracing::error!("WSL circuit breaker opened for {distro}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_init_command() {
        let mut cfg = AppConfig::default();
        assert_eq!(normalized_init(&cfg), None);
        cfg.init_command = Some("   ".into());
        assert_eq!(normalized_init(&cfg), None);
        cfg.init_command = Some("  sudo service docker start ".into());
        assert_eq!(
            normalized_init(&cfg).as_deref(),
            Some("sudo service docker start")
        );
    }
}
