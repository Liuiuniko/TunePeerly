// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../app/model";
import { Icons } from "../ui/icons";

/** The player-bar button that lists connected devices; choosing one moves playback there. */
export function DevicePicker() {
  const { t } = useTranslation();
  const app = useApp();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const others = app.devices.length > 1;

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (box.current && !box.current.contains(event.target as Node)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", key);
    };
  }, [open]);

  return (
    <div className="device-picker" ref={box}>
      <button
        type="button"
        className={`icon-btn${app.remote ? " connect-on" : open ? " on" : ""}`}
        aria-label={t("connect.pick")}
        title={app.remote ? t("connect.playingOn", { name: app.remote.name }) : t("connect.pick")}
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen(!open)}
      >
        <Icons.devices />
      </button>
      {open && (
        <div className="device-popover" role="menu" aria-label={t("connect.pick")}>
          <h3>{t("connect.title")}</h3>
          <ul>
            {app.devices.map((device) => (
              <li key={device.deviceId || "self"}>
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={device.active}
                  className={`device-row${device.active ? " active" : ""}`}
                  onClick={() => {
                    setOpen(false);
                    if (!device.active) app.transferTo(device.deviceId);
                  }}
                >
                  <span className="device-icon">{device.active ? <Icons.broadcast /> : <Icons.devices />}</span>
                  <span className="device-text">
                    <span className="device-name">{device.name}</span>
                    <span className="device-state">{device.active ? t("connect.listeningHere") : device.self ? t("connect.moveHere") : t("connect.moveThere")}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
          {!others && <p className="device-empty muted small">{app.settings.incognito ? t("connect.incognito") : t("connect.noDevices")}</p>}
        </div>
      )}
    </div>
  );
}

/** The green line under the player while another device plays. */
export function ConnectBar() {
  const { t } = useTranslation();
  const app = useApp();
  if (!app.remote) return null;
  return (
    <div className="connect-bar" role="status">
      <Icons.broadcast />
      <span>{t("connect.playingOn", { name: app.remote.name })}</span>
    </div>
  );
}
