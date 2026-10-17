import type { CSSProperties, ReactNode } from "react";

export function PageHeader({ kind, title, cover, meta, tint }: { kind?: string; title: string; cover: ReactNode; meta?: ReactNode; tint?: string }) {
  return (
    <header className="page-hero" style={tint ? ({ "--tint": tint } as CSSProperties) : undefined}>
      <div className="hero-cover">{cover}</div>
      <div className="hero-text">
        {kind && <div className="hero-kind">{kind}</div>}
        <h1 className={`hero-title${title.length > 28 ? " long" : ""}`} title={title}>
          {title}
        </h1>
        {meta && <div className="hero-meta">{meta}</div>}
      </div>
    </header>
  );
}

export function ActionBar({ children }: { children: ReactNode }) {
  return <div className="action-bar">{children}</div>;
}
