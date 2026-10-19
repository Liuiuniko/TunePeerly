import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../app/model";
import { reshuffleCovers } from "../lib/library";
import { Icons } from "../ui/icons";

export const SEARCH_INPUT_ID = "global-search";

export function TopBar() {
  const { t } = useTranslation();
  const app = useApp();
  const [refreshing, setRefreshing] = useState(false);
  const online = app.boot?.links.filter((link) => link.state === "online").length ?? 0;
  const incognito = Boolean(app.settings.incognito);
  const reload = () => {
    if (refreshing) return;
    setRefreshing(true);
    reshuffleCovers();
    void app.refresh().finally(() => setRefreshing(false));
  };
  const openSearch = () => {
    if (app.route.name === "search") return;
    app.navigate({ name: "search", id: app.route.name === "playlist" && app.route.id ? `playlist:${app.route.id}` : undefined });
  };
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="nav-arrows" data-tauri-drag-region="false">
        <button type="button" className="round-btn" aria-label={t("nav.back")} title={t("nav.backShortcut")} disabled={!app.navigation.canBack} onClick={app.back}>
          <Icons.back />
        </button>
        <button type="button" className="round-btn" aria-label={t("nav.forward")} title={t("nav.forwardShortcut")} disabled={!app.navigation.canForward} onClick={app.forward}>
          <Icons.forward />
        </button>
      </div>
      <label className="search-box" data-tauri-drag-region="false">
        <Icons.search />
        <input
          id={SEARCH_INPUT_ID}
          type="search"
          value={app.search}
          placeholder={t("search.placeholder")}
          aria-label={t("nav.search")}
          autoComplete="off"
          spellCheck={false}
          onFocus={() => {
            if (app.search) openSearch();
          }}
          onChange={(event) => {
            app.setSearch(event.target.value);
            openSearch();
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              if (app.search) app.setSearch("");
              else (event.target as HTMLInputElement).blur();
            }
          }}
        />
        {app.search ? (
          <button type="button" className="search-clear" aria-label={t("common.clear")} onClick={() => app.setSearch("")}>
            <Icons.close />
          </button>
        ) : (
          <kbd>Ctrl K</kbd>
        )}
      </label>
      <div className="top-actions" data-tauri-drag-region="false">
        <button type="button" className={`round-btn${refreshing ? " spinning" : ""}`} aria-label={t("top.refresh")} title={t("top.refreshHelp")} disabled={!app.boot || refreshing} onClick={reload}>
          <Icons.sync />
        </button>
        <button
          type="button"
          className={`round-btn${incognito ? " on" : ""}`}
          aria-label={t("top.incognito")}
          aria-pressed={incognito}
          title={incognito ? t("top.incognitoOn") : t("top.incognitoOff")}
          disabled={!app.boot}
          onClick={() => void app.setSetting("incognito", !incognito)}
        >
          <Icons.incognito />
        </button>
      </div>
      <div className="top-status" data-tauri-drag-region="false">
        {app.settings.incognito && (
          <button type="button" className="status-pill warn" onClick={() => app.navigate({ name: "settings" })} title={t("settings.incognitoHelp")}>
            <Icons.incognito /> <span>{t("status.incognito")}</span>
          </button>
        )}
        {app.boot?.offer && (
          <button type="button" className="status-pill accent" onClick={() => app.navigate({ name: "devices" })}>
            <Icons.link /> <span>{t("status.pairRequest")}</span>
          </button>
        )}
        {online > 0 && (
          <button type="button" className="status-pill" onClick={() => app.navigate({ name: "devices" })} title={t("status.onlineHelp")}>
            <span className="device-dot online" /> <span>{t("status.online", { count: online })}</span>
          </button>
        )}
      </div>
    </header>
  );
}
