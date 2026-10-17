import { useTranslation } from "react-i18next";
import { useApp } from "../app/model";
import { Icons } from "../ui/icons";

export const SEARCH_INPUT_ID = "global-search";

export function TopBar() {
  const { t } = useTranslation();
  const app = useApp();
  const online = app.boot?.links.filter((link) => link.state === "online").length ?? 0;
  const openSearch = () => {
    if (app.route.name === "search") return;
    app.navigate({ name: "search", id: app.route.name === "playlist" && app.route.id ? `playlist:${app.route.id}` : undefined });
  };
  return (
    <header className="topbar">
      <div className="nav-arrows">
        <button type="button" className="round-btn" aria-label={t("nav.back")} title={t("nav.backShortcut")} disabled={!app.navigation.canBack} onClick={app.back}>
          <Icons.back />
        </button>
        <button type="button" className="round-btn" aria-label={t("nav.forward")} title={t("nav.forwardShortcut")} disabled={!app.navigation.canForward} onClick={app.forward}>
          <Icons.forward />
        </button>
      </div>
      <label className="search-box">
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
      <div className="top-status">
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
