import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type Track } from "../api";
import { useApp } from "../app/model";
import { Card, Shelf } from "../components/Card";
import { Collage, Cover, LikedCover } from "../components/Cover";
import { TrackTable } from "../components/TrackTable";
import { tintFor } from "../lib/format";
import { covers, displayTitle, groupTracks } from "../lib/library";
import { Icons } from "../ui/icons";
import { EmptyHint } from "./TrackListPage";

export function SearchPage({ scope: initialScope }: { scope?: string }) {
  const { t } = useTranslation();
  const app = useApp();
  const [scope, setScope] = useState(initialScope ?? "all");
  const [results, setResults] = useState<Track[] | null>(null);
  const [showAll, setShowAll] = useState(false);
  const ticket = useRef(0);
  const query = app.search.trim();

  useEffect(() => setScope(initialScope ?? "all"), [initialScope]);

  useEffect(() => {
    setShowAll(false);
    if (!query) {
      setResults(null);
      return;
    }
    const local = app.findTracks(query, scope);
    if (local) {
      setResults(local);
      return;
    }
    const mine = ++ticket.current;
    const timer = window.setTimeout(() => {
      void api
        .search(query)
        .then((items) => {
          if (mine === ticket.current) setResults(items);
        })
        .catch(() => {
          if (mine === ticket.current) setResults([]);
        });
    }, 160);
    return () => window.clearTimeout(timer);
  }, [query, scope, app.findTracks, app.tracks]);

  const lower = query.toLowerCase();
  const artists = useMemo(() => (results ? groupTracks(results, "artist").filter((group) => group.name && group.name.toLowerCase().includes(lower)).slice(0, 8) : []), [results, lower]);
  const albums = useMemo(() => (results ? groupTracks(results, "album").filter((group) => group.name && group.name.toLowerCase().includes(lower)).slice(0, 8) : []), [results, lower]);
  const playlists = useMemo(() => (app.boot?.playlists ?? []).filter((item) => lower && item.name.toLowerCase().includes(lower)), [app.boot, lower]);

  const scopes = [
    { id: "all", label: t("search.all") },
    ...(initialScope?.startsWith("playlist:") ? [{ id: initialScope, label: t("search.thisPlaylist") }] : []),
    { id: "queue", label: t("search.queue") },
  ];

  return (
    <div className="page search-page">
      <div className="chip-row">
        {scopes.map((item) => (
          <button type="button" key={item.id} className={`chip${scope === item.id ? " on" : ""}`} onClick={() => setScope(item.id)}>
            {item.label}
          </button>
        ))}
      </div>

      {!query && <BrowseTiles />}

      {query && results && results.length === 0 && artists.length === 0 && playlists.length === 0 && (
        <EmptyHint icon={<Icons.search />} title={t("search.noResults", { text: query })} body={t("search.noResultsBody")} />
      )}

      {query && results && results.length > 0 && (
        <>
          <div className="search-top">
            <section className="top-result">
              <h2>{t("search.top")}</h2>
              <div className="top-card" role="button" tabIndex={0} onClick={() => app.playList(results, 0, "search")} onKeyDown={(event) => {
                if (event.key === "Enter") app.playList(results, 0, "search");
              }}>
                <Cover src={results[0].coverUrl} className="top-cover" />
                <div className="top-title">{displayTitle(results[0], t("library.untitled"))}</div>
                <div className="top-sub">
                  <span className="pill-label">{t("kind.song")}</span>
                  {results[0].artist || t("library.unknownArtist")}
                </div>
                <button type="button" className="play-fab small" aria-label={t("list.play")} onClick={(event) => {
                  event.stopPropagation();
                  app.playList(results, 0, "search");
                }}>
                  <Icons.play />
                </button>
              </div>
            </section>
            <section className="top-songs">
              <div className="shelf-head">
                <h2>{t("search.songs")}</h2>
                {results.length > 5 && (
                  <button type="button" className="link-btn" onClick={() => setShowAll((value) => !value)}>
                    {showAll ? t("common.showLess") : t("common.showAll")}
                  </button>
                )}
              </div>
              <TrackTable tracks={showAll ? results : results.slice(0, 5)} contextId="search" columns={{ folder: false, added: false, source: true }} />
            </section>
          </div>
        </>
      )}

      {query && artists.length > 0 && (
        <Shelf title={t("search.artists")} rail>
          {artists.map((group) => (
            <Card key={group.name} cover={<Cover src={group.cover} round icon="artist" />} title={group.name} subtitle={t("kind.artist")} onOpen={() => app.navigate({ name: "artist", id: group.name })} onPlay={() => app.playAll(app.contextTracks(`artist:${group.name}`), `artist:${group.name}`)} />
          ))}
        </Shelf>
      )}
      {query && albums.length > 0 && (
        <Shelf title={t("search.albums")} rail>
          {albums.map((group) => (
            <Card key={group.name} cover={<Cover src={group.cover} />} title={group.name} subtitle={group.tracks[0]?.artist || t("kind.album")} onOpen={() => app.navigate({ name: "album", id: group.name })} onPlay={() => app.playAll(app.contextTracks(`album:${group.name}`), `album:${group.name}`)} />
          ))}
        </Shelf>
      )}
      {query && playlists.length > 0 && (
        <Shelf title={t("search.playlists")} rail>
          {playlists.map((playlist) => {
            const tracks = app.index.members(app.members[playlist.id] ?? []);
            return <Card key={playlist.id} cover={<Collage covers={covers(tracks)} />} title={playlist.name} subtitle={t("list.songCount", { count: playlist.count })} onOpen={() => app.navigate({ name: "playlist", id: playlist.id })} onPlay={() => app.playAll(tracks, `playlist:${playlist.id}`)} />;
          })}
        </Shelf>
      )}
    </div>
  );
}

function BrowseTiles() {
  const { t } = useTranslation();
  const app = useApp();
  const tiles = [
    { label: t("nav.allMusic"), go: () => app.navigate({ name: "library" }), icon: <Icons.music /> },
    { label: t("nav.liked"), go: () => app.navigate({ name: "liked" }), icon: <LikedCover className="tile-art" /> },
    { label: t("nav.recent"), go: () => app.navigate({ name: "recent" }), icon: <Icons.clock /> },
    { label: t("nav.artists"), go: () => app.navigate({ name: "artists" }), icon: <Icons.artist /> },
    { label: t("nav.albums"), go: () => app.navigate({ name: "albums" }), icon: <Icons.album /> },
    { label: t("nav.folders"), go: () => app.navigate({ name: "folders" }), icon: <Icons.folder /> },
    { label: t("nav.devices"), go: () => app.navigate({ name: "devices" }), icon: <Icons.devices /> },
  ];
  return (
    <section className="shelf">
      <div className="shelf-head">
        <h2>{t("search.browse")}</h2>
      </div>
      <div className="browse-tiles">
        {tiles.map((tile) => (
          <button type="button" key={tile.label} className="browse-tile" style={{ background: tintFor(tile.label) }} onClick={tile.go}>
            <span>{tile.label}</span>
            <span className="browse-icon">{tile.icon}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
