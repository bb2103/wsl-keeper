import { useEffect } from "react";
import Titlebar from "./components/Titlebar";
import Dashboard from "./screens/Dashboard";
import Settings from "./screens/Settings";
import I18nProvider from "./i18n/I18nProvider";
import { useKeeper } from "./lib/useKeeper";
import { asThemePreference, nextTheme, useResolvedTheme } from "./lib/theme";
import { keeper } from "./api";
import { errorMessage } from "./lib/format";
import ClickRipple from "./components/ClickRipple";

export default function App() {
  const keeperState = useKeeper();

  return (
    <I18nProvider preference={keeperState.config.locale}>
      <AppShell {...keeperState} />
    </I18nProvider>
  );
}

function AppShell({
  page,
  setPage,
  config,
  setConfig,
  status,
  error,
  setError,
  busy,
  refresh,
  run,
}: ReturnType<typeof useKeeper>) {
  const themePreference = asThemePreference(config.theme);
  const resolvedTheme = useResolvedTheme(themePreference);

  useEffect(() => {
    document.documentElement.classList.toggle(
      "in-tauri",
      "__TAURI_INTERNALS__" in window,
    );
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", resolvedTheme === "dark");
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.style.colorScheme = resolvedTheme;
  }, [resolvedTheme]);

  function persistTheme(next: string) {
    const updated = { ...config, theme: next };
    setConfig(updated);
    void keeper.config.save(updated).catch((e) => setError(errorMessage(e)));
  }

  return (
    <div className="app-root">
      <ClickRipple>
        <div className="app-shell" data-tone={status?.overall ?? "paused"}>
          <Titlebar
            page={page}
            overall={status?.overall}
            theme={themePreference}
            onNavigate={setPage}
            onCycleTheme={() => persistTheme(nextTheme(themePreference))}
          />

          {error && <div className="banner">{error}</div>}

          {page === "dashboard" ? (
            <Dashboard
              config={config}
              status={status}
              busy={busy}
              resolvedTheme={resolvedTheme}
              onOpenSettings={() => setPage("settings")}
              run={run}
            />
          ) : (
            <Settings
              config={config}
              status={status}
              busy={busy}
              setError={setError}
              onChange={setConfig}
              onSaved={refresh}
            />
          )}
        </div>
      </ClickRipple>
    </div>
  );
}
