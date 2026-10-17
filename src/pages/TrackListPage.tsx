import { useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, type Track } from "../api";
import { useApp, type Route } from "../app/model";
import { Collage, Cover, LikedCover } from "../components/Cover";
import { Select } from "../components/controls";
import { ActionBar, PageHeader } from "../components/PageHeader";
import { TrackTable, type Columns } from "../components/TrackTable";
import { tintFor, totalText } from "../lib/format";
import { covers, filterText, folderName, sortTracks, type Sort } from "../lib/library";
import { transport } from "../player/clock";
import { Icons } from "../ui/icons";

const sorts = new Map<string, Sort>();

type PageInfo = {
  kind: string;
  title: string;
  cover: ReactNode;
  tracks: Track[];
  contextId: string;
  defaultSort: Sort;
  columns?: Columns;
  playlistId?: string;
  empty: ReactNode;
  extraMeta?: ReactNode;
  shuffleAll?: boolean;
};

export function TrackListPage({ route }: { route: Route }) {
  const { t } = useTranslation();
  const app = useApp();
  const routeKey = `${route.name}:${route.id ?? ""}`;
  const [filter, setFilter] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  const [sharingOpen, setSharingOpen] = useState(false);
  const filterInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setFilter("");
    setFilterOpen(false);
    setSharingOpen(false);
  }, [routeKey]);

  const info = usePageInfo(route);
  const [sort, setSortState] = useState<Sort>(() => sorts.get(routeKey) ?? info.defaultSort);
  useEffect(() => setSortState(sorts.get(routeKey) ?? info.defaultSort), [routeKey]);
  const setSort = (value: Sort) => {
    sorts.set(routeKey, value);
    setSortState(value);
  };

  const shown = useMemo(() => sortTracks(filterText(info.tracks, filter), sort), [info.tracks, filter, sort]);
  const total = info.tracks.reduce((sum, track) => sum + (track.durationMs ?? 0), 0);
  const playlist = info.playlistId ? app.boot?.playlists.find((item) => item.id === info.playlistId) : undefined;
  const reorderable = Boolean(info.playlistId) && sort.key === "order" && sort.dir === 1 && !filter;

  return (
    <div className="page list-page">
      <PageHeader
        kind={info.kind}
        title={info.title}
        cover={info.cover}
        tint={route.name === "liked" ? "#d36c7c" : tintFor(info.title)}
        meta={
          <>
            {info.extraMeta}
            <span>{t("list.songCount", { count: info.tracks.length })}</span>
            {total > 0 && <span>{totalText(total, t)}</span>}
          </>
        }
      />
      <ActionBar>
        <PlayFab
          active={app.queue.contextId === info.contextId && Boolean(app.current)}
          disabled={info.tracks.length === 0}
          onPlay={() => app.playAll(shown, info.contextId)}
        />
        <button
          type="button"
          className="icon-btn lg"
          aria-label={info.shuffleAll ? t("list.shuffleLibrary") : t("list.shuffle")}
          title={info.shuffleAll ? t("list.shuffleLibrary") : t("list.shuffle")}
          disabled={info.tracks.length === 0}
          onClick={() => (info.shuffleAll ? app.shuffleLibrary() : app.playAll(shown, info.contextId, true))}
        >
          <Icons.shuffle />
        </button>
        {route.name === "library" && (
          <>
            <button type="button" className="btn ghost" onClick={() => void app.addMusic("directory")}>
              <Icons.folder /> {t("library.addFolder")}
            </button>
            <button type="button" className="btn ghost" onClick={() => void app.addMusic("file")}>
              <Icons.music /> {t("library.addFiles")}
            </button>
          </>
        )}
        {playlist && (
          <>
            <button type="button" className={`icon-btn lg${sharingOpen ? " on" : ""}`} aria-label={t("playlist.sharing")} title={t("playlist.sharing")} onClick={() => setSharingOpen((value) => !value)}>
              {playlist.exposure === "hide" ? <Icons.eyeOff /> : <Icons.eye />}
            </button>
            <button type="button" className="icon-btn lg" aria-label={t("playlist.rename")} title={t("playlist.rename")} onClick={() => app.renamePlaylist(playlist.id)}>
              <Icons.pencil />
            </button>
            <button type="button" className="icon-btn lg" aria-label={t("playlist.delete")} title={t("playlist.delete")} onClick={() => app.deletePlaylist(playlist.id)}>
              <Icons.trash />
            </button>
          </>
        )}
        <div className="action-spacer" />
        <div className={`list-filter${filterOpen || filter ? " open" : ""}`}>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("list.filter")}
            onClick={() => {
              setFilterOpen(true);
              window.setTimeout(() => filterInput.current?.focus(), 0);
            }}
          >
            <Icons.filter />
          </button>
          <input
            ref={filterInput}
            className="input"
            value={filter}
            placeholder={t("list.filterPlaceholder")}
            aria-label={t("list.filter")}
            onChange={(event) => setFilter(event.target.value)}
            onBlur={() => {
              if (!filter) setFilterOpen(false);
            }}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setFilter("");
                (event.target as HTMLInputElement).blur();
              }
            }}
          />
        </div>
        <Select
          label={t("list.sortBy")}
          value={`${sort.key}:${sort.dir}`}
          onChange={(value) => {
            const [key, dir] = value.split(":");
            setSort({ key: key as Sort["key"], dir: Number(dir) === -1 ? -1 : 1 });
          }}
          options={sortOptions(info.defaultSort, t, route.name === "recent")}
        />
      </ActionBar>
      {playlist && sharingOpen && <SharingPanel playlistId={playlist.id} exposure={playlist.exposure} />}
      <TrackTable
        tracks={shown}
        contextId={info.contextId}
        sort={sort}
        onSort={setSort}
        defaultSort={info.defaultSort}
        playlistId={info.playlistId}
        reorderable={reorderable}
        columns={info.columns}
        empty={filter ? t("list.noMatch", { text: filter }) : info.empty}
      />
    </div>
  );
}

