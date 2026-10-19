import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type Feature } from "../api";
import { useApp } from "../app/model";
import { Icons } from "../ui/icons";

/** Two actions: send this computer's copy out, or replace it with one connected device's copy. */
export function SyncButton({ feature, playlists, compact, quiet }: { feature: Feature; playlists?: boolean; compact?: boolean; quiet?: boolean }) {
  const { t } = useTranslation();
  const app = useApp();
  const boot = app.boot;
  const settings = app.settings;
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: 0, top: 0 });
  const root = useRef<HTMLDivElement>(null);
  const online = (boot?.links ?? []).filter((link) => link.state === "online" && link.deviceId !== boot?.deviceId);
  const flags = settings.syncBlocks?.[feature];
  const blocked = Boolean(flags?.manual || flags?.all);
  const incognito = Boolean(settings.incognito);
  const unavailable = !boot || incognito || online.length === 0;
  const preferred = String(settings.syncRequestDevice ?? "");
  const reason = incognito ? t("sync.pushIncognito") : online.length === 0 ? t("sync.noneOnline") : "";
  const pushTitle = blocked ? t("sync.pushBlocked") : reason || (playlists ? t("sync.pushPlaylists") : t("sync.pushTitle", { feature: t(`feature.${feature}`) }));
  const pullTitle = reason || (playlists ? t("sync.pullPlaylists") : t("sync.pullTitle", { feature: t(`feature.${feature}`) }));
  const buttonClass = compact ? "btn soft small" : quiet ? "link-btn" : "btn ghost";

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  const request = (deviceId: string, name: string) => {
    setOpen(false);
    void api.network("request-sync", { feature, deviceId }).then(() => app.toast(t("toast.requested", { name }))).catch(app.say);
  };

  return (
    <div className={`sync-actions${compact ? " compact" : ""}`} ref={root}>
      <button type="button" className={buttonClass} disabled={unavailable} title={pullTitle} aria-label={pullTitle} aria-expanded={open} onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        const height = 52 + online.length * 38;
        const below = rect.bottom + 6 + height < window.innerHeight;
        setPos({ left: Math.max(8, Math.min(rect.left, window.innerWidth - 236)), top: below ? rect.bottom + 6 : Math.max(8, rect.top - 6 - height) });
        setOpen((value) => !value);
      }}>
        <Icons.download />
        {compact ? t("sync.pull") : <span className="btn-label">{t("sync.pull")}</span>}
      </button>
      {open && (
        <div className="sync-menu" style={{ left: pos.left, top: pos.top }} role="menu" aria-label={t("sync.pullChoose")}>
          <div className="sync-menu-title">{t("sync.pullChoose")}</div>
          {online.map((link) => (
            <button type="button" role="menuitem" key={link.deviceId} className={link.deviceId === preferred ? "current" : ""} onClick={() => request(link.deviceId, link.name || link.deviceId)}>
              <span>{link.name || link.deviceId}</span>
              {link.deviceId === preferred && <span className="muted small">{t("sync.pullDefault")}</span>}
            </button>
          ))}
        </div>
      )}
      <button
        type="button"
        className={buttonClass}
        disabled={unavailable || blocked}
        title={pushTitle}
        aria-label={pushTitle}
        onClick={() => void api.network("sync", { feature, refresh: true }).then(() => app.toast(t("toast.synced"))).catch(app.say)}
      >
        <Icons.broadcast />
        {compact ? t("sync.push") : <span className="btn-label">{t("sync.push")}</span>}
      </button>
    </div>
  );
}


