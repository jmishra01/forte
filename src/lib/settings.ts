export interface Settings {
  theme: "system" | "light" | "dark";
  textSize: "small" | "medium" | "large";
  width: "narrow" | "medium" | "wide" | "full";
  editorFont: "mono" | "sans";
  /** Vault folder for daily notes. */
  dailyFolder: string;
  /** Vault folder whose notes are offered as templates. */
  templatesFolder: string;
  showOutline: boolean;
  /** AI help (summaries, link suggestions, questions) with the person's own API key. Off until turned on. */
  aiEnabled: boolean;
  /** Sync every few minutes: Git (when the vault is a repository) and encrypted sync (when set up). */
  gitAutoSync: boolean;
  /** Plugins approved on this computer: id -> SHA-256 of the code that was approved. */
  plugins: Record<string, string>;
}

export const defaults: Settings = {
  theme: "system",
  textSize: "medium",
  width: "medium",
  editorFont: "mono",
  dailyFolder: "Daily",
  templatesFolder: "Templates",
  showOutline: true,
  aiEnabled: false,
  gitAutoSync: false,
  plugins: {},
};

/** Stored settings over the defaults, ignoring unknown keys and wrong types. */
export function withDefaults(stored: Record<string, unknown> | null): Settings {
  const out = { ...defaults } as Record<string, unknown>;
  for (const [k, v] of Object.entries(stored ?? {})) {
    if (k in defaults && v !== null && typeof v === typeof (defaults as unknown as Record<string, unknown>)[k]) out[k] = v;
  }
  return out as unknown as Settings;
}

const SIZES = { small: "14px", medium: "15px", large: "17px" };
const WIDTHS = { narrow: "640px", medium: "820px", wide: "1080px", full: "none" };
const FONTS = {
  mono: 'ui-monospace, "SF Mono", Menlo, monospace',
  sans: '-apple-system, BlinkMacSystemFont, "Segoe UI", Inter, Roboto, sans-serif',
};

/** Applies appearance settings to the page through `data-theme` and CSS variables. */
export function applySettings(s: Settings) {
  const root = document.documentElement;
  if (s.theme === "system") delete root.dataset.theme;
  else root.dataset.theme = s.theme;
  root.style.setProperty("--text-size", SIZES[s.textSize]);
  root.style.setProperty("--measure", WIDTHS[s.width]);
  root.style.setProperty("--editor-font", FONTS[s.editorFont]);
}

/** Whether the page is dark, from the setting or else the system's preference. */
export function isDark(s: Settings, systemDark: boolean) {
  return s.theme === "dark" || (s.theme === "system" && systemDark);
}
