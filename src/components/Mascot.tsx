export type MascotMood = "idle" | "playing" | "paused" | "error";

/** A small vinyl creature. It only appears on Now Playing and the empty library. */
export function Mascot({ mood, size = 132 }: { mood: MascotMood; size?: number }) {
  return (
    <div className={`mascot mood-${mood}`} style={{ width: size, height: size }} aria-hidden="true">
      <svg viewBox="0 0 160 160">
        <ellipse className="mascot-shadow" cx="80" cy="148" rx="42" ry="6" />
        <g className="mascot-body">
          <circle cx="80" cy="80" r="58" className="mascot-disc" />
          <circle cx="80" cy="80" r="46" className="mascot-groove" />
          <circle cx="80" cy="80" r="35" className="mascot-groove" />
          <circle cx="80" cy="80" r="24" className="mascot-label" />
          <g className="mascot-face">
            {mood === "error" ? (
              <>
                <path d="M66 72l8 8M74 72l-8 8M86 72l8 8M94 72l-8 8" className="mascot-line" />
                <path d="M70 95c6-5 14-5 20 0" className="mascot-line" />
              </>
            ) : mood === "paused" ? (
              <>
                <path d="M65 77h10M85 77h10" className="mascot-line" />
                <path d="M72 92c5 3 11 3 16 0" className="mascot-line" />
              </>
            ) : (
              <>
                <ellipse cx="70" cy="76" rx="4" ry="5" className="mascot-eye" />
                <ellipse cx="90" cy="76" rx="4" ry="5" className="mascot-eye" />
                <path d={mood === "playing" ? "M70 89c5 7 15 7 20 0" : "M72 90c5 4 11 4 16 0"} className="mascot-line" />
              </>
            )}
            <circle cx="62" cy="88" r="4" className="mascot-cheek" />
            <circle cx="98" cy="88" r="4" className="mascot-cheek" />
          </g>
        </g>
        <g className="mascot-notes">
          <path d="M128 40v18a6 6 0 1 1-3-5V36l12-4v6z" />
          <path d="M28 52v14a5 5 0 1 1-2.6-4.4V48l9-3v5z" />
        </g>
        {mood === "paused" && (
          <g className="mascot-zzz">
            <text x="118" y="44">z</text>
            <text x="130" y="30">z</text>
          </g>
        )}
      </svg>
    </div>
  );
}
