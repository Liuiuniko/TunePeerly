import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { api, pickFolder, type BlockFlags, type Feature, type Grant } from "../api";
import { useApp } from "../app/model";
import { CommitInput, Row, Segmented, Select, Switch } from "../components/controls";
import { problemGroups } from "../lib/library";
import { Icons } from "../ui/icons";
import { relayLabel } from "./DevicesPage";

const FEATURES: Feature[] = ["favorites", "playlists", "queue", "history", "theme", "language"];
const BLOCKS = ["manual", "source", "timestamp", "all"] as const;
type BlockKey = (typeof BLOCKS)[number];

function appliesTo(feature: Feature, block: BlockKey): boolean {
  return block !== "timestamp" || feature === "history";
}

export function SettingsPage() {
  const { t } = useTranslation();
  const app = useApp();
  const boot = app.boot;
  const refs = useRef<Record<string, HTMLElement | null>>({});
  if (!boot) return null;
  const settings = app.settings;
  const set = (key: string, value: unknown) => void app.setSetting(key, value);
  const groups = problemGroups(app.tracks);
  const problems = groups.access.length + groups.playback.length;
  const relay = relayLabel(boot.relay, t);

  const sections = [
    { id: "appearance", label: t("settings.appearance") },
    { id: "library", label: t("settings.library") },
    { id: "history", label: t("settings.history") },
    { id: "network", label: t("settings.network") },
    { id: "sync", label: t("settings.sync") },
    { id: "storage", label: t("settings.storage") },
  ];

  return (
    <div className="page settings-page">
      <h1 className="page-title">{t("nav.settings")}</h1>
      <nav className="chip-row sticky-chips" aria-label={t("nav.settings")}>
        {sections.map((section) => (
          <button type="button" key={section.id} className="chip" onClick={() => refs.current[section.id]?.scrollIntoView({ behavior: "smooth", block: "start" })}>
            {section.label}
          </button>
        ))}
      </nav>

      <section className="panel" ref={(node) => { refs.current.appearance = node; }}>
        <h2>{t("settings.appearance")}</h2>
        <Row title={t("settings.theme")}>
          <Segmented
            label={t("settings.theme")}
            value={boot.theme === "dark" ? "dark" : "light"}
            onChange={(value) => set("theme", value)}
            options={[
              { value: "light", label: t("settings.light"), icon: <Icons.sun /> },
              { value: "dark", label: t("settings.dark"), icon: <Icons.moon /> },
            ]}
          />
        </Row>
        <Row title={t("settings.language")}>
          <Segmented
            label={t("settings.language")}
            value={boot.language === "zh" ? "zh" : "en"}
            onChange={(value) => set("language", value)}
            options={[
              { value: "en", label: "English" },
              { value: "zh", label: "中文" },
            ]}
          />
        </Row>
      </section>

      <section className="panel" ref={(node) => { refs.current.library = node; }}>
        <h2>{t("settings.library")}</h2>
        <p className="setting-help">{t("settings.playbackHelp")}</p>
        <div className="grant-list">
          {boot.grants.length === 0 && <p className="muted">{t("settings.noFolders")}</p>}
          {boot.grants.map((grant) => (
            <GrantRow key={grant.id} grant={grant} />
          ))}
        </div>
        <div className="button-row">
          <button type="button" className="btn primary" onClick={() => void app.addMusic("directory")}>
            <Icons.folder /> {t("library.addFolder")}
          </button>
          <button type="button" className="btn soft" onClick={() => void app.addMusic("file")}>
            <Icons.music /> {t("library.addFiles")}
          </button>
          <button
            type="button"
            className="btn ghost"
            onClick={() =>
              void api
                .rescan()
                .then(() => app.toast(t("toast.rescanned")))
                .catch(app.say)
            }
          >
            <Icons.sync /> {t("library.rescan")}
          </button>
        </div>
        <Row title={t("settings.showUnavailable")} help={t("settings.showUnavailableHelp")}>
          <Switch label={t("settings.showUnavailable")} checked={settings.showUnavailable !== false} onChange={(value) => set("showUnavailable", value)} />
        </Row>
        <Row title={t("settings.exposeDefault")} help={t("settings.exposeDefaultHelp")}>
          <Select
            label={t("settings.exposeDefault")}
            value={settings.exposeDefault === "expose" ? "expose" : "hide"}
            onChange={(value) => set("exposeDefault", value)}
            options={[
              { value: "hide", label: t("exposure.hideDefault") },
              { value: "expose", label: t("exposure.exposeDefault") },
            ]}
          />
        </Row>
        <Row title={t("settings.problems")} help={t("settings.problemsHelp")}>
          <button type="button" className="btn soft" onClick={() => app.navigate({ name: "problems" })}>
            {problems > 0 && <span className="count-badge">{problems}</span>}
            {t("settings.openProblems")} <Icons.chevronRight />
          </button>
        </Row>
      </section>

      <section className="panel" ref={(node) => { refs.current.history = node; }}>
        <h2>{t("settings.history")}</h2>
        <Row title={t("settings.historyCap")} help={t("settings.historyCapHelp")}>
          <CommitInput
            label={t("settings.historyCap")}
            type="number"
            width={110}
            value={String(settings.historyCap ?? 100)}
            onCommit={(value) => {
              const cap = Math.round(Number(value));
              if (Number.isFinite(cap) && cap >= 1 && cap <= 10000) set("historyCap", cap);
              else app.setNotice(t("settings.historyCapInvalid"));
            }}
          />
        </Row>
        <Row title={t("settings.historyMode")} help={t(`settings.historyModeHelp.${settings.historySyncMode ?? "utc"}`)}>
          <Segmented
            label={t("settings.historyMode")}
            value={settings.historySyncMode ?? "utc"}
            onChange={(value) => set("historySyncMode", value)}
            options={[
              { value: "utc", label: t("settings.historyUtc") },
              { value: "overwrite", label: t("settings.historyOverwrite") },
              { value: "follow", label: t("settings.historyFollow") },
            ]}
          />
        </Row>
        <Row title={t("settings.incognito")} help={t("settings.incognitoHelp")}>
          <Switch label={t("settings.incognito")} checked={Boolean(settings.incognito)} onChange={(value) => set("incognito", value)} />
        </Row>
      </section>

      <section className="panel" ref={(node) => { refs.current.network = node; }}>
        <h2>{t("settings.network")}</h2>
        {settings.incognito && (
          <div className="banner warn">
            <Icons.incognito /> <span>{t("settings.networkIncognito")}</span>
          </div>
        )}
        <Row title={t("settings.discovery")} help={t("settings.discoveryHelp")}>
          <Switch label={t("settings.discovery")} checked={Boolean(settings.discovery)} onChange={(value) => set("discovery", value)} />
        </Row>
        <Row title={t("settings.syncEnabled")} help={t("settings.syncEnabledHelp")}>
          <Switch label={t("settings.syncEnabled")} checked={Boolean(settings.syncEnabled)} onChange={(value) => set("syncEnabled", value)} />
        </Row>
        <Row title={t("settings.audioShare")} help={t("settings.audioShareHelp")}>
          <Switch label={t("settings.audioShare")} checked={Boolean(settings.audioShare)} onChange={(value) => set("audioShare", value)} />
        </Row>
        <Row title={t("settings.positionSync")} help={t("settings.positionSyncHelp")}>
          <Switch label={t("settings.positionSync")} checked={Boolean(settings.positionSync)} onChange={(value) => set("positionSync", value)} />
        </Row>
        <h3>{t("settings.relay")}</h3>
        <p className="setting-help">
          {t("settings.relayHelp")} <span className={`state-pill ${relay.tone}`}>{relay.text}</span>
        </p>
        <Row title={t("settings.relayHost")}>
          <CommitInput label={t("settings.relayHost")} width={240} value={String(settings.relayHost ?? "")} placeholder="relay.example.com" onCommit={(value) => set("relayHost", value.trim())} />
        </Row>
        <Row title={t("settings.relayPort")}>
          <CommitInput
            label={t("settings.relayPort")}
            type="number"
            width={110}
            value={String(settings.relayPort ?? 22067)}
            onCommit={(value) => {
              const port = Number(value);
              if (Number.isInteger(port) && port > 0 && port < 65536) set("relayPort", port);
              else app.setNotice(t("settings.portInvalid"));
            }}
          />
        </Row>
        <Row title={t("settings.relayId")} help={t("settings.relayIdHelp")}>
          <CommitInput label={t("settings.relayId")} width={320} value={String(settings.relayDeviceId ?? "")} placeholder="XXXXXXX-XXXXXXX-…" onCommit={(value) => set("relayDeviceId", value.trim())} />
        </Row>
        <Row title={t("settings.relayToken")}>
          <CommitInput label={t("settings.relayToken")} width={240} secret value={String(settings.relayToken ?? "")} onCommit={(value) => set("relayToken", value.trim())} />
        </Row>
      </section>

      <section className="panel" ref={(node) => { refs.current.sync = node; }}>
        <h2>{t("settings.sync")}</h2>
        <p className="setting-help">{t("settings.syncHelp")}</p>
        {!settings.syncEnabled && (
          <div className="banner">
            <Icons.info /> <span>{t("settings.syncOffNote")}</span>
          </div>
        )}
        <SyncTable />
      </section>

      <section className="panel" ref={(node) => { refs.current.storage = node; }}>
        <h2>{t("settings.storage")}</h2>
        <Row title={t("settings.downloadFolder")} help={settings.downloadFolder ? <span className="mono">{settings.downloadFolder}</span> : t("settings.downloadFolderNone")}>
          <button
            type="button"
            className="btn soft"
            onClick={() =>
              void pickFolder()
                .then((folder) => {
                  if (folder) set("downloadFolder", folder);
                })
                .catch(app.say)
            }
          >
            <Icons.folder /> {t("common.choose")}
          </button>
        </Row>
        <Row title={t("settings.databasePath")} help={t("settings.restartHelp")}>
          <CommitInput
            label={t("settings.databasePath")}
            width={360}
            value={boot.config.database_path}
            onCommit={(value) =>
              void api
                .updateConfig(value.trim(), boot.config.listen_port)
                .then(() => {
                  app.toast(t("toast.restartNeeded"));
                  app.scheduleRefresh();
                })
                .catch(app.say)
            }
          />
        </Row>
        <Row title={t("settings.lanPort")} help={t("settings.lanPortHelp", { port: boot.listenPort || "—" })}>
          <CommitInput
            label={t("settings.lanPort")}
            type="number"
            width={110}
            value={String(boot.config.listen_port)}
            onCommit={(value) => {
              const port = Number(value);
              if (!Number.isInteger(port) || port < 0 || port > 65535) {
                app.setNotice(t("settings.portInvalid"));
                return;
              }
              void api
                .updateConfig(boot.config.database_path, port)
                .then(() => app.scheduleRefresh())
                .catch(app.say);
            }}
          />
        </Row>
        <Row title={t("settings.deviceId")}>
          <code className="mono small selectable">{boot.certId}</code>
        </Row>
      </section>
    </div>
  );
}

