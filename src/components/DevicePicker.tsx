// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import { useEffect, useRef, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../app/model";
import { transport } from "../player/clock";
import { Icons } from "../ui/icons";
import { Switch } from "./controls";

type PickerProps = { open: boolean; setOpen: (open: boolean) => void };

/** Moving bars next to the device that is playing. */
function Eq({ on }: { on: boolean }) {
  return (
    <span className={`eq device-eq${on ? " on" : ""}`} aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
    </span>
  );
}

/**
 * The player-bar button that lists connected devices, laid out like "Connect to a device": the current
 * device first, then the others to move playback to. The switch at the bottom plays only on this
 * computer (leaving device control) or joins the others again.
 */
export function DevicePicker({ open, setOpen }: PickerProps) {
  const { t } = useTranslation();
  const app = useApp();
  const box = useRef<HTMLDivElement>(null);
  const { playing: herePlaying } = useSyncExternalStore(transport.subscribe, transport.get);

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      const target = event.target as Element | null;
      // The strip under the player opens this list too; its own click toggles it.
      if (target?.closest?.(".connect-strip")) return;
      if (box.current && !box.current.contains(target as Node)) setOpen(false);
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
  }, [open, setOpen]);

  // While this computer plays on its own it is the current device, whoever else is playing.
  const current = app.detached ? app.devices.find((device) => device.self) : app.devices.find((device) => device.active) ?? app.devices.find((device) => device.self);
  const others = app.devices.filter((device) => device !== current);
  const currentPlaying = app.remote && !app.detached ? app.remote.playing : herePlaying;

  const stateText = (device: (typeof app.devices)[number]) => {
    if (device.self && app.detached) return t("connect.soloHere");
    if (device.detached) return t("connect.soloThere");
    if (device.active && !app.detached) return app.remote ? (app.remote.playing ? t("connect.controlling") : t("connect.pausedThere")) : t("connect.listeningHere");
    if (app.detached) return device.self ? t("connect.rejoinHere") : t("connect.rejoinThere");
    return device.self ? t("connect.moveHere") : t("connect.moveThere");
  };

  return (
    <div className="device-picker" ref={box}>
      <button
        type="button"
        className={`icon-btn${app.remote ? " link-on" : app.detached ? " link-solo" : open ? " on" : ""}`}
        aria-label={t("connect.pick")}
        title={app.remote ? t("connect.playingOn", { name: app.remote.name }) : app.detached ? t("connect.soloHere") : t("connect.pick")}
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen(!open)}
      >
        {app.detached ? <Icons.unlink /> : <Icons.devices />}
      </button>
      {open && (
        <div className="device-popover" role="menu" aria-label={t("connect.pick")}>
          <h3 className="device-title">{t("connect.pick")}</h3>
          {current && (
            <div className="device-current" title={t("connect.rowTip", { name: current.name, action: stateText(current) })}>
              <span className="device-current-icon">{currentPlaying ? <Eq on /> : <Icons.volume />}</span>
              <span className="device-text">
                <span className="device-caption">{t("connect.currentDevice")}</span>
                <span className="device-name">{current.name}</span>
                <span className="device-state">{stateText(current)}</span>
              </span>
            </div>
          )}
          {others.length > 0 && (
            <>
              <div className="device-section">{app.detached ? t("connect.joinOther") : t("connect.selectOther")}</div>
              <ul>
                {others.map((device) => (
                  <li key={device.deviceId || "self"} title={t("connect.rowTip", { name: device.name, action: stateText(device) })}>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={device.detached && !device.self}
                      className={`device-row${device.detached ? " solo" : ""}`}
                      onClick={() => {
                        setOpen(false);
                        app.transferTo(device.deviceId);
                      }}
                    >
                      <span className="device-icon">{device.detached ? <Icons.unlink /> : <Icons.devices />}</span>
                      <span className="device-text">
                        <span className="device-name">{device.name}</span>
                        <span className="device-state">{stateText(device)}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
          {others.length === 0 && !app.detached && <p className="device-empty muted small">{app.settings.incognito ? t("connect.incognito") : t("connect.noDevices")}</p>}
          {!app.settings.incognito && (app.detached || others.length > 0) && (
            <div className="device-foot" title={app.detached ? t("connect.tipSoloOff") : t("connect.tipSoloOn")}>
              <span className="device-switch-label">{t("connect.soloSwitch")}</span>
              <Switch
                checked={app.detached}
                label={t("connect.soloSwitch")}
                onChange={(solo) => {
                  setOpen(false);
                  if (solo) app.detach();
                  else void app.rejoin(true);
                }}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** True while another device plays and this computer is its remote: the strip under the player shows. */
export function useConnectStrip(): boolean {
  const app = useApp();
  return Boolean(app.remote) && !app.settings.incognito && !app.detached;
}

/**
 * The full-width bar along the bottom of the window while another device plays, as in Spotify
 * Connect. Clicking it opens the device list. Playing on its own shows nothing here: this computer is
 * then simply the player, and the device button says so.
 */
export function ConnectStrip({ open, setOpen }: PickerProps) {
  const { t } = useTranslation();
  const app = useApp();
  if (!useConnectStrip() || !app.remote) return null;
  const label = app.remote.playing ? t("connect.playingOn", { name: app.remote.name }) : t("connect.pausedOn", { name: app.remote.name });
  return (
    <button type="button" className="connect-strip" aria-expanded={open} onClick={() => setOpen(!open)}>
      {/* The hover name sits on the words, so its tip shows next to them rather than mid-window. */}
      <span className="connect-strip-label" title={t("connect.title")}>
        <Icons.volume />
        <span>{label}</span>
      </span>
    </button>
  );
}
