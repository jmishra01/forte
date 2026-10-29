// Which notes were open, per vault, so the app reopens where you left off.
// Kept in the webview's local storage: losing it only means starting fresh.

export interface Session {
  tabs: string[];
  current: string | null;
}

const key = (vault: string) => `forte.session:${vault}`;

export function loadSession(vault: string, notes: string[]): Session | null {
  try {
    const raw = JSON.parse(localStorage.getItem(key(vault)) ?? "null") as Session | null;
    if (!raw || !Array.isArray(raw.tabs)) return null;
    // Notes deleted or moved since are dropped.
    const tabs = raw.tabs.filter((t) => typeof t === "string" && notes.includes(t));
    const current = raw.current && tabs.includes(raw.current) ? raw.current : tabs[0] ?? null;
    return tabs.length ? { tabs, current } : null;
  } catch {
    return null;
  }
}

export function saveSession(vault: string, session: Session) {
  try {
    localStorage.setItem(key(vault), JSON.stringify(session));
  } catch {
    // Storage full or unavailable; the next start just opens nothing.
  }
}