function GrantRow({ grant }: { grant: Grant }) {
  const { t } = useTranslation();
  const app = useApp();
  const count = app.tracks.filter((track) => track.absolutePath && (grant.kind === "file" ? track.absolutePath === grant.path : track.absolutePath.startsWith(grant.path))).length;
  const refresh = () => app.scheduleRefresh();
  return (
    <div className="grant-row">
      <span className="grant-icon">{grant.kind === "directory" ? <Icons.folder /> : <Icons.music />}</span>
      <div className="grant-text">
        <div className="grant-path mono" title={grant.path}>
          {grant.path}
        </div>
        <div className="setting-help">
          {grant.kind === "directory" ? t("list.songCount", { count }) : t("settings.singleFile")} · {t("settings.exposeNowHelp")}
        </div>
      </div>
      <Select
        label={t("playlist.exposureDefault")}
        value={grant.exposure}
        onChange={(value) => void api.setExposure("grant", String(grant.id), value).then(refresh).catch(app.say)}
        options={[
          { value: "follow", label: t("exposure.follow") },
          { value: "expose", label: t("exposure.expose") },
          { value: "hide", label: t("exposure.hide") },
        ]}
      />
      <button type="button" className="icon-btn" title={t("exposure.exposeAll")} aria-label={t("exposure.exposeAll")} onClick={() => void api.exposeMembers("grant", String(grant.id), true).then(() => { app.toast(t("toast.exposed")); refresh(); }).catch(app.say)}>
        <Icons.eye />
      </button>
      <button type="button" className="icon-btn" title={t("exposure.hideAll")} aria-label={t("exposure.hideAll")} onClick={() => void api.exposeMembers("grant", String(grant.id), false).then(() => { app.toast(t("toast.hidden")); refresh(); }).catch(app.say)}>
        <Icons.eyeOff />
      </button>
      <button
        type="button"
        className="icon-btn danger"
        title={t("settings.removeFolder")}
        aria-label={t("settings.removeFolder")}
        onClick={() =>
          app.setDialog({
            kind: "confirm",
            title: t("settings.removeFolderTitle"),
            body: t("settings.removeFolderBody", { path: grant.path }),
            action: t("common.remove"),
            danger: true,
            run: () => void api.removeGrant(grant.id).then(refresh).catch(app.say),
          })
        }
      >
        <Icons.trash />
      </button>
    </div>
  );
}

