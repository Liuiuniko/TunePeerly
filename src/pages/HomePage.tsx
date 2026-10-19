import { useSyncExternalStore, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useApp } from "../app/model";
import { Card, Shelf } from "../components/Card";
import { Cover, LikedCover } from "../components/Cover";
import { SyncButton } from "../components/SyncButton";
import { Mascot } from "../components/Mascot";
import { clockText, greetingKey } from "../lib/format";
import { covers, displayTitle, trackKey } from "../lib/library";
import { clock, transport } from "../player/clock";
import { Icons } from "../ui/icons";

export function HomePage() {
  const { t } = useTranslation();
  const app = useApp();

  if (app.boot && app.tracks.length === 0) return <EmptyLibrary />;

  const playlists = app.boot?.playlists ?? [];
  const recent = app.recent.slice(0, 12);
  const recentlyAdded = app.library.slice().sort((a, b) => b.addedAt.localeCompare(a.addedAt)).slice(0, 12);

  return (
    <div className="page home-page">
      <h1 className="greeting">{t(greetingKey())}</h1>
      <div className="quick-grid">
        <QuickTile cover={covers(app.liked, 1, "liked")[0] ? <Cover src={covers(app.liked, 1, "liked")[0]} /> : <LikedCover />} title={t("nav.liked")} onOpen={() => app.navigate({ name: "liked" })} onPlay={() => app.playAll(app.liked, "liked")} dropId="liked" />
        <QuickTile cover={covers(app.library, 1, "library")[0] ? <Cover src={covers(app.library, 1, "library")[0]} /> : <Cover />} title={t("nav.allMusic")} onOpen={() => app.navigate({ name: "library" })} onPlay={() => app.shuffleLibrary()} playLabel={t("list.shuffleLibrary")} />
        {playlists.slice(0, 6).map((playlist) => {
          const tracks = app.index.members(app.members[playlist.id] ?? []);
          return (
            <QuickTile
              key={playlist.id}
              cover={<Cover src={covers(tracks, 1, `playlist:${playlist.id}`)[0]} />}
              title={playlist.name}
              dropId={`playlist:${playlist.id}`}
              onOpen={() => app.navigate({ name: "playlist", id: playlist.id })}
              onPlay={() => app.playAll(tracks, `playlist:${playlist.id}`)}
            />
          );
        })}
      </div>

      {app.current && <ContinueCard />}

      {recent.length > 0 && (
        <Shelf title={t("home.recent")} rail action={<button type="button" className="link-btn" onClick={() => app.navigate({ name: "recent" })}>{t("common.showAll")}</button>}>
          {recent.map((track, at) => (
            <Card
              key={trackKey(track)}
              cover={<Cover src={track.coverUrl} />}
              title={displayTitle(track, t("library.untitled"))}
              subtitle={track.artist || t("library.unknownArtist")}
              onOpen={() => app.playList(recent, at, "recent")}
              onPlay={() => app.playList(recent, at, "recent")}
            />
          ))}
        </Shelf>
      )}

      {recentlyAdded.length > 0 && (
        <Shelf title={t("home.added")} rail action={<button type="button" className="link-btn" onClick={() => app.navigate({ name: "library" })}>{t("common.showAll")}</button>}>
          {recentlyAdded.map((track, at) => (
            <Card
              key={trackKey(track)}
              cover={<Cover src={track.coverUrl} />}
              title={displayTitle(track, t("library.untitled"))}
              subtitle={track.artist || t("library.unknownArtist")}
              onOpen={() => app.playList(recentlyAdded, at, "library")}
              onPlay={() => app.playList(recentlyAdded, at, "library")}
            />
          ))}
        </Shelf>
      )}

      <Shelf
        title={t("home.playlists")}
        action={
          <span className="shelf-actions">
            <SyncButton feature="playlists" playlists quiet />
            <button type="button" className="link-btn" onClick={() => app.createPlaylist([], true)}><Icons.plus /> {t("playlist.create")}</button>
          </span>
        }
      >
        <Card cover={covers(app.liked, 1, "liked")[0] ? <Cover src={covers(app.liked, 1, "liked")[0]} /> : <LikedCover />} title={t("nav.liked")} subtitle={t("list.songCount", { count: app.liked.length })} dropId="liked" onOpen={() => app.navigate({ name: "liked" })} onPlay={() => app.playAll(app.liked, "liked")} />
        {playlists.map((playlist) => {
          const tracks = app.index.members(app.members[playlist.id] ?? []);
          return (
            <Card
              key={playlist.id}
              cover={<Cover src={covers(tracks, 1, `playlist:${playlist.id}`)[0]} />}
              title={playlist.name}
              subtitle={t("list.songCount", { count: playlist.count })}
              dropId={`playlist:${playlist.id}`}
              onOpen={() => app.navigate({ name: "playlist", id: playlist.id })}
              onPlay={() => app.playAll(tracks, `playlist:${playlist.id}`)}
            />
          );
        })}
      </Shelf>
    </div>
  );
}

