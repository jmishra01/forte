import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Entry {
  name: string;
  path: string; // relative to vault root, "/"-separated
  isDir: boolean;
  children: Entry[];
}

export interface Hit {
  path: string;
  line: number; // 1-based
  text: string;
}

export interface NoteInfo {
  path: string;
  links: { target: string; line: number; text: string }[];
  /** Lowercase, without "#". */
  tags: string[];
}

export interface TrashItem {
  name: string;
  isDir: boolean;
  original: string | null;
  /** Seconds since the epoch. */
  deletedAt: number | null;
}

export interface Conflict {
  path: string;
  kind: "copy" | "markers";
  original: string | null;
}

export interface GitStatus {
  isRepo: boolean;
  branch: string | null;
  hasUpstream: boolean;
  changes: number;
  ahead: number;
  behind: number;
  conflicted: string[];
  merging: boolean;
}

export type AiEvent =
  | { type: "sources"; paths: string[] }
  | { type: "text"; text: string }
  | { type: "done" }
  | { type: "error"; message: string };

export type AiTask = "summarize" | "links" | "ask";

export const api = {
  getVault: () => invoke<string>("get_vault"),
  setVault: (path: string) => invoke<void>("set_vault", { path }),
  listTree: () => invoke<Entry[]>("list_tree"),
  readNote: (path: string) => invoke<string>("read_note", { path }),
  writeNote: (path: string, content: string) => invoke<void>("write_note", { path, content }),
  createNote: (dir: string, name: string) => invoke<string>("create_note", { dir, name }),
  createFolder: (dir: string, name: string) => invoke<string>("create_folder", { dir, name }),
  rename: (path: string, name: string) => invoke<string>("rename_entry", { path, name }),
  move: (path: string, dir: string) => invoke<string>("move_entry", { path, dir }),
  remove: (path: string) => invoke<void>("delete_entry", { path }),
  search: (query: string) => invoke<Hit[]>("search_notes", { query }),
  noteIndex: () => invoke<NoteInfo[]>("note_index"),
  getSettings: () => invoke<Record<string, unknown> | null>("get_settings"),
  setSettings: (settings: object) => invoke<void>("set_settings", { settings }),
  /** Asks where to save and writes the page; resolves to the saved path, or null if cancelled. */
  exportHtml: (name: string, html: string) => invoke<string | null>("export_html", { name, html }),
  /** Stores `data` under `attachments/` and returns its vault-relative path. */
  saveAttachment: async (name: string, data: Blob) =>
    invoke<string>("save_attachment", new Uint8Array(await data.arrayBuffer()), {
      headers: { "x-name": encodeURIComponent(name) },
    }),
  listTrash: () => invoke<TrashItem[]>("list_trash"),
  restoreFromTrash: (name: string) => invoke<string>("restore_from_trash", { name }),
  deleteFromTrash: (name: string) => invoke<void>("delete_from_trash", { name }),
  emptyTrash: () => invoke<void>("empty_trash"),
  findConflicts: () => invoke<Conflict[]>("find_conflicts"),
  /** Keep "original", "copy" or "both"; resolves to the note to show next. */
  settleConflict: (path: string, keep: "original" | "copy" | "both") => invoke<string>("settle_conflict", { path, keep }),
  gitStatus: () => invoke<GitStatus>("git_status"),
  gitSync: (message: string) => invoke<GitStatus>("git_sync", { message }),
  gitInit: () => invoke<GitStatus>("git_init"),
  aiHasKey: () => invoke<boolean>("ai_has_key"),
  /** Stores the key in the OS keychain; it is never read back into the page. */
  aiSetKey: (key: string) => invoke<void>("ai_set_key", { key }),
  aiClearKey: () => invoke<void>("ai_clear_key"),
  aiStop: () => invoke<void>("ai_stop"),
  aiRun: (task: AiTask, onEvent: (e: AiEvent) => void, path?: string, question?: string) => {
    const events = new Channel<AiEvent>();
    events.onmessage = onEvent;
    return invoke<void>("ai_run", { task, path, question, events });
  },
  /** Calls `fn` with the paths that changed on disk, including our own writes. Resolves to an unsubscribe function. */
  onVaultChanged: (fn: (paths: string[]) => void) => listen<string[]>("vault-changed", (e) => fn(e.payload)),
};

export function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

export function nameOf(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1).replace(/\.md$/, "");
}
