import { useEffect, useState } from "react";
import { Copy, Desktop, Minus, Moon, Square, Sun, X } from "@phosphor-icons/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { OverallKind } from "../api/types";
import type { Page } from "../lib/useKeeper";
import type { ThemePreference } from "../lib/theme";
import { useI18n } from "../i18n";
import GradientText from "./GradientText";
import { Tabs, TabsList, TabsTrigger } from "./ui/tabs";

interface Props {
  page: Page;
  overall: OverallKind | undefined;
  theme: ThemePreference;
  onNavigate: (page: Page) => void;
  onCycleTheme: () => void;
}

export default function Titlebar({ page, overall, theme, onNavigate, onCycleTheme }: Props) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void (async () => {
      try {
        const next = await win.isMaximized();
        if (!cancelled) setMaximized(next);
        unlisten = await win.onResized(async () => {
          setMaximized(await win.isMaximized());
        });
      } catch {
        // Preview in a normal browser has no window chrome IPC.
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("is-maximized", maximized);
  }, [maximized]);

  async function run(action: "minimize" | "maximize" | "close") {
    const win = getCurrentWindow();
    try {
      if (action === "minimize") await win.minimize();
      if (action === "maximize") await win.toggleMaximize();
      if (action === "close") await win.close();
    } catch {
      // Ignore preview-mode failures.
    }
  }

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <span className={`pulse-dot ${overall ?? "paused"}`} />
        <GradientText
          className="brand-mark"
          colors={["#3f9d6e", "#64748b", "#3f9d6e"]}
          animationSpeed={6}
        >
          WSL Keeper
        </GradientText>
      </div>

      <Tabs
        value={page}
        onValueChange={(value) => onNavigate(value as Page)}
        aria-label={t("nav.label")}
      >
        <TabsList>
          <TabsTrigger value="dashboard">{t("nav.dashboard")}</TabsTrigger>
          <TabsTrigger value="settings">{t("nav.settings")}</TabsTrigger>
        </TabsList>
      </Tabs>

      <button
        type="button"
        className="caption-btn theme-btn"
        title={`${t("theme.cycle")} (${
          theme === "light"
            ? t("theme.light")
            : theme === "dark"
              ? t("theme.dark")
              : t("theme.system")
        })`}
        aria-label={t("theme.cycle")}
        onClick={onCycleTheme}
      >
        {theme === "light" ? (
          <Sun size={14} weight="bold" />
        ) : theme === "dark" ? (
          <Moon size={14} weight="bold" />
        ) : (
          <Desktop size={14} weight="bold" />
        )}
      </button>

      <div className="window-controls">
        <button
          type="button"
          className="caption-btn"
          title={t("window.minimize")}
          aria-label={t("window.minimize")}
          onClick={() => void run("minimize")}
        >
          <Minus size={11} weight="bold" />
        </button>
        <button
          type="button"
          className="caption-btn"
          title={maximized ? t("window.restore") : t("window.maximize")}
          aria-label={maximized ? t("window.restore") : t("window.maximize")}
          onClick={() => void run("maximize")}
        >
          {maximized ? (
            <Copy size={10} weight="bold" />
          ) : (
            <Square size={10} weight="bold" />
          )}
        </button>
        <button
          type="button"
          className="caption-btn close"
          title={t("window.close")}
          aria-label={t("window.close")}
          onClick={() => void run("close")}
        >
          <X size={12} weight="bold" />
        </button>
      </div>
    </header>
  );
}
