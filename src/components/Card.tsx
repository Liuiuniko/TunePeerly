import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Icons } from "../ui/icons";

export function Card({ cover, title, subtitle, onOpen, onPlay, dropId, active }: { cover: ReactNode; title: string; subtitle?: ReactNode; onOpen: () => void; onPlay?: () => void; dropId?: string; active?: boolean }) {
  const { t } = useTranslation();
  return (
    <div className={`card${active ? " active" : ""}`} data-drop={dropId} role="button" tabIndex={0} onClick={onOpen} onKeyDown={(event) => {
      if (event.key === "Enter") onOpen();
    }}>
      <div className="card-cover">
        {cover}
        {onPlay && (
          <button
            type="button"
            className="card-play"
            aria-label={t("list.play")}
            onClick={(event) => {
              event.stopPropagation();
              onPlay();
            }}
          >
            <Icons.play />
          </button>
        )}
      </div>
      <div className="card-title" title={title}>
        {title}
      </div>
      {subtitle && <div className="card-sub">{subtitle}</div>}
    </div>
  );
}

export function Shelf({ title, action, children, rail }: { title: string; action?: ReactNode; children: ReactNode; rail?: boolean }) {
  return (
    <section className="shelf">
      <div className="shelf-head">
        <h2>{title}</h2>
        {action}
      </div>
      <div className={rail ? "rail" : "card-grid"}>{children}</div>
    </section>
  );
}
