// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
/** A typed "Connect by address" target. Without a port, the app asks the other computer which port it uses. */
export type TypedAddress = { host: string; port?: number };

function ipv4(text: string): boolean {
  const parts = text.split(".");
  return parts.length === 4 && parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255);
}

/** Reads `192.168.1.20` or, for advanced use, `192.168.1.20:41220`. Anything else is null. */
export function parseAddress(text: string): TypedAddress | null {
  const value = text.trim();
  const match = /^([^:\s]+)(?::(\d{1,5}))?$/.exec(value);
  if (!match || !ipv4(match[1])) return null;
  const host = match[1].split(".").map((part) => String(Number(part))).join(".");
  if (match[2] === undefined) return { host };
  const port = Number(match[2]);
  return port >= 1 && port <= 65535 ? { host, port } : null;
}

/** Saved recent targets are `host:port`; the list shows each computer's address once, newest first. */
export function recentHosts(saved: string[]): string[] {
  const hosts: string[] = [];
  for (const item of saved) {
    const host = parseAddress(item)?.host;
    if (host && !hosts.includes(host)) hosts.push(host);
  }
  return hosts;
}
