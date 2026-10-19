// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
export function clockText(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = String(whole % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

/** "1 hr 4 min" / "12 min 30 sec" style totals for page headers. */
export function totalText(ms: number, t: (key: string, options?: Record<string, unknown>) => string): string {
  const seconds = Math.round(ms / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return t("format.hoursMinutes", { hours, minutes });
  if (minutes > 0) return t("format.minutesSeconds", { minutes, seconds: seconds % 60 });
  return t("format.seconds", { seconds });
}

export function dateText(value: string, language: string): string {
  if (!value) return "";
  const parsed = new Date(value.includes("T") ? value : value.replace(" ", "T"));
  if (Number.isNaN(parsed.getTime())) return value.slice(0, 10);
  return parsed.toLocaleDateString(language === "zh" ? "zh-CN" : "en-US", { year: "numeric", month: "short", day: "numeric" });
}

const tints = ["#e07a4a", "#c99a3e", "#79a072", "#5f8fbf", "#a77ac2", "#cf6a7a", "#4fa39a", "#b9784f"];

/** A stable warm accent per name for page headers and cards. */
export function tintFor(text: string): string {
  let hash = 0;
  for (const char of text) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  return tints[Math.abs(hash) % tints.length];
}

export function greetingKey(date = new Date()): string {
  const hour = date.getHours();
  if (hour < 5) return "home.night";
  if (hour < 12) return "home.morning";
  if (hour < 18) return "home.afternoon";
  return "home.evening";
}

/** Splits a search snippet with \u0002…\u0003 markers into plain and highlighted parts. */
export function snippetParts(snippet: string): { text: string; hit: boolean }[] {
  const parts: { text: string; hit: boolean }[] = [];
  let hit = false;
  let buffer = "";
  for (const char of snippet) {
    if (char === "\u0002" || char === "\u0003") {
      if (buffer) parts.push({ text: buffer, hit });
      buffer = "";
      hit = char === "\u0002";
    } else {
      buffer += char;
    }
  }
  if (buffer) parts.push({ text: buffer, hit });
  return parts;
}
