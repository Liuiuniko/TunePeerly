// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useApp, type RouteName } from "../app/model";
import { Card } from "../components/Card";
import { Cover } from "../components/Cover";
import { filterText, folderName, groupTracks } from "../lib/library";
import { Icons } from "../ui/icons";
import { EmptyHint } from "./TrackListPage";

const fields = { artists: "artist", albums: "album", folders: "folder" } as const;
const targets: Record<keyof typeof fields, RouteName> = { artists: "artist", albums: "album", folders: "folder" };

export function BrowsePage({ kind }: { kind: keyof typeof fields }) {
  const { t } = useTranslation();
  const app = useApp();
  const [filter, setFilter] = useState("");
  const groups = useMemo(() => groupTracks(filterText(app.library, filter), fields[kind]), [app.library, filter, kind]);
  const title = t(`nav.${kind}`);
  return (
    <div className="page browse-page">
      <div className="browse-head">
        <h1>{title}</h1>
        <div className="list-filter open">
          <span className="icon-btn" aria-hidden="true">
            <Icons.filter />
          </span>
          <input className="input" value={filter} placeholder={t("list.filterPlaceholder")} aria-label={t("list.filter")} onChange={(event) => setFilter(event.target.value)} />
        </div>
      </div>
      {groups.length === 0 ? (
        <EmptyHint icon={kind === "artists" ? <Icons.artist /> : kind === "albums" ? <Icons.album /> : <Icons.folder />} title={t("empty.browseTitle")} body={t("empty.libraryBody")} />
      ) : (
        <div className="card-grid">
          {groups.map((group) => {
            const name = kind === "folders" ? folderName(group.name) : group.name || (kind === "artists" ? t("library.unknownArtist") : t("library.unknownAlbum"));
            const routeId = group.id;
            const contextId = `${targets[kind]}:${routeId}`;
            const subtitle =
              kind === "albums"
                ? [...new Set(group.tracks.map((track) => track.artist).filter(Boolean))].slice(0, 2).join(", ") || t("list.songCount", { count: group.tracks.length })
                : kind === "folders"
                  ? <span className="mono ellipsis" title={group.name}>{group.name}</span>
                  : t("list.songCount", { count: group.tracks.length });
            return (
              <Card
                key={group.id || "_"}
                cover={<Cover src={group.cover} round={kind === "artists"} icon={kind === "artists" ? "artist" : kind === "folders" ? "folder" : "music"} />}
                title={name}
                subtitle={subtitle}
                onOpen={() => app.navigate({ name: targets[kind], id: routeId })}
                onPlay={() => app.playAll(app.contextTracks(contextId), contextId)}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
