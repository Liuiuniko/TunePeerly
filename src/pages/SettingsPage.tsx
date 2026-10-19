import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, pickFolder, type BlockFlags, type Feature, type Grant } from "../api";
import { useApp } from "../app/model";
import { CommitInput, Row, Segmented, Select, Switch } from "../components/controls";
import { SyncButton } from "../components/SyncButton";
import { problemGroups } from "../lib/library";
import { Icons } from "../ui/icons";
import { relayLabel } from "./DevicesPage";

const SOURCE_URL = "https://github.com/zhebuilds/TunePeerly";
const LICENSE_URL = "https://www.gnu.org/licenses/agpl-3.0.html";
const TERMS_URL = `${SOURCE_URL}/blob/main/ADDITIONAL_TERMS.md`;
const MARKS_URL = `${SOURCE_URL}/blob/main/TRADEMARKS.md`;
const FALLBACK_VERSION = "0.6.0";

function openLink(url: string) {
  if ("__TAURI_INTERNALS__" in window) {
    void import("@tauri-apps/plugin-opener")
      .then(({ openUrl }) => openUrl(url))
      .catch(() => {});
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

const FEATURES: Feature[] = ["favorites", "playlists", "appearance"];
const BLOCKS = ["manual", "source", "all"] as const;
type BlockKey = (typeof BLOCKS)[number];

function appliesTo(_feature: Feature, _block: BlockKey): boolean {
  return true;
}

export function SettingsPage() {
  const { t } = useTranslation();
  const app = useApp();
  const boot = app.boot;
  const refs = useRef<Record<string, HTMLElement | null>>({});
  useEffect(() => {
    if (app.route.name === "settings" && app.route.id) {
      refs.current[app.route.id]?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, [app.route.name, app.route.id]);
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
    { id: "notices", label: t("settings.notices") },
    { id: "network", label: t("settings.network") },
    { id: "sync", label: t("settings.sync") },
    { id: "storage", label: t("settings.storage") },
    { id: "about", label: t("settings.about") },
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
        <div className="panel-head">
          <h2>{t("settings.appearance")}</h2>
          <SyncButton feature="appearance" compact />
        </div>
        <Row title={t("settings.theme")}>
          <Segmented
            label={t("settings.theme")}
            value={boot.theme === "light" ? "light" : "dark"}
            onChange={(value) => set("theme", value)}
            options={[
              { value: "light", label: t("settings.light"), icon: <Icons.sun /> },
              { value: "dark", label: t("settings.dark"), icon: <Icons.moon /> },
            ]}
          />
        </Row>
        <Row title={t("settings.miniPlayer")} help={t("settings.miniPlayerHelp")}>
          <Switch label={t("settings.miniPlayer")} checked={Boolean(settings.miniPlayer)} onChange={(value) => set("miniPlayer", value)} />
        </Row>
        <Row title={t("settings.closeWindow")} help={t("settings.closeWindowHelp")}>
          <Segmented
            label={t("settings.closeWindow")}
            value={settings.closeAction === "tray" || settings.closeAction === "quit" ? String(settings.closeAction) : "ask"}
            onChange={(value) => set("closeAction", value === "ask" ? "" : value)}
            options={[
              { value: "ask", label: t("settings.closeAsk") },
              { value: "tray", label: t("settings.closeTray") },
              { value: "quit", label: t("settings.closeQuit") },
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
              { value: "hide", label: t("exposure.hideDefault"), hint: t("exposure.hideDefaultHint") },
              { value: "expose", label: t("exposure.exposeDefault"), hint: t("exposure.exposeDefaultHint") },
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
        <Row title={t("settings.historySync")} help={t("settings.historySyncHelp")}>
          <Switch
            label={t("settings.historySync")}
            checked={Boolean(settings.historySync)}
            onChange={(value) => {
              void app.setSetting("historySync", value).then(() => {
                if (!value) void app.setSetting("historyStartup", false);
                else void api.network("sync", { feature: "history", refresh: true }).catch(() => {});
              });
            }}
          />
        </Row>
        <Row title={t("settings.historyStartup")} help={t("settings.historyStartupHelp")}>
          <Switch
            label={t("settings.historyStartup")}
            checked={Boolean(settings.historySync && settings.historyStartup)}
            disabled={!settings.historySync}
            onChange={(value) => set("historyStartup", value)}
          />
        </Row>
        <Row title={t("settings.historyPause")} help={t("settings.historyPauseHelp")}>
          <Switch label={t("settings.historyPause")} checked={Boolean(settings.historyPaused)} onChange={(value) => set("historyPaused", value)} />
        </Row>
        <Row title={t("settings.incognito")} help={t("settings.incognitoHelp")}>
          <Switch label={t("settings.incognito")} checked={Boolean(settings.incognito)} onChange={(value) => set("incognito", value)} />
        </Row>
        <Row title={t("settings.silence")} help={t("settings.silenceHelp")}>
          <CommitInput
            label={t("settings.silence")}
            value={String(settings.silenceSeconds ?? 60)}
            width={120}
            onCommit={(value) => {
              const seconds = Number(value);
              if (!Number.isInteger(seconds) || seconds < 1 || seconds > 86400) {
                app.say(t("settings.silenceInvalid"));
                return;
              }
              void app.setSetting("silenceSeconds", seconds);
            }}
          />
        </Row>
        <Row title={t("settings.conflictPolicy")} help={t("settings.conflictPolicyHelp")}>
          <Select
            label={t("settings.conflictPolicy")}
            value={settings.conflictPolicy === "keep" || settings.conflictPolicy === "take" ? settings.conflictPolicy : "ask"}
            onChange={(value) => set("conflictPolicy", value)}
            options={[
              { value: "ask", label: t("settings.conflictAsk") },
              { value: "keep", label: t("settings.conflictKeep") },
              { value: "take", label: t("settings.conflictTake") },
            ]}
          />
        </Row>
      </section>

      <section className="panel" ref={(node) => { refs.current.notices = node; }}>
        <h2>{t("settings.notices")}</h2>
        <Row title={t("settings.noticeRead")} help={t("settings.noticeReadHelp")}>
          <Select
            label={t("settings.noticeRead")}
            value={settings.noticeRead === "open" || settings.noticeRead === "click" ? settings.noticeRead : "visible"}
            onChange={(value) => set("noticeRead", value)}
            options={[
              { value: "visible", label: t("settings.noticeReadVisible") },
              { value: "open", label: t("settings.noticeReadOpen") },
              { value: "click", label: t("settings.noticeReadClick") },
            ]}
          />
        </Row>
        <Row title={t("settings.noticeClear")} help={t("settings.noticeClearHelp")}>
          <Select
            label={t("settings.noticeClear")}
            value={settings.noticeClear === "unpinned" || settings.noticeClear === "all" ? settings.noticeClear : "ask"}
            onChange={(value) => set("noticeClear", value)}
            options={[
              { value: "ask", label: t("settings.noticeClearAsk") },
              { value: "unpinned", label: t("settings.noticeClearUnpinned") },
              { value: "all", label: t("settings.noticeClearAll") },
            ]}
          />
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
        <Row title={t("settings.autoSend")} help={t("settings.autoSendHelp")}>
          <div className="chip-row">
            {FEATURES.map((feature) => {
              const on = Boolean(settings.autoSend?.[feature]);
              return (
                <button
                  key={feature}
                  type="button"
                  className={`chip small${on ? " on" : ""}`}
                  aria-pressed={on}
                  onClick={() => set("autoSend", { ...(settings.autoSend ?? {}), [feature]: !on })}
                >
                  {t(`feature.${feature}`)}
                </button>
              );
            })}
          </div>
        </Row>
        <Row title={t("settings.positionSync")} help={t("settings.positionSyncHelp")}>
          <Select
            label={t("settings.positionSync")}
            value={String(settings.positionSyncDevice ?? "")}
            disabled={Boolean(settings.incognito)}
            onChange={(value) => set("positionSyncDevice", value)}
            options={[
              { value: "", label: t("settings.positionSyncOff") },
              ...boot.links
                .filter((link) => link.deviceId !== boot.deviceId && (link.state === "online" || link.deviceId === settings.positionSyncDevice))
                .map((link) => ({ value: link.deviceId, label: link.name || link.deviceId })),
            ]}
          />
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
        <Row title={t("settings.requestDevice")} help={t("settings.requestDeviceHelp")}>
          <Select
            label={t("settings.requestDevice")}
            value={String(settings.syncRequestDevice ?? "")}
            onChange={(value) => set("syncRequestDevice", value)}
            options={[{ value: "", label: t("sync.requestNone") }, ...boot.links.map((link) => ({ value: link.deviceId, label: link.name || link.deviceId }))]}
          />
        </Row>
        <Row title={t("settings.overwriteMode")} help={t("settings.overwriteModeHelp")}>
          <Select
            label={t("settings.overwriteMode")}
            value={settings.overwriteMode === "ask" ? "ask" : "silent"}
            onChange={(value) => set("overwriteMode", value)}
            options={[
              { value: "silent", label: t("devices.overwriteSilent") },
              { value: "ask", label: t("devices.overwriteAsk") },
            ]}
          />
        </Row>
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
        <Row
          title={t("settings.lanPort")}
          help={
            <>
              <div>{t("settings.lanPortHelp", { port: boot.listenPort || "—" })}</div>
              {boot.portFallback ? <div className="setting-error">{t("settings.portFallback", { saved: boot.portFallback, port: boot.listenPort || "—" })}</div> : null}
            </>
          }
        >
          <CommitInput
            label={t("settings.lanPort")}
            type="number"
            width={110}
            value={String(settings.listenPort || "")}
            onCommit={(value) => {
              const port = Number(value);
              if (!Number.isInteger(port) || port < 1024 || port > 65535) {
                app.setNotice(t("settings.portInvalid"));
                return;
              }
              void app.setSetting("listenPort", port).then(() => app.scheduleRefresh());
            }}
          />
        </Row>
        <Row title={t("settings.deviceId")}>
          <code className="mono small selectable">{boot.certId}</code>
        </Row>
      </section>

      <AboutPanel nodeRef={(node) => { refs.current.about = node; }} />
    </div>
  );
}

function AboutPanel({ nodeRef }: { nodeRef: (node: HTMLElement | null) => void }) {
  const { t } = useTranslation();
  const [version, setVersion] = useState(FALLBACK_VERSION);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let gone = false;
    void import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion())
      .then((value) => {
        if (!gone && value) setVersion(value);
      })
      .catch(() => {});
    return () => {
      gone = true;
    };
  }, []);
  return (
    <section className="panel" ref={nodeRef}>
      <h2>{t("settings.about")}</h2>
      <div className="about-head">
        <img className="about-icon" src="/favicon.png" alt="" />
        <div>
          <div className="about-name">{t("about.name")}</div>
          <div className="about-version">{t("about.version", { version })}</div>
        </div>
      </div>
      <p className="setting-help about-summary">{t("about.summary")}</p>
      <h3>{t("about.credits")}</h3>
      <p className="setting-help">{t("about.creditsOriginal")}</p>
      <p className="setting-help">{t("about.creditsModify")}</p>
      <p className="setting-help">{t("about.creditsBrand")}</p>
      <Row title={t("about.license")} help={t("about.licenseHelp")}>
        <button type="button" className="btn soft" onClick={() => openLink(LICENSE_URL)}>
          {t("about.licenseName")} <Icons.external />
        </button>
      </Row>
      <Row title={t("about.additionalTerms")}>
        <button type="button" className="btn soft" onClick={() => openLink(TERMS_URL)}>
          {t("about.open")} <Icons.external />
        </button>
      </Row>
      <Row title={t("about.trademarks")}>
        <button type="button" className="btn soft" onClick={() => openLink(MARKS_URL)}>
          {t("about.open")} <Icons.external />
        </button>
      </Row>
      <Row
        title={t("about.source")}
        help={
          <>
            <div className="mono selectable about-url">{SOURCE_URL}</div>
            <div>{t("about.sourceNote")}</div>
          </>
        }
      >
        <button type="button" className="btn soft" onClick={() => openLink(SOURCE_URL)}>
          {t("about.open")} <Icons.external />
        </button>
      </Row>
      <Row title={t("about.author")} help={t("about.copyright")} />
      <Row title={t("about.privacy")} help={t("about.privacyHelp")} />
      <Row title={t("about.components")} help={t("about.componentsHelp")} />
    </section>
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
          { value: "follow", label: t("exposure.follow"), hint: t("exposure.followHint") },
          { value: "expose", label: t("exposure.expose"), hint: t("exposure.exposeHint") },
          { value: "hide", label: t("exposure.hide"), hint: t("exposure.hideHint") },
        ]}
      />
      <button type="button" className="icon-btn" title={`${t("exposure.exposeAll")}: ${t("exposure.exposeAllHint")}`} aria-label={t("exposure.exposeAll")} onClick={() => void api.exposeMembers("grant", String(grant.id), true).then(() => { app.toast(t("toast.exposed")); refresh(); }).catch(app.say)}>
        <Icons.eye />
      </button>
      <button type="button" className="icon-btn" title={`${t("exposure.hideAll")}: ${t("exposure.hideAllHint")}`} aria-label={t("exposure.hideAll")} onClick={() => void api.exposeMembers("grant", String(grant.id), false).then(() => { app.toast(t("toast.hidden")); refresh(); }).catch(app.say)}>
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
  const disabled = Boolean(settings.incognito);

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
  const setSource = (feature: Feature, value: string) => void app.setSetting("syncSources", { ...sources, [feature]: value || boot.deviceId });

  return (
    <div className={`sync-table${disabled ? " disabled" : ""}`}>
      <div className="sync-head">
        <span>{t("sync.feature")}</span>
        <span title={t("sync.sourceHelp")}>{t("sync.source")}</span>
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
        const stored = sources[feature] ?? "";
        const remote = stored && stored !== boot.deviceId ? stored : "";
        const online = boot.links.filter((link) => link.state === "online" && link.deviceId !== boot.deviceId);
        const options = [{ value: "", label: t("sync.thisDevice") }, ...online.map((link) => ({ value: link.deviceId, label: link.name }))];
        if (remote && !options.some((item) => item.value === remote)) {
          const current = boot.links.find((link) => link.deviceId === remote);
          if (current) options.push({ value: current.deviceId, label: current.name });
        }
        const pending = boot.pending.find((item) => item.feature === feature);
        return (
          <div className="sync-row" key={feature}>
            <span className="sync-name">
              {t(`feature.${feature}`)}
              {settings.dirty?.[feature] && <span className="dirty-dot" title={t("sync.dirty")} />}
              {pending && <span className="state-pill pending small">{t("sync.waiting")}</span>}
            </span>
            <span className="sync-follow">
              <Select
                label={t("sync.source")}
                value={remote}
                onChange={(value) => setSource(feature, value)}
                options={options}
              />
            </span>
            {BLOCKS.map((block) => (
              <span key={block} className="sync-check">
                {appliesTo(feature, block) ? <Check state={flag(feature, block) ? "on" : "off"} label={t(`sync.block${block[0].toUpperCase()}${block.slice(1)}`)} onChange={(value) => toggle(feature, block, value)} /> : <span className="muted">—</span>}
              </span>
            ))}
            <span>
              <SyncButton feature={feature} playlists={feature === "playlists"} compact />
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
