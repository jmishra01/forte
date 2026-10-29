import { invoke } from "@tauri-apps/api/core";

export interface Entry {
  name: string;
  path: string; // relative to vault root, "/"-separated
  isDir: boolean;
  children: Entry[];
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
};

export function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

export function nameOf(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1).replace(/\.md$/, "");
}