export function PlayFab({ active, disabled, onPlay }: { active: boolean; disabled?: boolean; onPlay: () => void }) {
  const { t } = useTranslation();
  const app = useApp();
  const { playing } = useSyncExternalStore(transport.subscribe, transport.get);
  const pausing = active && playing;
  return (
    <button type="button" className="play-fab" aria-label={pausing ? t("player.pause") : t("list.play")} disabled={disabled} onClick={() => (active ? app.toggle() : onPlay())}>
      {pausing ? <Icons.pause /> : <Icons.play />}
    </button>
  );
}

function sortOptions(defaultSort: Sort, t: (key: string) => string, history: boolean) {
  const options = [
    { value: "title:1", label: t("sort.title") },
    { value: "artist:1", label: t("sort.artist") },
    { value: "album:1", label: t("sort.album") },
    { value: "folder:1", label: t("sort.folder") },
    { value: "addedAt:-1", label: t("sort.added") },
    { value: "favorite:-1", label: t("sort.liked") },
  ];
  if (defaultSort.key === "order") options.unshift({ value: "order:1", label: history ? t("sort.recent") : t("sort.custom") });
  if (defaultSort.key === "trackNumber") options.unshift({ value: "trackNumber:1", label: t("sort.trackNumber") });
  return options;
}

function SharingPanel({ playlistId, exposure }: { playlistId: string; exposure: string }) {
  const { t } = useTranslation();
  const app = useApp();
  return (
    <div className="sharing-panel">
      <div className="sharing-row">
        <div>
          <div className="setting-title">{t("playlist.exposureDefault")}</div>
          <div className="setting-help">{t("settings.exposureHelp")}</div>
        </div>
        <Select
          label={t("playlist.exposureDefault")}
          value={exposure}
          onChange={(value) => void api.setExposure("playlist", playlistId, value).then(app.scheduleRefresh).catch(app.say)}
          options={[
            { value: "follow", label: t("exposure.follow") },
            { value: "expose", label: t("exposure.expose") },
            { value: "hide", label: t("exposure.hide") },
          ]}
        />
      </div>
      <div className="sharing-row">
        <div className="setting-help">{t("settings.exposeNowHelp")}</div>
        <div className="button-row">
          <button type="button" className="btn soft" onClick={() => void api.exposeMembers("playlist", playlistId, true).then(() => { app.toast(t("toast.exposed")); app.scheduleRefresh(); }).catch(app.say)}>
            <Icons.eye /> {t("exposure.exposeAll")}
          </button>
          <button type="button" className="btn soft" onClick={() => void api.exposeMembers("playlist", playlistId, false).then(() => { app.toast(t("toast.hidden")); app.scheduleRefresh(); }).catch(app.say)}>
            <Icons.eyeOff /> {t("exposure.hideAll")}
          </button>
        </div>
      </div>
    </div>
  );
}

