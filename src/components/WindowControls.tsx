// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../api";
import { useApp } from "../app/model";
import { Icons } from "../ui/icons";
import { Modal } from "./Overlays";

export function WindowControls() {
  const { t } = useTranslation();
  const [ready, setReady] = useState(false);
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let stop = () => {};
    let gone = false;
    void import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        if (win.label !== "main") return;
        const isMaximized = await win.isMaximized();
        if (gone) return;
        setReady(true);
        setMaximized(isMaximized);
        const unlisten = await win.onResized(() => {
          void win.isMaximized().then((value) => {
            if (!gone) setMaximized(value);
          });
        });
        if (gone) unlisten();
        else stop = unlisten;
      })
      .catch(() => {});
    return () => {
      gone = true;
      stop();
    };
  }, []);
  if (!ready) return null;
  const run = (action: "minimize" | "toggleMaximize" | "close") => {
    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        if (win.label !== "main") return;
        return win[action]();
      })
      .catch((error: unknown) => {
        console.error(error);
      });
  };
  const keepClick = (event: ReactMouseEvent) => event.stopPropagation();
  return (
    <div className="window-controls" data-tauri-drag-region="false" onMouseDown={keepClick}>
      <button type="button" className="caption-btn" data-tauri-drag-region="false" aria-label={t("window.minimize")} title={t("window.minimize")} onMouseDown={keepClick} onClick={() => run("minimize")}>
        <Icons.minimize />
      </button>
      <button type="button" className="caption-btn" data-tauri-drag-region="false" aria-label={maximized ? t("window.restore") : t("window.maximize")} title={maximized ? t("window.restore") : t("window.maximize")} onMouseDown={keepClick} onClick={() => run("toggleMaximize")}>
        {maximized ? <Icons.restore /> : <Icons.maximize />}
      </button>
      <button type="button" className="caption-btn close" data-tauri-drag-region="false" aria-label={t("window.close")} title={t("window.close")} onMouseDown={keepClick} onClick={() => run("close")}>
        <Icons.close />
      </button>
    </div>
  );
}

export function CloseGate() {
  const { t, i18n } = useTranslation();
  const app = useApp();
  const [open, setOpen] = useState(false);
  const [remember, setRemember] = useState(false);
  const choice = useRef("");
  const showing = useRef(false);
  choice.current = String(app.settings.closeAction ?? "");
  showing.current = open;

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void api.setTrayMenu(t("tray.show"), t("tray.quit"), t("about.name")).catch(() => {});
  }, [t, i18n.language]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let stop = () => {};
    let gone = false;
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const unlisten = await listen("close-requested", () => {
        if (showing.current) return;
        const saved = choice.current;
        if (saved === "quit") {
          void api.quitApp();
          return;
        }
        if (saved === "tray") {
          void api.hideToTray();
          return;
        }
        setRemember(false);
        setOpen(true);
      });
      if (gone) unlisten();
      else stop = unlisten;
    });
    return () => {
      gone = true;
      stop();
    };
  }, []);

  if (!open) return null;
  const close = () => setOpen(false);
  const choose = (action: "quit" | "tray") => {
    setOpen(false);
    void (async () => {
      if (remember) await app.setSetting("closeAction", action);
      if (action === "quit") await api.quitApp();
      else await api.hideToTray();
    })();
  };
  return (
    <Modal onClose={close} label={t("window.closeAskTitle")}>
      <div className="close-ask">
        <h2 className="modal-title">{t("window.closeAskTitle")}</h2>
        <p className="modal-body">{t("window.closeAskBody")}</p>
        <button type="button" className="remember" onClick={() => setRemember((value) => !value)}>
          <span className={`check${remember ? " on" : ""}`} aria-hidden="true">
            {remember ? <Icons.check /> : null}
          </span>
          {t("window.closeRemember")}
        </button>
        <div className="modal-actions">
          <button type="button" className="btn ghost" onClick={close}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn soft" onClick={() => choose("tray")}>
            {t("window.closeTray")}
          </button>
          <button type="button" className="btn danger" onClick={() => choose("quit")}>
            {t("window.closeQuit")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
