import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type Link } from "../api";
import { useApp } from "../app/model";
import { CommitInput, Row } from "../components/controls";
import { Icons } from "../ui/icons";

export function relayLabel(relay: string, t: (key: string, options?: Record<string, unknown>) => string): { text: string; tone: string } {
  if (!relay) return { text: t("relay.off"), tone: "offline" };
  if (relay === "registered") return { text: t("relay.registered"), tone: "online" };
  if (relay === "connecting") return { text: t("relay.connecting"), tone: "pending" };
  if (relay === "badId") return { text: t("relay.badId"), tone: "failed" };
  return { text: t("relay.failed", { reason: relay.replace(/^failed:/, "") }), tone: "failed" };
}

export function DevicesPage() {
  const { t } = useTranslation();
  const app = useApp();
  const boot = app.boot;
  const [copied, setCopied] = useState(false);
  const [showCode, setShowCode] = useState(false);
  const [paste, setPaste] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  if (!boot) return null;

  const settings = app.settings;
  const networkOff = Boolean(settings.incognito) || (!settings.discovery && !settings.syncEnabled && !settings.audioShare);
  const pairedIds = new Set(boot.links.map((link) => link.deviceId));
  const nearby = boot.nearby.filter((item) => !pairedIds.has(item.deviceId) || boot.links.find((link) => link.deviceId === item.deviceId)?.state !== "online");
  const relay = relayLabel(boot.relay, t);
  const validHost = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/.test(host.trim());
  const portNumber = Number(port);
  const validPort = Number.isInteger(portNumber) && portNumber > 0 && portNumber < 65536;

  const run = (action: string, payload: unknown = null) => void api.network(action, payload).catch(app.say);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(boot.pairingText);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      app.say(error);
    }
  };
  const connect = (target: string) => {
    const [address, rawPort] = target.split(":");
    setHost(address);
    setPort(rawPort ?? "");
    run("manual", { host: address, port: Number(rawPort) });
  };
  const remove = (link: Link) =>
    app.setDialog({
      kind: "confirm",
      title: t("devices.removeTitle", { name: link.name }),
      body: t("devices.removeBody"),
      action: t("devices.remove"),
      danger: true,
      run: () => run("remove", link.deviceId),
    });

  return (
    <div className="page settings-page devices-page">
      <h1 className="page-title">{t("nav.devices")}</h1>

      {networkOff && (
        <div className="banner warn">
          <Icons.warning />
          <span>{settings.incognito ? t("devices.incognitoOff") : t("devices.networkOff")}</span>
          <button type="button" className="btn soft small" onClick={() => app.navigate({ name: "settings" })}>
            {t("nav.settings")}
          </button>
        </div>
      )}

      <section className="panel">
        <h2>{t("devices.thisDevice")}</h2>
        <Row title={t("devices.name")} help={t("devices.nameHelp")}>
          <CommitInput label={t("devices.name")} value={boot.deviceName} width={260} onCommit={(value) => void app.setSetting("displayName", value.trim().slice(0, 64)).then(app.scheduleRefresh)} />
        </Row>
        <Row title={t("devices.status")}>
          <div className="pill-row">
            <span className="state-pill online">{t("devices.port", { port: boot.listenPort || "—" })}</span>
            <span className={`state-pill ${relay.tone}`} title={boot.relay}>
              {relay.text}
            </span>
          </div>
        </Row>
        <div className="pair-code">
          <div className="setting-text">
            <div className="setting-title">{t("devices.pairingText")}</div>
            <div className="setting-help">{t("devices.pairingHelp")}</div>
          </div>
          <div className="code-box">
            <code className={showCode ? "" : "blurred"}>{boot.pairingText}</code>
            <div className="button-row">
              <button type="button" className="btn soft small" onClick={() => setShowCode((value) => !value)}>
                {showCode ? <Icons.eyeOff /> : <Icons.eye />} {showCode ? t("common.hide") : t("common.show")}
              </button>
              <button type="button" className="btn primary small" onClick={() => void copy()}>
                {copied ? <Icons.check /> : <Icons.copy />} {copied ? t("common.copied") : t("common.copy")}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section className="panel">
        <h2>{t("devices.paired")}</h2>
        {boot.links.length === 0 ? (
          <p className="muted">{t("devices.noPaired")}</p>
        ) : (
          <ul className="device-list">
            {boot.links.map((link) => (
              <li key={link.deviceId} className="device-item">
                <span className={`device-dot ${link.state}`} />
                <div className="device-text">
                  <button type="button" className="device-name" onClick={() => app.navigate({ name: "device", id: link.deviceId })}>
                    {link.name}
                  </button>
                  <div className="device-sub">
                    {t(`link.${link.state}`)}
                    {link.path && ` · ${link.path === "relay" ? t("devices.viaRelay") : t("devices.viaLan")}`}
                    {link.state === "online" && ` · ${t("list.songCount", { count: app.tracks.filter((track) => track.holderIds.includes(link.deviceId)).length })}`}
                  </div>
                </div>
                <button type="button" className="btn ghost small" onClick={() => app.navigate({ name: "device", id: link.deviceId })}>
                  {t("devices.browse")}
                </button>
                <button type="button" className="icon-btn" aria-label={t("devices.remove")} title={t("devices.remove")} onClick={() => remove(link)}>
                  <Icons.trash />
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h2>
          {t("devices.nearby")}
          {settings.discovery && !networkOff && <span className="scanning" aria-hidden="true" />}
        </h2>
        {!settings.discovery ? (
          <p className="muted">{t("devices.discoveryOff")}</p>
        ) : nearby.length === 0 ? (
          <p className="muted">{t("devices.noNearby")}</p>
        ) : (
          <ul className="device-list">
            {nearby.map((item) => (
              <li key={item.deviceId} className="device-item">
                <span className="device-dot nearby" />
                <div className="device-text">
                  <div className="device-name static">{item.name || item.deviceId.slice(0, 7)}</div>
                  <div className="device-sub mono">
                    {item.host}:{item.port}
                  </div>
                </div>
                <button type="button" className="btn primary small" onClick={() => run("pair", item.deviceId)}>
                  <Icons.link /> {t("devices.pair")}
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h2>{t("devices.manual")}</h2>
        <p className="setting-help">{t("devices.manualHelp")}</p>
        <form
          className="manual-form"
          onSubmit={(event) => {
            event.preventDefault();
            if (validHost && validPort) run("manual", { host: host.trim(), port: portNumber });
          }}
        >
          <input className={`input${host && !validHost ? " invalid" : ""}`} value={host} placeholder="192.168.1.20" aria-label={t("devices.address")} onChange={(event) => setHost(event.target.value)} />
          <input className={`input port${port && !validPort ? " invalid" : ""}`} value={port} placeholder="22000" inputMode="numeric" aria-label={t("devices.portLabel")} onChange={(event) => setPort(event.target.value.replace(/\D/g, ""))} />
          <button type="submit" className="btn primary" disabled={!validHost || !validPort || boot.manual === "dialing"}>
            {boot.manual === "dialing" ? t("devices.connecting") : t("devices.connect")}
          </button>
          {boot.manual && boot.manual !== "badText" && <span className={`state-text ${boot.manual}`}>{t(`manual.${boot.manual}`)}</span>}
        </form>
        {(settings.manualAddresses ?? []).length > 0 && (
          <div className="chip-row">
            <span className="muted small">{t("devices.recent")}</span>
            {(settings.manualAddresses ?? []).map((item) => (
              <button type="button" key={item} className="chip mono" onClick={() => connect(item)}>
                {item}
              </button>
            ))}
          </div>
        )}
      </section>

      <section className="panel">
        <h2>{t("devices.paste")}</h2>
        <p className="setting-help">{t("devices.pasteHelp")}</p>
        <textarea className={`input textarea${boot.manual === "badText" ? " invalid" : ""}`} rows={3} value={paste} placeholder={t("devices.pastePlaceholder")} aria-label={t("devices.paste")} onChange={(event) => setPaste(event.target.value)} />
        <div className="button-row">
          <button type="button" className="btn primary" disabled={!paste.trim()} onClick={() => run("paste", paste.trim())}>
            {t("devices.pair")}
          </button>
          {boot.manual === "badText" && <span className="state-text failed">{t("manual.badText")}</span>}
        </div>
      </section>
    </div>
  );
}