function usePageInfo(route: Route): PageInfo {
  const { t } = useTranslation();
  const app = useApp();
  const id = route.id ?? "";
  return useMemo((): PageInfo => {
    const base = { defaultSort: { key: "title", dir: 1 } as Sort };
    switch (route.name) {
      case "liked":
        return {
          ...base,
          kind: t("kind.playlist"),
          title: t("nav.liked"),
          cover: <LikedCover />,
          tracks: app.liked,
          contextId: "liked",
          defaultSort: { key: "addedAt", dir: -1 },
          empty: <EmptyHint icon={<Icons.heart />} title={t("empty.likedTitle")} body={t("empty.likedBody")} />,
        };
      case "recent":
        return {
          ...base,
          kind: t("kind.history"),
          title: t("nav.recent"),
          cover: <Collage covers={covers(app.recent)} />,
          tracks: app.recent,
          contextId: "recent",
          defaultSort: { key: "order", dir: 1 },
          empty: <EmptyHint icon={<Icons.clock />} title={t("empty.recentTitle")} body={t("empty.recentBody")} />,
        };
      case "playlist": {
        const playlist = app.boot?.playlists.find((item) => item.id === id);
        const tracks = app.index.members(app.members[id] ?? []);
        return {
          ...base,
          kind: t("kind.playlist"),
          title: playlist?.name ?? "",
          cover: <Collage covers={covers(tracks)} />,
          tracks,
          contextId: `playlist:${id}`,
          playlistId: id,
          defaultSort: { key: "order", dir: 1 },
          empty: <EmptyHint icon={<Icons.music />} title={t("empty.playlistTitle")} body={t("empty.playlistBody")} />,
        };
      }
      case "artist": {
        const tracks = app.library.filter((track) => track.artist === id);
        return { ...base, kind: t("kind.artist"), title: id || t("library.unknownArtist"), cover: <Cover src={covers(tracks, 1)[0]} round icon="artist" />, tracks, contextId: `artist:${id}`, empty: t("list.empty") };
      }
      case "album": {
        const tracks = app.library.filter((track) => track.album === id);
        const artists = [...new Set(tracks.map((track) => track.artist).filter(Boolean))];
        return {
          ...base,
          kind: t("kind.album"),
          title: id || t("library.unknownAlbum"),
          cover: <Cover src={covers(tracks, 1)[0]} />,
          tracks,
          contextId: `album:${id}`,
          defaultSort: { key: "trackNumber", dir: 1 },
          columns: { album: false, number: "track" },
          extraMeta: artists.length > 0 ? <strong>{artists.slice(0, 2).join(", ")}</strong> : undefined,
          empty: t("list.empty"),
        };
      }
      case "folder": {
        const tracks = app.library.filter((track) => track.folder === id);
        return { ...base, kind: t("kind.folder"), title: folderName(id), cover: <Cover src={covers(tracks, 1)[0]} icon="folder" />, tracks, contextId: `folder:${id}`, columns: { folder: false }, extraMeta: <span className="mono ellipsis">{id}</span>, empty: t("list.empty") };
      }
      case "device": {
        const link = app.boot?.links.find((item) => item.deviceId === id);
        const tracks = app.tracks.filter((track) => track.holderIds.includes(id));
        return {
          ...base,
          kind: t("kind.device"),
          title: link?.name || id.slice(0, 8),
          cover: <Cover icon="device" />,
          tracks,
          contextId: `device:${id}`,
          columns: { folder: false, added: false },
          extraMeta: link ? <span className={`state-pill ${link.state}`}>{t(`link.${link.state}`)}</span> : undefined,
          empty: <EmptyHint icon={<Icons.devices />} title={t("empty.deviceTitle")} body={t("empty.deviceBody")} />,
        };
      }
      default:
        return {
          ...base,
          kind: t("kind.library"),
          title: t("nav.allMusic"),
          cover: <Collage covers={covers(app.library)} />,
          tracks: app.library,
          contextId: "library",
          shuffleAll: true,
          empty: <EmptyHint icon={<Icons.folder />} title={t("empty.libraryTitle")} body={t("empty.libraryBody")} />,
        };
    }
  }, [route.name, id, app.liked, app.recent, app.boot, app.index, app.members, app.library, app.tracks, t]);
}

export function EmptyHint({ icon, title, body }: { icon: ReactNode; title: string; body: string }) {
  return (
    <div className="empty-hint">
      <div className="empty-icon">{icon}</div>
      <div className="empty-title">{title}</div>
      <div className="empty-body">{body}</div>
    </div>
  );
}
