// Runs approved plugins, each in its own Web Worker. A worker can't touch the
// page, reach the network (the app's Content Security Policy applies to it) or
// call the backend; it can only send messages, and every request is checked
// here against the permissions its manifest asked for and you approved.

import { invoke } from "@tauri-apps/api/core";

export interface PluginInfo {
  id: string;
  name: string;
  description: string;
  version: string;
  permissions: Permission[];
  code: string;
  /** SHA-256 of `code`; approval is for exactly this version. */
  hash: string;
  error: string | null;
}

export type Permission = "editor" | "notes:read" | "notes:write";

export const listPlugins = () => invoke<PluginInfo[]>("list_plugins");

/** What each request needs. Requests not listed here are refused. */
const NEEDS: Record<string, Permission | null> = {
  currentNote: "editor",
  insertText: "editor",
  listNotes: "notes:read",
  readNote: "notes:read",
  writeNote: "notes:write",
  notify: null,
};

export const PERMISSION_TEXT: Record<Permission, string> = {
  editor: "See the open note and add text to it",
  "notes:read": "Read all your notes",
  "notes:write": "Change and create notes",
};

/** Whether a request from a plugin with `permissions` may go ahead; the reason if not. */
export function check(method: unknown, args: unknown, permissions: Permission[]): string | null {
  if (typeof method !== "string" || !(method in NEEDS)) return `unknown request ${String(method)}`;
  if (!Array.isArray(args) || !args.every((a) => typeof a === "string")) return "arguments must be text";
  const need = NEEDS[method];
  if (need && !permissions.includes(need)) return `needs the "${need}" permission`;
  if ((method === "readNote" || method === "writeNote") && !isNotePath(args[0] as string)) return "not a note path";
  if (method === "writeNote" && typeof args[1] !== "string") return "writeNote needs text";
  return null;
}

/** A vault-relative `.md` path with no hidden or parent folders (so not `.forte/plugins/...`). */
export function isNotePath(path: string) {
  return /\.md$/i.test(path) && path.split("/").every((part) => part && !part.startsWith(".") && part !== "..");
}

/** What the app does for plugins. */
export interface PluginHandlers {
  currentNote(): { path: string; text: string } | null;
  insertText(text: string): void | Promise<void>;
  listNotes(): string[];
  readNote(path: string): Promise<string>;
  writeNote(path: string, text: string): Promise<void>;
  notify(pluginName: string, message: string): void;
}

export interface PluginCommand {
  plugin: string;
  label: string;
  run: () => void;
}

// Runs before the plugin's code inside the worker, giving it the `forte` object.
const PRELUDE = `
const __pending = new Map();
let __next = 1;
const __commands = new Map();
const __call = (method, ...args) => new Promise((resolve, reject) => {
  const id = __next++;
  __pending.set(id, { resolve, reject });
  postMessage({ type: "call", id, method, args });
});
self.onmessage = async (e) => {
  const m = e.data;
  if (m.type === "result") {
    const p = __pending.get(m.id);
    __pending.delete(m.id);
    if (p) m.error ? p.reject(new Error(m.error)) : p.resolve(m.value);
  } else if (m.type === "run") {
    try { await __commands.get(m.command)?.(); }
    catch (err) { postMessage({ type: "error", message: String((err && err.message) || err) }); }
  }
};
const forte = Object.freeze({
  registerCommand(label, run) {
    const id = String(__commands.size + 1);
    __commands.set(id, run);
    postMessage({ type: "command", id, label: String(label) });
  },
  currentNote: () => __call("currentNote"),
  insertText: (text) => __call("insertText", String(text)),
  listNotes: () => __call("listNotes"),
  readNote: (path) => __call("readNote", String(path)),
  writeNote: (path, text) => __call("writeNote", String(path), String(text)),
  notify: (message) => __call("notify", String(message)),
});
`;

/** Starts `plugin`; returns its commands (filled in as it registers them) and a stop function. */
export function startPlugin(
  plugin: PluginInfo,
  handlers: PluginHandlers,
  onCommands: (commands: PluginCommand[]) => void,
  onError: (message: string) => void,
) {
  const source = `${PRELUDE}\n;(function () {\n"use strict";\n${plugin.code}\n})();\n`;
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  const worker = new Worker(url, { name: `plugin:${plugin.id}` });
  URL.revokeObjectURL(url);
  const commands: PluginCommand[] = [];

  worker.onerror = (e) => {
    e.preventDefault();
    onError(e.message || "the plugin failed to start");
  };
  worker.onmessage = async (e: MessageEvent) => {
    const m = e.data ?? {};
    if (m.type === "command" && typeof m.id === "string" && typeof m.label === "string") {
      commands.push({ plugin: plugin.name, label: m.label.slice(0, 80), run: () => worker.postMessage({ type: "run", command: m.id }) });
      onCommands([...commands]);
    } else if (m.type === "error") {
      onError(String(m.message).slice(0, 300));
    } else if (m.type === "call") {
      const refusal = check(m.method, m.args, plugin.permissions);
      const reply = (value: unknown, error?: string) => worker.postMessage({ type: "result", id: m.id, value, error });
      if (refusal) return reply(undefined, `Forte refused: ${refusal}`);
      const [a, b] = m.args as string[];
      const actions: Record<string, () => unknown> = {
        currentNote: () => handlers.currentNote(),
        insertText: () => handlers.insertText(a),
        listNotes: () => handlers.listNotes(),
        readNote: () => handlers.readNote(a),
        writeNote: () => handlers.writeNote(a, b),
        notify: () => handlers.notify(plugin.name, a.slice(0, 300)),
      };
      try {
        const value = await actions[m.method]();
        reply(value ?? null);
      } catch (err) {
        reply(undefined, String(err));
      }
    }
  };
  return () => worker.terminate();
}
