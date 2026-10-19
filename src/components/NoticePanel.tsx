import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type Notice } from "../api";
import { useApp } from "../app/model";
import { Modal } from "./Overlays";
import { Icons } from "../ui/icons";

function when(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export function detailText(notice: Notice, t: (key: string, options?: Record<string, unknown>) => string): string {
  const detail = notice.detail;
  if (!detail) return "";
  if (detail === "lan") return t("devices.viaLan");
  if (detail === "relay") return t("devices.viaRelay");
  if (detail === "incognito" || detail === "full" || detail === "verify") {
    return t(`notice.detail.${detail}`, { defaultValue: detail });
  }
  const [feature, action] = detail.split(":");
  const featureName = t(`feature.${feature}`, { defaultValue: feature });
  const actionName = action ? t(`notice.detail.${action}`, { defaultValue: action }) : "";
  return [featureName, actionName].filter(Boolean).join(" · ");
}

export function NoticePanel() {
  const { t } = useTranslation();
  const app = useApp();
  const notices = [...(app.boot?.notices ?? [])].sort((left, right) => Number(Boolean(right.pinned)) - Number(Boolean(left.pinned)) || right.id - left.id);
  const readMode = app.settings.noticeRead === "open" || app.settings.noticeRead === "click" ? app.settings.noticeRead : "visible";
  const clearMode = app.settings.noticeClear === "unpinned" || app.settings.noticeClear === "all" ? app.settings.noticeClear : "ask";
  const [confirming, setConfirming] = useState(false);
  const [opened, setOpened] = useState<Notice | null>(null);
  const [menu, setMenu] = useState<{ notice: Notice; x: number; y: number } | null>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const sent = useRef(new Set<number>());
  const held = useRef(new Set<number>());

  const run = (payload: Record<string, unknown>) => void api.network("notice", payload).then(app.scheduleRefresh).catch(app.say);

  const unreadKey = notices.map((notice) => `${notice.id}:${notice.read ? 1 : 0}`).join(",");
  useEffect(() => {
    if (!app.noticesOpen || readMode !== "open") return;
    const ids = notices.filter((notice) => !notice.read).map((notice) => notice.id);
    if (ids.length > 0) run({ action: "read-ids", ids });
    // unreadKey changes after the marks are saved, and the effect then sees no unread ids.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app.noticesOpen, readMode, unreadKey]);

  useEffect(() => {
    const root = scroll.current;
    if (!app.noticesOpen || readMode !== "visible" || !root) return;
    const mark = () => {
      const box = root.getBoundingClientRect();
      const ids: number[] = [];
      const seen = new Set<number>();
      for (const row of root.querySelectorAll<HTMLElement>("[data-id]")) {
        const id = Number(row.dataset.id);
        if (!id) continue;
        seen.add(id);
        const rect = row.getBoundingClientRect();
        const shown = Math.min(rect.bottom, box.bottom) - Math.max(rect.top, box.top);
        const visible = shown >= Math.min(rect.height, 28);
        if (!visible) held.current.delete(id);
        if (visible && row.dataset.unread === "1" && !sent.current.has(id) && !held.current.has(id)) ids.push(id);
      }
      for (const id of [...held.current]) if (!seen.has(id)) held.current.delete(id);
      if (ids.length === 0) return;
      for (const id of ids) sent.current.add(id);
      void api.network("notice", { action: "read-ids", ids }).then(app.scheduleRefresh).catch((error: unknown) => {
        for (const id of ids) sent.current.delete(id);
        app.say(error);
      });
    };
    mark();
    root.addEventListener("scroll", mark, { passive: true });
    return () => root.removeEventListener("scroll", mark);
  }, [app.noticesOpen, readMode, unreadKey]);

  const openNotice = (notice: Notice) => {
    setOpened(notice);
    setMenu(null);
    if (!notice.read) run({ action: "read", id: notice.id });
  };
  const showMenu = (notice: Notice, x: number, y: number) => {
    setMenu({ notice, x: Math.min(x, window.innerWidth - 220), y: Math.min(y, window.innerHeight - 160) });
  };
  const clear = () => {
    if (clearMode === "ask") setConfirming(true);
    else run({ action: clearMode === "all" ? "clear-all" : "clear-unpinned" });
  };

  return (
    <aside className="queue-panel notice-panel" aria-label={t("player.notices")}>
      <header className="queue-head" data-tauri-drag-region>
        <h2>{t("player.notices")}</h2>
        <button type="button" className="icon-btn" data-tauri-drag-region="false" aria-label={t("common.close")} onClick={() => app.setNoticesOpen(false)}>
          <Icons.close />
        </button>
      </header>
      <div className="notice-toolbar">
        <button type="button" className="btn ghost small" disabled={!notices.some((notice) => !notice.read)} onClick={() => run({ action: "read-all" })}>
          {t("notice.readAll")}
        </button>
        <button type="button" className="btn ghost small" disabled={notices.length === 0} onClick={clear}>
          {t("notice.clear")}
        </button>
      </div>
      <div className="queue-scroll" ref={scroll}>
        {notices.length === 0 ? (
          <p className="muted small">{t("notice.empty")}</p>
        ) : (
          <ul className="notice-list">
            {notices.map((notice) => (
              <li key={notice.id} data-id={notice.id} data-unread={notice.read ? undefined : "1"} className={notice.pinned ? "pinned" : ""}>
                <button type="button" className="notice-main" onClick={() => openNotice(notice)} onContextMenu={(event) => { event.preventDefault(); showMenu(notice, event.clientX, event.clientY); }}>
                  {!notice.read && <span className="notice-dot" aria-label={t("notice.unread")} />}
                  {notice.pinned && <Icons.pin className="notice-pin" />}
                  <span className="notice-kind">{t(`notice.${notice.kind}`, { defaultValue: notice.kind })}</span>
                  <span className="notice-line">{[notice.name, detailText(notice, t)].filter(Boolean).join(" · ")}</span>
                  <span className="notice-time">{when(notice.at)}</span>
                </button>
                <button type="button" className="icon-btn" aria-label={t("notice.more")} onClick={(event) => showMenu(notice, event.clientX, event.clientY)}>
                  <Icons.more />
                </button>
                <button type="button" className="icon-btn" aria-label={t("notice.dismiss")} onClick={() => run({ action: "delete", id: notice.id })}>
                  <Icons.close />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {menu && (
        <>
          <button type="button" className="menu-layer" aria-label={t("common.close")} onClick={() => setMenu(null)} />
          <div className="menu" style={{ left: menu.x, top: menu.y }} role="menu">
            <button type="button" className="menu-item" onClick={() => { setMenu(null); app.setNoticesOpen(false); app.navigate({ name: "settings", id: "notices" }); }}>
              <span>{t("notice.openSettings")}</span>
            </button>
            <button type="button" className="menu-item" onClick={() => { run({ action: "unread", id: menu.notice.id }); setMenu(null); }}>
              <span>{t("notice.markUnread")}</span>
            </button>
            <button type="button" className="menu-item" onClick={() => { run({ action: menu.notice.pinned ? "unpin" : "pin", id: menu.notice.id }); setMenu(null); }}>
              <span>{menu.notice.pinned ? t("notice.unpin") : t("notice.pin")}</span>
            </button>
          </div>
        </>
      )}
      {opened && (
        <Modal onClose={() => setOpened(null)} label={t(`notice.${opened.kind}`, { defaultValue: opened.kind })}>
          <h2 className="modal-title">{t(`notice.${opened.kind}`, { defaultValue: opened.kind })}</h2>
          <p className="modal-body notice-full">{[opened.name, detailText(opened, t), when(opened.at)].filter(Boolean).join("\n")}</p>
          <div className="modal-actions">
            <button type="button" className="btn primary" onClick={() => setOpened(null)}>{t("common.close")}</button>
          </div>
        </Modal>
      )}
      {confirming && (
        <Modal onClose={() => setConfirming(false)} label={t("notice.clearTitle")}>
          <h2 className="modal-title">{t("notice.clearTitle")}</h2>
          <p className="modal-body">{t("notice.clearBody")}</p>
          <div className="modal-actions">
            <button type="button" className="btn ghost" onClick={() => setConfirming(false)}>{t("common.cancel")}</button>
            <button type="button" className="btn soft" onClick={() => { setConfirming(false); run({ action: "clear-unpinned" }); }}>{t("notice.clearKeepPinned")}</button>
            <button type="button" className="btn primary" onClick={() => { setConfirming(false); run({ action: "clear-all" }); }}>{t("notice.clearAll")}</button>
          </div>
        </Modal>
      )}
    </aside>
  );
}
