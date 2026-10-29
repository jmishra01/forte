import { invoke } from "@tauri-apps/api/core";
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

export interface NoteLinks {
  path: string;
  links: { target: string; line: number; text: string }[];
}

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
  linkIndex: () => invoke<NoteLinks[]>("link_index"),
  /** Stores `data` under `attachments/` and returns its vault-relative path. */
  saveAttachment: async (name: string, data: Blob) =>
    invoke<string>("save_attachment", new Uint8Array(await data.arrayBuffer()), {
      headers: { "x-name": encodeURIComponent(name) },
    }),
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
