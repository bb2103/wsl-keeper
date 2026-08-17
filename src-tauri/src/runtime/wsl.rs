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
                } else {
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
                    if let Some(cmd) = cfg
                        .init_command
                        .as_ref()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                    {
                        let distro = session.distro.clone();
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            wsl::exec_in_distro(&distro, &cmd)
                        })
                        .await;
                    }
                    mark_running(&app, &state, &session.distro, &mut notified_down).await;
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