function QuickTile({ cover, title, onOpen, onPlay, dropId, playLabel }: { cover: ReactNode; title: string; onOpen: () => void; onPlay: () => void; dropId?: string; playLabel?: string }) {
  const { t } = useTranslation();
  return (
    <div className="quick-tile" role="button" tabIndex={0} data-drop={dropId} onClick={onOpen} onKeyDown={(event) => {
      if (event.key === "Enter" && !(event.target as HTMLElement).closest("button, input, a, select, textarea")) onOpen();
    }}>
      {cover}
      <span className="quick-title">{title}</span>
      <button
        type="button"
        className="quick-play"
        aria-label={playLabel ?? t("list.play")}
        onClick={(event) => {
          event.stopPropagation();
          onPlay();
        }}
      >
        <Icons.play />
      </button>
    </div>
  );
}

function ContinueCard() {
  const { t } = useTranslation();
  const app = useApp();
  const time = useSyncExternalStore(clock.subscribe, clock.get);
  const { playing } = useSyncExternalStore(transport.subscribe, transport.get);
  const track = app.current;
  if (!track) return null;
  const duration = time.duration || (track.durationMs ?? 0) / 1000;
  const progress = duration > 0 ? Math.min(100, (time.position / duration) * 100) : 0;
  return (
    <section className="continue-card">
      <Cover src={track.coverUrl} className="continue-cover" />
      <div className="continue-text">
        <div className="hero-kind">{playing ? t("home.nowPlaying") : t("home.continue")}</div>
        <button type="button" className="continue-title" onClick={() => app.navigate({ name: "now" })}>
          {displayTitle(track, t("library.untitled"))}
        </button>
        <div className="continue-artist">{track.artist || t("library.unknownArtist")}</div>
        <div className="continue-progress">
          <div className="bar">
            <span style={{ width: `${progress}%` }} />
          </div>
          <span className="mono">
            {clockText(time.position)} / {clockText(duration)}
          </span>
        </div>
      </div>
      <button type="button" className="play-fab" aria-label={playing ? t("player.pause") : t("player.play")} onClick={app.toggle}>
        {playing ? <Icons.pause /> : <Icons.play />}
      </button>
    </section>
  );
}

function EmptyLibrary() {
  const { t } = useTranslation();
  const app = useApp();
  return (
    <div className="page empty-library">
      <Mascot mood="idle" size={168} />
      <h1>{t("home.emptyTitle")}</h1>
      <p>{t("home.emptyBody")}</p>
      <div className="button-row center">
        <button type="button" className="btn primary" onClick={() => void app.addMusic("directory")}>
          <Icons.folder /> {t("library.addFolder")}
        </button>
        <button type="button" className="btn soft" onClick={() => void app.addMusic("file")}>
          <Icons.music /> {t("library.addFiles")}
        </button>
      </div>
      <p className="drop-hint">{t("home.dropHint")}</p>
    </div>
  );
}
