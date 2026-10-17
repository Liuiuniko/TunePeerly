import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useApp, type Route } from "../app/model";
import { covers, folderName, groupTracks } from "../lib/library";
import { Icons } from "../ui/icons";
import { Collage, Cover, LikedCover } from "./Cover";

type Chip = "playlists" | "artists" | "albums" | "folders" | "devices";
type Item = { key: string; title: string; sub: string; cover: ReactNode; open: () => void; active: boolean; drop?: string; menu?: string };
const MIN = 200;
const MAX = 400;
const COLLAPSED = 76;

function stored(): { width: number; collapsed: boolean } {
  try {
    const raw = JSON.parse(localStorage.getItem("sidebar") ?? "{}") as { width?: number; collapsed?: boolean };
    return { width: Math.min(MAX, Math.max(MIN, Number(raw.width) || 264)), collapsed: Boolean(raw.collapsed) };
  } catch {
    return { width: 264, collapsed: false };
  }
}

export function useSidebarWidth() {
  const [state, setState] = useState(stored);
  useEffect(() => localStorage.setItem("sidebar", JSON.stringify(state)), [state]);
  return [state, setState] as const;
}

export function Sidebar({ width, collapsed, onResize, onCollapse }: { width: number; collapsed: boolean; onResize: (width: number) => void; onCollapse: (value: boolean) => void }) {
  const { t } = useTranslation();
  const app = useApp();
  const [chip, setChip] = useState<Chip>("playlists");
  const drag = useRef<{ start: number; width: number } | null>(null);
  const route = app.route;
  const is = (name: Route["name"], id?: string) => route.name === name && (id === undefined || route.id === id);

  const items = useMemo((): Item[] => {
    const go = (next: Route) => () => app.navigate(next);
    switch (chip) {
      case "artists":
        return groupTracks(app.library, "artist")
          .filter((group) => group.name)
          .map((group) => ({ key: group.name, title: group.name, sub: t("kind.artist"), cover: <Cover src={group.cover} round icon="artist" />, open: go({ name: "artist", id: group.name }), active: is("artist", group.name) }));
      case "albums":
        return groupTracks(app.library, "album")
          .filter((group) => group.name)
          .map((group) => ({ key: group.name, title: group.name, sub: group.tracks[0]?.artist || t("kind.album"), cover: <Cover src={group.cover} />, open: go({ name: "album", id: group.name }), active: is("album", group.name) }));
      case "folders":
        return groupTracks(app.library, "folder").map((group) => ({ key: group.name, title: folderName(group.name), sub: group.name, cover: <Cover src={group.cover} icon="folder" />, open: go({ name: "folder", id: group.name }), active: is("folder", group.name) }));
      case "devices":
        return (app.boot?.links ?? []).map((link) => ({
          key: link.deviceId,
          title: link.name,
          sub: t(`link.${link.state}`),
          cover: (
            <span className="device-cover">
              <Icons.devices />
              <span className={`device-dot ${link.state}`} />
            </span>
          ),
          open: go({ name: "device", id: link.deviceId }),
          active: is("device", link.deviceId),
        }));
      default:
        return (app.boot?.playlists ?? []).map((playlist) => ({
          key: playlist.id,
          title: playlist.name,
          sub: t("sidebar.playlistSub", { count: playlist.count }),
          cover: <Collage covers={covers(app.index.members(app.members[playlist.id] ?? []))} />,
          open: go({ name: "playlist", id: playlist.id }),
          active: is("playlist", playlist.id),
          drop: `playlist:${playlist.id}`,
          menu: playlist.id,
        }));
    }
  }, [chip, app.library, app.boot, app.members, app.index, app.navigate, route, t]);

  const onHandleDown = (event: ReactPointerEvent) => {
    event.preventDefault();
    drag.current = { start: event.clientX, width: collapsed ? COLLAPSED : width };
    const move = (moveEvent: PointerEvent) => {
      if (!drag.current) return;
      const next = drag.current.width + moveEvent.clientX - drag.current.start;
      if (next < 150) onCollapse(true);
      else {
        onCollapse(false);
        onResize(Math.min(MAX, Math.max(MIN, next)));
      }
    };
    const up = () => {
      drag.current = null;
      document.body.classList.remove("resizing");
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    document.body.classList.add("resizing");
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <aside className={`sidebar${collapsed ? " collapsed" : ""}`} style={{ width: collapsed ? COLLAPSED : width }}>
      <nav className="side-card side-nav">
        <NavItem icon={is("home") ? <Icons.homeFill /> : <Icons.home />} label={t("nav.home")} active={is("home")} collapsed={collapsed} onClick={() => app.navigate({ name: "home" })} />
        <NavItem icon={<Icons.search />} label={t("nav.search")} active={is("search")} collapsed={collapsed} onClick={() => app.navigate({ name: "search" })} />
        <NavItem icon={<Icons.music />} label={t("nav.allMusic")} active={is("library")} collapsed={collapsed} onClick={() => app.navigate({ name: "library" })} />
      </nav>

      <section className="side-card side-library">
        <header className="library-head">
          <button type="button" className="library-title" onClick={() => onCollapse(!collapsed)} title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")} aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}>
            <Icons.library />
            {!collapsed && <span>{t("sidebar.library")}</span>}
          </button>
          {!collapsed && (
            <button type="button" className="icon-btn" title={t("playlist.create")} aria-label={t("playlist.create")} onClick={() => app.createPlaylist([], true)}>
              <Icons.plus />
            </button>
          )}
        </header>
        {!collapsed && (
          <div className="chip-row side-chips">
            {(["playlists", "artists", "albums", "folders", "devices"] as Chip[]).map((item) => (
              <button type="button" key={item} className={`chip small${chip === item ? " on" : ""}`} onClick={() => setChip(item)}>
                {t(`sidebar.${item}`)}
              </button>
            ))}
          </div>
        )}
        <div className="side-list">
          {chip === "playlists" && (
            <>
              <SideItem cover={<LikedCover />} title={t("nav.liked")} sub={t("sidebar.playlistSub", { count: app.liked.length })} active={is("liked")} collapsed={collapsed} drop="liked" onClick={() => app.navigate({ name: "liked" })} pinned />
              <SideItem cover={<Collage covers={covers(app.recent)} />} title={t("nav.recent")} sub={t("sidebar.playlistSub", { count: app.recent.length })} active={is("recent")} collapsed={collapsed} onClick={() => app.navigate({ name: "recent" })} />
            </>
          )}
          {items.map((item) => (
            <SideItem
              key={item.key}
              cover={item.cover}
              title={item.title}
              sub={item.sub}
              active={item.active}
              collapsed={collapsed}
              drop={item.drop}
              onClick={item.open}
              onMenu={
                item.menu
                  ? (event) => {
                      event.preventDefault();
                      const id = item.menu!;
                      app.setMenu({ x: event.clientX, y: event.clientY, tracks: app.index.members(app.members[id] ?? []), playlistMenu: id });
                    }
                  : undefined
              }
            />
          ))}
          {!collapsed && items.length === 0 && chip !== "playlists" && <p className="side-empty">{chip === "devices" ? t("sidebar.noDevices") : t("sidebar.nothing")}</p>}
          {!collapsed && chip === "playlists" && items.length === 0 && (
            <div className="side-cta">
              <div className="setting-title">{t("sidebar.firstPlaylist")}</div>
              <div className="setting-help">{t("sidebar.firstPlaylistBody")}</div>
              <button type="button" className="btn primary small" onClick={() => app.createPlaylist([], true)}>
                {t("playlist.create")}
              </button>
            </div>
          )}
        </div>
      </section>

      <nav className="side-card side-foot">
        <NavItem icon={<Icons.devices />} label={t("nav.devices")} active={is("devices") || is("device")} collapsed={collapsed} onClick={() => app.navigate({ name: "devices" })} badge={app.boot?.offer ? "!" : undefined} />
        <NavItem icon={<Icons.settings />} label={t("nav.settings")} active={is("settings") || is("problems")} collapsed={collapsed} onClick={() => app.navigate({ name: "settings" })} />
      </nav>
      <div className="resize-handle" onPointerDown={onHandleDown} onDoubleClick={() => onCollapse(!collapsed)} role="separator" aria-orientation="vertical" aria-label={t("sidebar.resize")} />
    </aside>
  );
}

function NavItem({ icon, label, active, collapsed, onClick, badge }: { icon: ReactNode; label: string; active: boolean; collapsed: boolean; onClick: () => void; badge?: string }) {
  return (
    <button type="button" className={`nav-item${active ? " active" : ""}`} onClick={onClick} title={collapsed ? label : undefined} aria-label={label} aria-current={active ? "page" : undefined}>
      {icon}
      {!collapsed && <span>{label}</span>}
      {badge && <span className="nav-badge">{badge}</span>}
    </button>
  );
}

function SideItem({ cover, title, sub, active, collapsed, drop, onClick, onMenu, pinned }: { cover: ReactNode; title: string; sub?: string; active: boolean; collapsed: boolean; drop?: string; onClick: () => void; onMenu?: (event: ReactMouseEvent) => void; pinned?: boolean }) {
  return (
    <button type="button" className={`side-item${active ? " active" : ""}${pinned ? " pinned" : ""}`} data-drop={drop} onClick={onClick} onContextMenu={onMenu} title={collapsed ? title : undefined} aria-label={title}>
      <span className="side-cover">{cover}</span>
      {!collapsed && (
        <span className="side-text">
          <span className="side-title">{title}</span>
          {sub && <span className="side-sub">{sub}</span>}
        </span>
      )}
    </button>
  );
}
