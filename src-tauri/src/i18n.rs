//! App locale: `system` follows the Windows display language; `en` / `zh` pin it.

use std::sync::RwLock;

static PREFERENCE: RwLock<String> = RwLock::new(String::new());

#[cfg(windows)]
mod winlang {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetUserDefaultUILanguage() -> u16;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Zh,
}

pub fn set_preference(pref: &str) {
    if let Ok(mut guard) = PREFERENCE.write() {
        *guard = pref.to_string();
    }
}

pub fn current() -> Locale {
    let pref = PREFERENCE
        .read()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    resolve(&pref)
}

pub fn resolve(pref: &str) -> Locale {
    match pref {
        "en" | "en-US" | "en-GB" => Locale::En,
        "zh" | "zh-CN" | "zh-Hans" | "zh-TW" | "zh-Hant" => Locale::Zh,
        _ => detect_system(),
    }
}

fn detect_system() -> Locale {
    #[cfg(windows)]
    {
        // LANG_CHINESE = 0x04
        let langid = unsafe { winlang::GetUserDefaultUILanguage() };
        if langid & 0x3ff == 0x04 {
            return Locale::Zh;
        }
    }
    Locale::En
}

pub fn t(key: &'static str) -> &'static str {
    match current() {
        Locale::Zh => zh(key).or_else(|| en(key)).unwrap_or(key),
        Locale::En => en(key).unwrap_or(key),
    }
}

pub fn tf(key: &'static str, pairs: &[(&str, &str)]) -> String {
    let mut text = t(key).to_string();
    for (name, value) in pairs {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

fn en(key: &str) -> Option<&'static str> {
    Some(match key {
        "overall.ok" => "Guardian is running",
        "overall.starting" => "Recovering",
        "overall.paused" => "Guardian is paused",
        "overall.circuit" => "Automatic recovery stopped",
        "overall.stopped" => "WSL is stopped",
        "overall.error" => "Something needs attention",
        "tray.status" => "Status: {state}",
        "tray.open" => "Open dashboard",
        "tray.settings" => "Settings",
        "tray.pause" => "Pause guardian",
        "tray.pause.15" => "15 minutes",
        "tray.pause.60" => "1 hour",
        "tray.pause.240" => "4 hours",
        "tray.pause.1440" => "24 hours",
        "tray.pause.manual" => "Until I resume",
        "tray.resume" => "Resume guardian",
        "tray.quit" => "Quit",
        "tray.tooltip.ok" => "WSL Keeper · running · {count} mounted",
        "tray.tooltip.starting" => "WSL Keeper · recovering...",
        "tray.tooltip.pausedUntil" => "WSL Keeper · paused until {time}",
        "tray.tooltip.paused" => "WSL Keeper · paused",
        "tray.tooltip.circuit" => "WSL Keeper · mount failed (click to view)",
        "tray.tooltip.stopped" => "WSL Keeper · WSL is stopped",
        "tray.tooltip.error" => "WSL Keeper · error (click to view)",
        "notify.wslStopped" => "{distro} is not running. Starting it now.",
        "notify.wslCircuit" => {
            "{distro} failed to start repeatedly. Auto-start is paused until you retry."
        }
        "notify.diskMountFailed" => "{disk} failed to mount repeatedly. Open the app to retry.",
        "notify.pauseExpired" => "Pause ended. Guardian is running again.",
        "notify.wslRecovered" => "{distro} is running again.",
        "error.wslMissing" => "wsl.exe was not found. Install Windows Subsystem for Linux.",
        "error.selectDistro" => "Select a WSL distro in Settings.",
        "error.distroNotFound" => {
            "Distro '{name}' was not found. Pick an installed distro in Settings."
        }
        "error.startTimeout" => "Timed out starting distro",
        "error.keepAliveExited" => "Keep-alive exited",
        "error.partitionMin" => "Partition numbers start at 1",
        "error.unsupportedFs" => "Unsupported filesystem type: {fs}",
        "error.mountName" => {
            "Mount name '{name}' must be 1-32 chars of letters, digits, _ or -"
        }
        "error.duplicateMount" => "Duplicate mount name '{name}'",
        _ => return None,
    })
}

fn zh(key: &str) -> Option<&'static str> {
    Some(match key {
        "overall.ok" => "守护进程运行中",
        "overall.starting" => "正在恢复",
        "overall.paused" => "守护进程已暂停",
        "overall.circuit" => "已停止自动恢复",
        "overall.stopped" => "WSL 已停止",
        "overall.error" => "需要处理",
        "tray.status" => "状态：{state}",
        "tray.open" => "打开概览",
        "tray.settings" => "设置",
        "tray.pause" => "暂停守护",
        "tray.pause.15" => "15 分钟",
        "tray.pause.60" => "1 小时",
        "tray.pause.240" => "4 小时",
        "tray.pause.1440" => "24 小时",
        "tray.pause.manual" => "直到我恢复",
        "tray.resume" => "恢复守护",
        "tray.quit" => "退出",
        "tray.tooltip.ok" => "WSL Keeper · 运行中 · 已挂载 {count} 块磁盘",
        "tray.tooltip.starting" => "WSL Keeper · 正在恢复…",
        "tray.tooltip.pausedUntil" => "WSL Keeper · 暂停至 {time}",
        "tray.tooltip.paused" => "WSL Keeper · 已暂停",
        "tray.tooltip.circuit" => "WSL Keeper · 挂载失败（点击查看）",
        "tray.tooltip.stopped" => "WSL Keeper · WSL 已停止",
        "tray.tooltip.error" => "WSL Keeper · 出错（点击查看）",
        "notify.wslStopped" => "{distro} 未在运行，正在启动。",
        "notify.wslCircuit" => "{distro} 多次启动失败，已暂停自动启动，请手动重试。",
        "notify.diskMountFailed" => "{disk} 多次挂载失败，请打开应用重试。",
        "notify.pauseExpired" => "暂停已结束，守护进程已重新运行。",
        "notify.wslRecovered" => "{distro} 已重新运行。",
        "error.wslMissing" => "未找到 wsl.exe，请先安装 Windows Subsystem for Linux。",
        "error.selectDistro" => "请在设置中选择 WSL 发行版。",
        "error.distroNotFound" => "未找到发行版“{name}”，请在设置中选择已安装的发行版。",
        "error.startTimeout" => "启动发行版超时",
        "error.keepAliveExited" => "保活进程已退出",
        "error.partitionMin" => "分区号从 1 开始",
        "error.unsupportedFs" => "不支持的文件系统类型：{fs}",
        "error.mountName" => "挂载名“{name}”须为 1–32 位字母、数字、_ 或 -",
        "error.duplicateMount" => "挂载名“{name}”重复",
        _ => return None,
    })
}
