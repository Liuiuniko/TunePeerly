import { useState } from "react";
import { api } from "../api";
import { Icons } from "../ui/icons";

export function coverSrc(path: string): string {
  return /^(data:|blob:|https?:)/.test(path) ? path : api.fileUrl(path);
}

type CoverProps = { src?: string | null; className?: string; round?: boolean; icon?: "music" | "artist" | "folder" | "device" };

export function Cover({ src, className = "", round, icon = "music" }: CoverProps) {
  const [broken, setBroken] = useState<string | null>(null);
  const shape = `cover ${round ? "round " : ""}${className}`;
  if (src && broken !== src) {
    return (
      <div className={shape}>
        <img src={coverSrc(src)} alt="" loading="lazy" draggable={false} onError={() => setBroken(src)} />
      </div>
    );
  }
  const Glyph = icon === "artist" ? Icons.artist : icon === "folder" ? Icons.folder : icon === "device" ? Icons.devices : Icons.music;
  return (
    <div className={`${shape} placeholder`}>
      <Glyph />
    </div>
  );
}

/** Playlist cover: the first four covers as a 2x2 grid, or one cover, or a placeholder. */
export function Collage({ covers, className = "" }: { covers: string[]; className?: string }) {
  if (covers.length >= 4) {
    return (
      <div className={`cover collage ${className}`}>
        {covers.slice(0, 4).map((item) => (
          <img key={item} src={coverSrc(item)} alt="" loading="lazy" draggable={false} />
        ))}
      </div>
    );
  }
  return <Cover src={covers[0]} className={className} />;
}

export function LikedCover({ className = "" }: { className?: string }) {
  return (
    <div className={`cover liked-cover ${className}`}>
      <Icons.heartFill />
    </div>
  );
}
