import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, pickFolder } from "../api";
import { useApp } from "../app/model";
import { Icons } from "../ui/icons";

export function Overlays({ dropping }: { dropping: boolean }) {
  return (
    <>
      <Toasts />
      <DialogHost />
      <OfferDialog />
      <MoveDialog />
      <DownloadWatcher />
      {dropping && <DropOverlay />}
    </>
  );
}

function Toasts() {
  const app = useApp();
  return (
    <div className="toasts" role="status" aria-live="polite">
      {app.toasts.map((toast) => (
        <div key={toast.id} className="toast">
          {toast.text}
        </div>
      ))}
    </div>
  );
}

export function Banners() {
  const { t } = useTranslation();
  const app = useApp();
  const pending = app.boot?.pending ?? [];
  const sourceName = (id: string | null) => (id ? app.boot?.links.find((link) => link.deviceId === id)?.name ?? id.slice(0, 7) : "");
  if (!app.notice && pending.length === 0) return null;
  return (
    <div className="banners">
      {pending.map((item) => (
        <div key={item.feature} className="banner conflict" role="alert">
          <Icons.sync />
          <span>{t("conflict.body", { feature: t(`feature.${item.feature}`), device: sourceName(item.source) })}</span>
          <div className="button-row">
            <button type="button" className="btn ghost small" onClick={() => void api.network("resolve", { feature: item.feature, take: false }).then(app.scheduleRefresh).catch(app.say)}>
              {t("conflict.keep")}
            </button>
            <button type="button" className="btn primary small" onClick={() => void api.network("resolve", { feature: item.feature, take: true }).then(app.scheduleRefresh).catch(app.say)}>
              {t("conflict.take")}
            </button>
          </div>
        </div>
      ))}
      {app.notice && (
        <div className="banner notice" role="alert">
          <Icons.warning />
          <span>{app.notice}</span>
          <button type="button" className="icon-btn small" aria-label={t("common.close")} onClick={() => app.setNotice(null)}>
            <Icons.close />
          </button>
        </div>
      )}
    </div>
  );
}

function Modal({ children, onClose, label }: { children: ReactNode; onClose: () => void; label: string }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const first = box.current?.querySelector<HTMLElement>("input, textarea, button.primary, button");
    first?.focus();
    if (first instanceof HTMLInputElement) first.select();
    return () => previous?.focus?.();
  }, []);
  return (
    <div
      className="modal-layer"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={box}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label={label}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.stopPropagation();
            onClose();
          }
        }}
      >
        {children}
      </div>
    </div>
  );
}

function DialogHost() {
  const { t } = useTranslation();
  const app = useApp();
  const dialog = app.dialog;
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (dialog?.kind === "prompt") setValue(dialog.value);
    setBusy(false);
  }, [dialog]);
  if (!dialog) return null;
  const close = () => app.setDialog(null);
  const submit = async () => {
    if (busy) return;
    if (dialog.kind === "prompt" && !value.trim()) return;
    setBusy(true);
    try {
      if (dialog.kind === "prompt") await dialog.run(value.trim());
      else await dialog.run();
    } finally {
      setBusy(false);
      close();
    }
  };
  return (
    <Modal onClose={close} label={dialog.title}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <h2 className="modal-title">{dialog.title}</h2>
        {dialog.kind === "confirm" ? (
          <p className="modal-body">{dialog.body}</p>
        ) : (
          <label className="field">
            <span>{dialog.label}</span>
            <input className="input" value={value} maxLength={120} onChange={(event) => setValue(event.target.value)} />
          </label>
        )}
        <div className="modal-actions">
          <button type="button" className="btn ghost" onClick={close}>
            {t("common.cancel")}
          </button>
          <button type="submit" className={`btn ${dialog.kind === "confirm" && dialog.danger ? "danger" : "primary"}`} disabled={busy || (dialog.kind === "prompt" && !value.trim())}>
            {dialog.action}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function OfferDialog() {
  const { t } = useTranslation();
  const app = useApp();
  const offer = app.boot?.offer;
  if (!offer) return null;
  const groups = offer.safetyNumber.replace(/\s+/g, "").match(/.{1,5}/g) ?? [offer.safetyNumber];
  const answer = (action: "confirm" | "reject") => void api.network(action, offer.deviceId).then(app.scheduleRefresh).catch(app.say);
  return (
    <Modal onClose={() => answer("reject")} label={t("offer.title")}>
      <div className="offer">
        <div className="offer-icon">
          <Icons.link />
        </div>
        <h2 className="modal-title">{t("offer.title")}</h2>
        <p className="modal-body">{t("offer.body", { name: offer.name || offer.deviceId.slice(0, 7) })}</p>
        <div className="safety-number mono">
          {groups.map((group, at) => (
            <span key={at}>{group}</span>
          ))}
        </div>
        <p className="setting-help">{t("offer.help")}</p>
        <div className="modal-actions">
          <button type="button" className="btn ghost" onClick={() => answer("reject")}>
            {t("offer.reject")}
          </button>
          <button type="button" className="btn primary" onClick={() => answer("confirm")}>
            {t("offer.confirm")}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function MoveDialog() {
  const { t } = useTranslation();
  const app = useApp();
  const move = app.moves[0];
  if (!move) return null;
  const answer = (accept: boolean) => {
    app.setMoves((items) => items.slice(1));
    void api
      .moveMembership(move.previousContentId, move.nextContentId, accept)
      .then(() => accept && app.scheduleRefresh())
      .catch(app.say);
  };
  return (
    <Modal onClose={() => answer(false)} label={t("move.title")}>
      <h2 className="modal-title">{t("move.title")}</h2>
      <p className="modal-body">{t("move.body", { title: move.title })}</p>
      <div className="modal-actions">
        <button type="button" className="btn ghost" onClick={() => answer(false)}>
          {t("move.keep")}
        </button>
        <button type="button" className="btn primary" onClick={() => answer(true)}>
          {t("move.accept")}
        </button>
      </div>
    </Modal>
  );
}

function DownloadWatcher() {
  const { t } = useTranslation();
  const app = useApp();
  const download = app.boot?.download ?? null;
  const seen = useRef("");
  useEffect(() => {
    if (!download) return;
    const key = `${download.contentId}:${download.state}:${download.detail}`;
    if (key === seen.current) return;
    seen.current = key;
    const track = app.index.get({ contentId: download.contentId });
    const title = track?.title || t("library.untitled");
    switch (download.state) {
      case "started":
        app.toast(t("download.started", { title }));
        break;
      case "saved":
        app.toast(t("download.saved", { title }));
        break;
      case "busy":
        app.toast(t("download.busy"));
        break;
      case "failed":
        app.setNotice(download.detail ? t(download.detail, { defaultValue: t("download.failed", { title }) }) : t("download.failed", { title }));
        break;
      case "need-folder":
        app.setDialog({
          kind: "confirm",
          title: t("download.needFolderTitle"),
          body: t("download.needFolderBody"),
          action: t("common.choose"),
          run: async () => {
            const folder = await pickFolder();
            if (!folder) return;
            await app.setSetting("downloadFolder", folder);
            await api.network("download", download.contentId).catch(app.say);
          },
        });
        break;
    }
  }, [download, app, t]);
  return null;
}

function DropOverlay() {
  const { t } = useTranslation();
  return (
    <div className="drop-overlay">
      <div className="drop-card">
        <Icons.download />
        <div className="drop-title">{t("drop.title")}</div>
        <div className="muted">{t("drop.body")}</div>
      </div>
    </div>
  );
}