function SyncTable() {
  const { t } = useTranslation();
  const app = useApp();
  const boot = app.boot!;
  const settings = app.settings;
  const sources = settings.syncSources ?? {};
  const blocks = settings.syncBlocks ?? {};
  const online = boot.links.filter((link) => link.state === "online");
  const disabled = !settings.syncEnabled || Boolean(settings.incognito);

  const setBlocks = (next: Partial<Record<string, BlockFlags>>) => void app.setSetting("syncBlocks", next);
  const flag = (feature: Feature, block: BlockKey) => Boolean(blocks[feature]?.[block]);
  const toggle = (feature: Feature, block: BlockKey, value: boolean) => {
    const current = { ...(blocks[feature] ?? {}) };
    if (block === "all") {
      for (const key of BLOCKS) if (appliesTo(feature, key)) current[key] = value;
    } else {
      current[block] = value;
      if (!value) current.all = false;
    }
    setBlocks({ ...blocks, [feature]: current });
  };
  const globalState = (block: BlockKey) => {
    const list = FEATURES.filter((feature) => appliesTo(feature, block));
    const count = list.filter((feature) => flag(feature, block)).length;
    return count === 0 ? "off" : count === list.length ? "on" : "mixed";
  };
  const toggleGlobal = (block: BlockKey) => {
    const value = globalState(block) !== "on";
    const next = { ...blocks };
    for (const feature of FEATURES) {
      if (!appliesTo(feature, block)) continue;
      const current = { ...(next[feature] ?? {}) };
      if (block === "all") {
        for (const key of BLOCKS) if (appliesTo(feature, key)) current[key] = value;
      } else {
        current[block] = value;
        if (!value) current.all = false;
      }
      next[feature] = current;
    }
    setBlocks(next);
  };
  const setSource = (feature: Feature, value: string) => void app.setSetting("syncSources", { ...sources, [feature]: value || null });

  const send = (feature: Feature) => {
    const source = sources[feature];
    const refresh = Boolean(source && source !== boot.deviceId);
    app.setDialog({
      kind: "confirm",
      title: t(refresh ? "sync.refreshTitle" : "sync.sendTitle", { feature: t(`feature.${feature}`) }),
      body: t("sync.sendBody", { count: online.length }),
      action: refresh ? t("sync.refresh") : t("sync.send"),
      run: () => void api.network("sync", { feature, refresh }).then(() => app.toast(t("toast.synced"))).catch(app.say),
    });
  };

  return (
    <div className={`sync-table${disabled ? " disabled" : ""}`}>
      <div className="sync-head">
        <span>{t("sync.feature")}</span>
        <span>{t("sync.source")}</span>
        <span title={t("sync.blockManualHelp")}>{t("sync.blockManual")}</span>
        <span title={t("sync.blockSourceHelp")}>{t("sync.blockSource")}</span>
        <span title={t("sync.blockTimestampHelp")}>{t("sync.blockTimestamp")}</span>
        <span title={t("sync.blockAllHelp")}>{t("sync.blockAll")}</span>
        <span />
      </div>
      <div className="sync-row global">
        <span className="sync-name">{t("sync.everything")}</span>
        <span className="muted small">{t("sync.globalHelp")}</span>
        {BLOCKS.map((block) => (
          <span key={block} className="sync-check">
            <Check state={globalState(block)} label={t(`sync.block${block[0].toUpperCase()}${block.slice(1)}`)} onChange={() => toggleGlobal(block)} />
          </span>
        ))}
        <span />
      </div>
      {FEATURES.map((feature) => {
        const historyUtc = feature === "history" && (settings.historySyncMode ?? "utc") === "utc";
        const historyManual = feature === "history" && settings.historySyncMode === "overwrite";
        const source = sources[feature] ?? "";
        const pending = boot.pending.find((item) => item.feature === feature);
        const blocked = flag(feature, "manual") || flag(feature, "all");
        return (
          <div className="sync-row" key={feature}>
            <span className="sync-name">
              {t(`feature.${feature}`)}
              {settings.dirty?.[feature] && <span className="dirty-dot" title={t("sync.dirty")} />}
              {pending && <span className="state-pill pending small">{t("sync.waiting")}</span>}
            </span>
            <span>
              {historyUtc ? (
                <span className="muted small">{t("sync.utcMerge")}</span>
              ) : historyManual ? (
                <span className="muted small">{t("sync.manualOnly")}</span>
              ) : (
                <Select
                  label={t("sync.source")}
                  value={source}
                  onChange={(value) => setSource(feature, value)}
                  options={[
                    { value: "", label: t("sync.noSource") },
                    { value: boot.deviceId, label: t("sync.thisDevice") },
                    ...boot.links.map((link) => ({ value: link.deviceId, label: link.name })),
                    ...(source && source !== boot.deviceId && !boot.links.some((link) => link.deviceId === source) ? [{ value: source, label: source.slice(0, 7) }] : []),
                  ]}
                />
              )}
            </span>
            {BLOCKS.map((block) => (
              <span key={block} className="sync-check">
                {appliesTo(feature, block) ? <Check state={flag(feature, block) ? "on" : "off"} label={t(`sync.block${block[0].toUpperCase()}${block.slice(1)}`)} onChange={(value) => toggle(feature, block, value)} /> : <span className="muted">—</span>}
              </span>
            ))}
            <span>
              {!historyUtc && (
                <button type="button" className="btn soft small" disabled={disabled || blocked || online.length === 0} title={online.length === 0 ? t("sync.noneOnline") : undefined} onClick={() => send(feature)}>
                  {source && source !== boot.deviceId ? t("sync.refresh") : t("sync.send")}
                </button>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function Check({ state, label, onChange }: { state: "on" | "off" | "mixed"; label: string; onChange: (value: boolean) => void }) {
  return (
    <button type="button" role="checkbox" aria-checked={state === "mixed" ? "mixed" : state === "on"} aria-label={label} className={`check ${state}`} onClick={() => onChange(state !== "on")}>
      {state === "on" ? <Icons.check /> : state === "mixed" ? <span className="dash" /> : null}
    </button>
  );
}
