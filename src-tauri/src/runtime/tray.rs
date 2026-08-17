use crate::domain::actions;
use crate::domain::status::{NAVIGATE_EVENT, OverallKind};
use crate::i18n;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub fn icon_for(kind: OverallKind) -> Image<'static> {
    let bytes: &'static [u8] = match kind {
        OverallKind::Ok => include_bytes!("../../icons/tray-ok.png"),
        OverallKind::Paused => include_bytes!("../../icons/tray-paused.png"),
        OverallKind::Starting => include_bytes!("../../icons/tray-warn.png"),
        OverallKind::Stopped => include_bytes!("../../icons/tray-idle.png"),
        OverallKind::Circuit | OverallKind::Error => include_bytes!("../../icons/tray-error.png"),
    };
    Image::from_bytes(bytes).expect("tray icon")
}

struct TrayItems {
    status: MenuItem<Wry>,
    open: MenuItem<Wry>,
    settings: MenuItem<Wry>,
    pause: Submenu<Wry>,
    pause_15: MenuItem<Wry>,
    pause_60: MenuItem<Wry>,
    pause_240: MenuItem<Wry>,
    pause_1440: MenuItem<Wry>,
    pause_manual: MenuItem<Wry>,
    resume: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

static ITEMS: Mutex<Option<TrayItems>> = Mutex::new(None);

pub fn setup_tray(app: &AppHandle) -> anyhow::Result<()> {
    let status_i = MenuItem::with_id(app, "status", tray_status_fallback(), false, None::<&str>)?;
    let open_i = MenuItem::with_id(app, "open", i18n::t("tray.open"), true, None::<&str>)?;
    let settings_i = MenuItem::with_id(app, "settings", i18n::t("tray.settings"), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let pause_15 = MenuItem::with_id(app, "pause_15", i18n::t("tray.pause.15"), true, None::<&str>)?;
    let pause_60 = MenuItem::with_id(app, "pause_60", i18n::t("tray.pause.60"), true, None::<&str>)?;
    let pause_240 = MenuItem::with_id(app, "pause_240", i18n::t("tray.pause.240"), true, None::<&str>)?;
    let pause_1440 =
        MenuItem::with_id(app, "pause_1440", i18n::t("tray.pause.1440"), true, None::<&str>)?;
    let pause_manual =
        MenuItem::with_id(app, "pause_manual", i18n::t("tray.pause.manual"), true, None::<&str>)?;
    let resume_i = MenuItem::with_id(app, "resume", i18n::t("tray.resume"), true, None::<&str>)?;

    let pause_menu = Submenu::with_id_and_items(
        app,
        "pause",
        i18n::t("tray.pause"),
        true,
        &[&pause_15, &pause_60, &pause_240, &pause_1440, &pause_manual],
    )?;

    let quit_i = MenuItem::with_id(app, "quit", i18n::t("tray.quit"), true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_i,
            &separator,
            &open_i,
            &settings_i,
            &separator,
            &pause_menu,
            &resume_i,
            &separator,
            &quit_i,
        ],
    )?;

    if let Ok(mut guard) = ITEMS.lock() {
        *guard = Some(TrayItems {
            status: status_i,
            open: open_i,
            settings: settings_i,
            pause: pause_menu,
            pause_15,
            pause_60,
            pause_240,
            pause_1440,
            pause_manual,
            resume: resume_i,
            quit: quit_i,
        });
    }

    TrayIconBuilder::with_id("main-tray")
        .icon(icon_for(OverallKind::Stopped))
        .tooltip("WSL Keeper")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let _ = show_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let _ = show_window(app);
            }
            "settings" => {
                let _ = show_window(app);
                let _ = app.emit(NAVIGATE_EVENT, "settings");
            }
            "pause_15" => actions::pause_from_tray(app, Some(15)),
            "pause_60" => actions::pause_from_tray(app, Some(60)),
            "pause_240" => actions::pause_from_tray(app, Some(240)),
            "pause_1440" => actions::pause_from_tray(app, Some(1440)),
            "pause_manual" => actions::pause_from_tray(app, None),
            "resume" => actions::resume_from_tray(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

pub fn apply_labels(_app: &AppHandle) {
    let Ok(guard) = ITEMS.lock() else {
        return;
    };
    let Some(items) = guard.as_ref() else {
        return;
    };
    let _ = items.open.set_text(i18n::t("tray.open"));
    let _ = items.settings.set_text(i18n::t("tray.settings"));
    let _ = items.pause.set_text(i18n::t("tray.pause"));
    let _ = items.pause_15.set_text(i18n::t("tray.pause.15"));
    let _ = items.pause_60.set_text(i18n::t("tray.pause.60"));
    let _ = items.pause_240.set_text(i18n::t("tray.pause.240"));
    let _ = items.pause_1440.set_text(i18n::t("tray.pause.1440"));
    let _ = items.pause_manual.set_text(i18n::t("tray.pause.manual"));
    let _ = items.resume.set_text(i18n::t("tray.resume"));
    let _ = items.quit.set_text(i18n::t("tray.quit"));
}

pub fn set_status_label(text: impl AsRef<str>) {
    let Ok(guard) = ITEMS.lock() else {
        return;
    };
    if let Some(items) = guard.as_ref() {
        let _ = items.status.set_text(text);
    }
}

fn tray_status_fallback() -> String {
    i18n::tf("tray.status", &[("state", i18n::t("overall.starting"))])
}

pub fn show_window(app: &AppHandle) -> anyhow::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}
