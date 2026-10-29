<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, dirOf, nameOf, type Conflict, type Entry, type GitStatus, type NoteInfo } from "./lib/api";
  import { relativeHref, renderMarkdown, resolveLink, resolveRelative } from "./lib/markdown";
  import { backlinks, linkFor, rewriteLinks } from "./lib/links";
  import Tree from "./lib/Tree.svelte";
  import Palette, { type Command } from "./lib/Palette.svelte";
  import Graph from "./lib/Graph.svelte";
  import SettingsDialog from "./lib/Settings.svelte";
  import { headings, type Heading } from "./lib/outline";
  import { applySettings, defaults, isDark, withDefaults, type Settings } from "./lib/settings";
  import { expandTemplate, isoDate } from "./lib/templates";
  import { exportPage } from "./lib/exportPage";
  import Trash from "./lib/Trash.svelte";
  import Conflicts from "./lib/Conflicts.svelte";
  import AiPanel from "./lib/AiPanel.svelte";
  import { hasMarkers, resolveMarkers, type Side } from "./lib/mergeMarkers";
  import { findUpdate, installUpdate } from "./lib/updates";
  import type { Update } from "@tauri-apps/plugin-updater";
  import Editor, { type LinkOption } from "./lib/Editor.svelte";
  import { renderDiagrams } from "./lib/diagrams";
  import { highlightCodeBlocks } from "./lib/codeHighlight";

  let vault = "";
  let tree: Entry[] = [];
  let expanded = new Set<string>();
  let current: string | null = null;
  let title = "";
  let content = "";
  let mode: "edit" | "preview" = "edit";
  let renaming: string | null = null;
  let filter = "";
  let toast: { text: string; kind: "error" | "info" } | null = null;
  let toastTimer: ReturnType<typeof setTimeout>;
  let paletteOpen = false;
  let paletteInitial = "";
  let settingsOpen = false;
  let graphOpen = false;
  let focusMode = false;
  let tagsOpen = true;
  /** When set, the sidebar lists notes with this tag (or a tag nested under it). */
  let tagFilter: string | null = null;
  let settings: Settings = defaults;
  let trashOpen = false;
  let conflictsOpen = false;
  let conflicts: Conflict[] = [];
  let git: GitStatus | null = null;
  let syncing = false;
  let aiOpen = false;
  let aiHasKey = false;
  let update: Update | null = null;
  let installing = false;
  /** On narrow (phone) screens the note list and the note take turns; this shows the list. */
  let showList = false;
  let systemDark = matchMedia("(prefers-color-scheme: dark)").matches;
  let index: NoteInfo[] = [];
  /** Notes visited, for back/forward; `place` is the current position in it. */
  let history: string[] = [];
  let place = -1;
  let rootDrop = false;
  let editor: Editor | undefined;
  let previewEl: HTMLElement;
  let titleInput: HTMLInputElement;

  $: notes = flatten(tree);
  $: filtered = filter.trim()
    ? notes.filter((p) => nameOf(p).toLowerCase().includes(filter.trim().toLowerCase()))
    : [];
  $: dark = isDark(settings, systemDark);
  $: html =
    current && mode === "preview"
      ? renderMarkdown(content, (t: string) => resolveLink(t, current!, notes), imageSrc)
      : "";
  $: outline = current ? headings(content) : [];
  $: tagCounts = countTags(index);
  $: tagged = tagFilter ? taggedNotes(tagFilter, index) : [];
  $: templates = notes.filter((p) => settings.templatesFolder && p.startsWith(settings.templatesFolder + "/"));
  $: commands = buildCommands(current, mode, templates, settings, focusMode, git, update);
  $: markers = !!current && hasMarkers(content);
  $: incoming = current ? backlinks(current, index, notes) : [];
  $: canBack = history.slice(0, place).some((p) => notes.includes(p));
  $: canForward = history.slice(place + 1).some((p) => notes.includes(p));

  /** Relative images point at files in the vault, served over Tauri's asset protocol. */
  function imageSrc(href: string) {
    const rel = current && resolveRelative(href, current);
    return rel ? convertFileSrc(`${vault}/${rel}`) : href;
  }
  $: if (html)
    tick().then(() => {
      if (!previewEl) return;
      renderDiagrams(previewEl, dark);
      highlightCodeBlocks(previewEl);
    });

  function flatten(entries: Entry[]): string[] {
    return entries.flatMap((e) => (e.isDir ? flatten(e.children) : [e.path]));
  }

  function flash(text: string, kind: "error" | "info" = "error") {
    toast = { text, kind };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 4000);
  }

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      flash(String(e));
    }
  }

  async function refresh() {
    [tree, index, conflicts] = await Promise.all([
      run(api.listTree).then((t) => t ?? []),
      api.noteIndex().catch(() => index),
      api.findConflicts().catch(() => conflicts),
    ]);
    refreshGit();
  }

  // Git status runs a process; on a burst of changes, ask once it settles.
  let gitTimer: ReturnType<typeof setTimeout>;
  function refreshGit() {
    clearTimeout(gitTimer);
    gitTimer = setTimeout(async () => (git = await api.gitStatus().catch(() => null)), 1000);
  }

  // --- saving ---------------------------------------------------------------

  let saveTimer: ReturnType<typeof setTimeout>;
  let pending: { path: string; content: string } | null = null;
  /** What we believe is on disk for `current`: the text last read or written. */
  let saved = "";
  /** Set while asking how to resolve an outside edit; saving waits until it's answered. */
  let resolving = false;

  function scheduleSave() {
    if (!current) return;
    pending = { path: current, content };
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 400);
  }

  async function flush() {
    clearTimeout(saveTimer);
    if (!pending || resolving) return;
    const { path, content } = pending;
    pending = null;
    if (path === current) saved = content;
    await run(() => api.writeNote(path, content));
  }

  // --- changes made outside the app ------------------------------------------

  async function onDiskChange() {
    await refresh();
    const path = current;
    if (!path || resolving) return;
    let disk: string;
    try {
      disk = await api.readNote(path);
    } catch {
      // Moved, renamed or deleted elsewhere. Unsaved edits stay open; the next save recreates the file.
      if (path === current && content === saved) {
        current = null;
        flash(`"${nameOf(path)}" was moved or deleted outside Forte`);
      }
      return;
    }
    if (path !== current || disk === saved) return; // our own save, or nothing new
    if (content === saved) {
      content = saved = disk; // no local edits: just take the new version
      return;
    }
    resolving = true;
    let loadDisk = false; // if the dialog fails, keep what the user typed
    try {
      loadDisk = await ask(
        `"${nameOf(path)}" was changed outside Forte while you were editing it.\n\nLoad the version on disk? Your unsaved changes will be lost.`,
        { title: "Note changed on disk", kind: "warning", okLabel: "Load from disk", cancelLabel: "Keep mine" },
      );
    } catch {
    } finally {
      resolving = false;
    }
    if (path !== current) return;
    saved = disk;
    if (loadDisk) {
      clearTimeout(saveTimer);
      pending = null;
      content = disk;
    } else {
      scheduleSave();
    }
  }

  // --- notes ----------------------------------------------------------------

  function expandTo(path: string) {
    let dir = dirOf(path);
    while (dir) {
      expanded.add(dir);
      dir = dirOf(dir);
    }
    expanded = expanded;
  }

  /**
   * Opens `path`. With `line` (1-based), opens it for editing with the caret on that line.
   * `fromHistory` is set by back/forward, which move through history instead of adding to it.
   */
  async function openNote(path: string, forceMode?: "edit" | "preview", line?: number, fromHistory = false) {
    await flush();
    const text = await run(() => api.readNote(path));
    if (text === undefined) return false;
    current = path;
    title = nameOf(path);
    content = text;
    saved = text;
    mode = line ? "edit" : forceMode ?? (text.trim() ? "preview" : "edit");
    expandTo(path);
    showList = false;
    if (!fromHistory && history[place] !== path) {
      history = [...history.slice(0, place + 1), path];
      place = history.length - 1;
    }
    if (line) {
      await tick();
      editor?.goToLine(line);
    }
    return true;
  }

  async function go(step: -1 | 1) {
    for (let i = place + step; i >= 0 && i < history.length; i += step) {
      if (!notes.includes(history[i])) continue; // deleted since
      if (await openNote(history[i], undefined, undefined, true)) place = i;
      return;
    }
  }

  async function newNote(dir = "") {
    const path = await run(() => api.createNote(dir, "Untitled"));
    if (!path) return;
    await refresh();
    await openNote(path, "edit");
    await tick();
    titleInput?.select();
  }

  async function newFolder(dir = "") {
    const path = await run(() => api.createFolder(dir, "New folder"));
    if (!path) return;
    if (dir) expanded.add(dir);
    expanded = expanded;
    await refresh();
    renaming = path;
  }

  /** Where `path` is after `from` was renamed or moved to `to`. */
  function remap(path: string, from: string, to: string) {
    if (path === from) return to;
    if (path.startsWith(from + "/")) return to + path.slice(from.length);
    return path;
  }

  /** Keep `current` and history pointing at the same notes after `from` was renamed or moved to `to`. */
  function repoint(from: string, to: string) {
    history = history.map((p) => remap(p, from, to));
    if (!current || remap(current, from, to) === current) return;
    current = remap(current, from, to);
    if (pending) pending.path = current;
    title = nameOf(current);
    expandTo(current);
  }

  /**
   * Renames or moves `path` with `change`, then fixes `[[links]]` in other notes
   * that would no longer find their target. Returns the new path.
   */
  async function relocate(path: string, change: () => Promise<string>) {
    await flush();
    const oldNotes = notes;
    const oldIndex = await api.noteIndex().catch(() => index);
    const to = await run(change);
    if (!to) return;
    repoint(path, to);
    await refresh();

    const moved = new Map(oldNotes.map((n) => [n, remap(n, path, to)] as const).filter(([a, b]) => a !== b));
    const texts = new Map<string, string>();
    for (const note of oldIndex) {
      const newPath = moved.get(note.path) ?? note.path;
      const text = newPath === current ? content : await api.readNote(newPath).catch(() => undefined);
      if (text !== undefined) texts.set(note.path, text);
    }
    const updates = rewriteLinks(moved, oldNotes, notes, texts);
    for (const [notePath, text] of updates) {
      if (notePath === current) content = saved = text;
      await run(() => api.writeNote(notePath, text));
    }
    if (updates.size) {
      flash(`Updated links in ${updates.size} note${updates.size === 1 ? "" : "s"}`, "info");
      await refresh();
    }
    return to;
  }

  async function renameEntry(path: string, name: string) {
    renaming = null;
    if (!name.trim() || name.trim() === nameOf(path)) return;
    const to = await relocate(path, () => api.rename(path, name));
    if (to && expanded.delete(path)) expanded = expanded.add(to);
  }

  async function renameCurrent() {
    if (current && title.trim() !== nameOf(current)) await renameEntry(current, title);
    if (current) title = nameOf(current);
  }

  async function moveEntry(path: string, dir: string) {
    if (dirOf(path) === dir) return;
    const to = await relocate(path, () => api.move(path, dir));
    if (to && dir) expanded = expanded.add(dir);
  }

  async function deleteEntry(path: string) {
    const ok = await ask(`Delete "${nameOf(path)}"?\n\nIt will be moved to the .trash folder inside your notes folder.`, {
      title: "Delete",
      kind: "warning",
    });
    if (!ok) return;
    await flush();
    await run(() => api.remove(path));
    if (current && (current === path || current.startsWith(path + "/"))) current = null;
    await refresh();
  }

  async function chooseVault() {
    const dir = await openDialog({ directory: true, defaultPath: vault });
    if (typeof dir !== "string") return;
    await flush();
    await run(() => api.setVault(dir));
    vault = dir;
    current = null;
    history = [];
    place = -1;
    expanded = new Set();
    await refresh();
  }

  // --- links ----------------------------------------------------------------

  async function followLink(target: string) {
    if (!current) return;
    const found = resolveLink(target, current, notes);
    if (found) return openNote(found);
    // Link to a note that doesn't exist yet: create it.
    const clean = target.split("#")[0].replace(/\.md$/i, "").trim();
    const dir = clean.includes("/") ? dirOf(clean) : dirOf(current);
    const path = await run(() => api.createNote(dir, nameOf(clean)));
    if (!path) return;
    await refresh();
    await openNote(path, "edit");
  }

  /** Switches to editing, with the caret at character `at` of the note if given. */
  async function startEditing(at?: number) {
    mode = "edit";
    await tick();
    if (at === undefined) editor?.focus();
    else editor?.setCursor(at);
  }

  /**
   * Where in the note's text a click in the preview landed. Each preview block
   * knows its first source line; within it, the text just before the click is
   * looked up in the source to find the exact spot.
   */
  function sourcePosition(e: MouseEvent): number | undefined {
    const block = (e.target as HTMLElement).closest<HTMLElement>("[data-line]");
    if (!block) return undefined;
    const lines = content.split("\n");
    const line = Math.min(Number(block.dataset.line), lines.length);
    const lineStart = lines.slice(0, line - 1).join("\n").length + (line > 1 ? 1 : 0);
    const range = document.caretRangeFromPoint?.(e.clientX, e.clientY);
    const node = range?.startContainer;
    if (!range || node?.nodeType !== Node.TEXT_NODE || !block.contains(node)) return lineStart;
    const text = node.textContent ?? "";
    const before = text.slice(Math.max(0, range.startOffset - 16), range.startOffset);
    const after = text.slice(range.startOffset, range.startOffset + 16);
    // Rendered text drops markdown syntax, so search the source from this block onwards.
    if (before.trim()) {
      const i = content.indexOf(before, lineStart);
      if (i !== -1) return i + before.length;
    }
    if (after.trim()) {
      const i = content.indexOf(after, lineStart);
      if (i !== -1) return i;
    }
    return lineStart;
  }

  function stopEditing() {
    flush();
    if (content.trim()) mode = "preview";
  }

  function onPreviewClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) {
      // Clicking the text (but not selecting it) starts editing there.
      if (!window.getSelection()?.toString()) startEditing(sourcePosition(e));
      return;
    }
    e.preventDefault();
    const tag = a.getAttribute("data-tag");
    if (tag) return showTag(tag);
    const wiki = a.getAttribute("data-wikilink");
    const href = a.getAttribute("href") ?? "";
    if (wiki) followLink(wiki);
    else if (/^(https?|mailto):/i.test(href)) openUrl(href);
    else if (/\.md(#.*)?$/i.test(href)) followLink(decodeURIComponent(href));
  }

  // --- keyboard -------------------------------------------------------------

  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === "Escape" && graphOpen) {
      graphOpen = false;
      return;
    }
    if (mod && (e.key === "k" || (e.shiftKey && e.key.toLowerCase() === "f"))) {
      e.preventDefault();
      openPalette("");
    } else if (mod && e.shiftKey && e.key.toLowerCase() === "p") {
      e.preventDefault();
      openPalette(">");
    } else if (mod && e.shiftKey && e.key.toLowerCase() === "d") {
      e.preventDefault();
      openToday();
    } else if (mod && e.key === ",") {
      e.preventDefault();
      settingsOpen = true;
    } else if (mod && e.key === ".") {
      e.preventDefault();
      focusMode = !focusMode;
    } else if (mod && (e.key === "[" || e.key === "]")) {
      e.preventDefault();
      go(e.key === "[" ? -1 : 1);
    } else if (mod && e.key === "n") {
      e.preventDefault();
      newNote(current ? dirOf(current) : "");
    } else if (mod && e.key === "e" && current) {
      e.preventDefault();
      mode === "edit" ? stopEditing() : startEditing();
    } else if (mod && e.key === "s") {
      e.preventDefault();
      flush();
    }
  }

  // --- note picker: typing "/" or "[[" offers notes to link -----------------

  function linkOptions(trigger: string, query: string): LinkOption[] {
    const q = query.trim().toLowerCase();
    const starts = (p: string) => (nameOf(p).toLowerCase().startsWith(q) ? 0 : 1);
    const items: LinkOption[] = notes
      .filter((p) => p !== current && p.toLowerCase().replace(/\.md$/, "").includes(q))
      .sort((a, b) => starts(a) - starts(b) || nameOf(a).localeCompare(nameOf(b)))
      .slice(0, 8)
      .map((p) => ({ label: nameOf(p), detail: dirOf(p), link: linkFor(p, notes) }));
    // "[[" is clearly a link, so offer to link a note that doesn't exist yet.
    // A plain "/" is too common in normal text (paths, dates) to do that.
    if (trigger === "[[" && q && !items.some((i) => i.label.toLowerCase() === q)) {
      items.push({ label: `New note "${query.trim()}"`, detail: "", link: query.trim() });
    }
    return items;
  }

  const unlisten = api.onVaultChanged(onDiskChange);
  onDestroy(() => unlisten.then((stop) => stop()));

  // --- images pasted or dropped into the editor -----------------------------

  /** Local time as "2026-10-29 14-05-09", safe in file names. */
  function timestamp() {
    const d = new Date();
    const two = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}-${two(d.getMinutes())}-${two(d.getSeconds())}`;
  }

  async function insertImages(files: File[]) {
    const note = current;
    if (!note || !editor) return;
    const at = editor.cursor();
    let text = "";
    for (const file of files) {
      // Clipboard images are all called "image.png"; give them a findable name.
      const name =
        !file.name || file.name === "image.png"
          ? `Pasted image ${timestamp()}.${file.type.split("/")[1] || "png"}`
          : file.name;
      const rel = await run(() => api.saveAttachment(name, file));
      if (rel) text += `![](${relativeHref(rel, note)})\n`;
    }
    if (text && current === note) editor?.insert(text, at);
  }

  // --- tags -----------------------------------------------------------------

  function countTags(index: NoteInfo[]) {
    const counts = new Map<string, number>();
    for (const n of index) for (const t of n.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].sort((a, b) => a[0].localeCompare(b[0]));
  }

  /** Notes tagged `tag` or a tag nested under it (`project` matches `project/forte`). */
  function taggedNotes(tag: string, index: NoteInfo[]) {
    return index.filter((n) => n.tags.some((t) => t === tag || t.startsWith(tag + "/"))).map((n) => n.path);
  }

  function showTag(tag: string) {
    filter = "";
    tagFilter = tag;
    focusMode = false;
  }

  // --- outline --------------------------------------------------------------

  function jumpTo(h: Heading) {
    if (mode === "edit") return editor?.goToLine(h.line);
    // The nth preview heading with the same text as the nth outline entry with that text.
    const nth = outline.filter((o) => o.text === h.text).indexOf(h);
    const els = Array.from(previewEl?.querySelectorAll<HTMLElement>("h1, h2, h3, h4, h5, h6") ?? []);
    els.filter((el) => el.textContent?.trim() === h.text)[nth]?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // --- daily notes and templates ---------------------------------------------

  /** Template text by note path, or "" if it can't be read. */
  async function templateText(path: string) {
    return (await api.readNote(path).catch(() => "")) ?? "";
  }

  async function openToday() {
    const name = isoDate();
    const dir = settings.dailyFolder;
    const path = (dir ? dir + "/" : "") + name + ".md";
    if (notes.includes(path)) return openNote(path);
    const created = await run(() => api.createNote(dir, name));
    if (!created) return;
    const tpl = templates.find((t) => nameOf(t).toLowerCase() === "daily");
    const text = tpl ? expandTemplate(await templateText(tpl), name) : `# ${name}\n\n`;
    await run(() => api.writeNote(created, text));
    await refresh();
    await openNote(created, "edit");
    await tick();
    editor?.goToLine(Number.MAX_SAFE_INTEGER); // start writing below the template
  }

  async function newFromTemplate(tpl: string) {
    const dir = current && !current.startsWith(settings.templatesFolder + "/") ? dirOf(current) : "";
    const path = await run(() => api.createNote(dir, "Untitled"));
    if (!path) return;
    await run(async () => api.writeNote(path, expandTemplate(await templateText(tpl), nameOf(path))));
    await refresh();
    await openNote(path, "edit");
    await tick();
    titleInput?.select();
  }

  async function insertTemplate(tpl: string) {
    if (!current) return;
    const text = expandTemplate(await templateText(tpl), nameOf(current));
    if (mode !== "edit") await startEditing();
    await tick();
    editor?.insert(text);
  }

  // --- export -----------------------------------------------------------------

  async function exportHtml() {
    if (!current) return;
    await flush();
    const note = current;
    const body = renderMarkdown(
      content,
      (t: string) => resolveLink(t, note, notes),
      (href) => {
        const rel = resolveRelative(href, note);
        return rel ? "vault:" + rel.split("/").map(encodeURIComponent).join("/") : href;
      },
    );
    // Diagrams render only in the document, so lay the page out off-screen first.
    const scratch = document.createElement("div");
    scratch.style.cssText = "position: fixed; left: -10000px; top: 0; width: 800px;";
    scratch.innerHTML = body;
    document.body.appendChild(scratch);
    try {
      await renderDiagrams(scratch, false);
      await highlightCodeBlocks(scratch);
      // Links to other notes can't work outside the app; keep their text.
      scratch.querySelectorAll("a.wikilink, a.tag").forEach((a) => a.replaceWith(...a.childNodes));
      const saved = await run(() => api.exportHtml(nameOf(note), exportPage(nameOf(note), scratch.innerHTML)));
      if (saved) flash(`Exported to ${saved}`, "info");
    } finally {
      scratch.remove();
    }
  }

  async function printNote() {
    if (!current) return;
    await flush();
    mode = "preview";
    await tick();
    if (previewEl) await renderDiagrams(previewEl, false);
    window.print();
  }

  // --- commands and settings -------------------------------------------------

  function openPalette(initial: string) {
    paletteInitial = initial;
    paletteOpen = true;
  }

  function buildCommands(
    current: string | null,
    mode: string,
    templates: string[],
    settings: Settings,
    focusMode: boolean,
    git: GitStatus | null,
    update: Update | null,
  ): Command[] {
    const list: Command[] = [
      { label: "New note", hint: "⌘N", run: () => newNote(current ? dirOf(current) : "") },
      { label: "Open today's note", hint: "⌘⇧D", run: openToday },
      { label: "Show graph of linked notes", run: () => (graphOpen = true) },
      { label: focusMode ? "Leave focus mode" : "Focus mode", hint: "⌘.", run: () => (focusMode = !focusMode) },
      {
        label: settings.showOutline ? "Hide outline" : "Show outline",
        run: () => updateSettings({ ...settings, showOutline: !settings.showOutline }),
      },
      { label: "Settings", hint: "⌘,", run: () => (settingsOpen = true) },
      { label: "Show trash", run: () => (trashOpen = true) },
      { label: "Show sync conflicts", run: () => (conflictsOpen = true) },
      git?.isRepo
        ? { label: "Sync notes with Git", run: syncNow }
        : { label: "Keep a Git history of these notes", run: startGit },
      { label: update ? `Install update ${update.version}` : "Check for updates", run: () => checkForUpdate(true) },
      ...(settings.aiEnabled ? [{ label: "AI help", run: () => (aiOpen = true) }] : []),
      ...templates.map((t) => ({ label: `New note from template: ${nameOf(t)}`, run: () => newFromTemplate(t) })),
    ];
    if (current) {
      list.push(
        { label: "Export note as web page (HTML)…", run: exportHtml },
        { label: "Print or save note as PDF…", run: printNote },
        ...templates.map((t) => ({ label: `Insert template: ${nameOf(t)}`, run: () => insertTemplate(t) })),
      );
    }
    if (!templates.length) {
      list.push({
        label: `Create a templates folder ("${settings.templatesFolder || "Templates"}")`,
        run: async () => {
          const dir = settings.templatesFolder || "Templates";
          if (!settings.templatesFolder) updateSettings({ ...settings, templatesFolder: dir });
          const path = await run(() => api.createNote(dir, "Daily"));
          if (!path) return;
          await run(() => api.writeNote(path, "# {{date}}\n\n## Notes\n\n## To do\n\n- [ ] \n"));
          await refresh();
          await openNote(path, "edit");
        },
      });
    }
    return list;
  }

  let settingsTimer: ReturnType<typeof setTimeout>;
  function updateSettings(next: Settings) {
    settings = next;
    applySettings(settings);
    clearTimeout(settingsTimer);
    settingsTimer = setTimeout(() => run(() => api.setSettings(settings)), 300);
  }

  // --- sync ------------------------------------------------------------------

  async function syncNow() {
    if (syncing) return;
    syncing = true;
    await flush();
    const when = new Date().toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
    const result = await run(() => api.gitSync(`Notes from Forte, ${when}`));
    syncing = false;
    if (!result) return;
    git = result;
    await refresh();
    if (result.conflicted.length) {
      flash(`${result.conflicted.length} note${result.conflicted.length === 1 ? "" : "s"} changed in both places. Fix them, then sync again.`);
      conflictsOpen = true;
    } else {
      flash("Notes synced", "info");
    }
  }

  function syncTitle(g: GitStatus) {
    const parts = [
      g.changes && `${g.changes} changed`,
      g.ahead && `${g.ahead} to send`,
      g.behind && `${g.behind} to receive`,
      !g.hasUpstream && "no remote set up",
    ].filter(Boolean);
    return `Sync with Git (${g.branch ?? "no branch"})${parts.length ? ": " + parts.join(", ") : ""}`;
  }

  async function startGit() {
    const result = await run(api.gitInit);
    if (!result) return;
    git = result;
    flash("Git history started. Add a remote to sync between computers.", "info");
  }

  function keepSide(side: Side) {
    content = resolveMarkers(content, side);
    scheduleSave();
  }

  let autoSync: ReturnType<typeof setInterval> | undefined;
  $: {
    clearInterval(autoSync);
    if (settings.gitAutoSync && git?.isRepo) {
      autoSync = setInterval(() => {
        if (!syncing && !git?.conflicted.length) syncNow();
      }, 5 * 60_000);
    }
  }
  onDestroy(() => clearInterval(autoSync));

  // --- updates ---------------------------------------------------------------

  async function checkForUpdate(manual = false) {
    if (manual && update) return applyUpdate();
    update = await findUpdate();
    if (manual && !update) flash("Forte is up to date", "info");
  }

  async function applyUpdate() {
    if (!update || installing) return;
    installing = true;
    await flush();
    await run(() => installUpdate(update!));
    installing = false;
  }

  const darkQuery = matchMedia("(prefers-color-scheme: dark)");
  const onSystemTheme = () => (systemDark = darkQuery.matches);
  darkQuery.addEventListener("change", onSystemTheme);
  onDestroy(() => darkQuery.removeEventListener("change", onSystemTheme));

  onMount(async () => {
    settings = withDefaults(await api.getSettings().catch(() => null));
    applySettings(settings);
    vault = (await run(api.getVault)) ?? "";
    await refresh();
    aiHasKey = await api.aiHasKey().catch(() => false);
    setTimeout(() => checkForUpdate(), 4000);
  });
</script>

<svelte:window on:keydown={onKey} on:beforeunload={flush} />

<div class="app" class:focus={focusMode} class:show-list={showList || !current}>
  <aside>
    <div class="side-head">
      <input class="search" placeholder="Filter by name" bind:value={filter} on:input={() => (tagFilter = null)} />
      <button title="New note (⌘N)" on:click={() => newNote()}>+</button>
      <button title="New folder" on:click={() => newFolder()}>⊞</button>
    </div>
    <button class="today" title="Open today's note (⌘⇧D)" on:click={openToday}>☀ Today</button>

    <div
      class="tree"
      class:drop={rootDrop}
      role="tree"
      tabindex="-1"
      on:dragover={(e) => {
        e.preventDefault();
        rootDrop = true;
      }}
      on:dragleave={() => (rootDrop = false)}
      on:drop={(e) => {
        e.preventDefault();
        rootDrop = false;
        const path = e.dataTransfer?.getData("text/plain");
        if (path) moveEntry(path, "");
      }}
    >
      {#if tagFilter}
        <div class="tag-head">
          <span>#{tagFilter}</span>
          <button title="Show all notes" on:click={() => (tagFilter = null)}>×</button>
        </div>
        {#each tagged as path (path)}
          <button class="result" class:active={path === current} on:click={() => openNote(path)}>
            {nameOf(path)}
            {#if dirOf(path)}<span class="where">{dirOf(path)}</span>{/if}
          </button>
        {:else}
          <p class="hint">No notes with this tag</p>
        {/each}
      {:else if filter.trim()}
        {#each filtered as path (path)}
          <button class="result" class:active={path === current} on:click={() => openNote(path)}>
            {nameOf(path)}
            {#if dirOf(path)}<span class="where">{dirOf(path)}</span>{/if}
          </button>
        {:else}
          <p class="hint">No matching notes</p>
        {/each}
      {:else}
        <Tree
          entries={tree}
          {expanded}
          {current}
          {renaming}
          on:open={(e) => openNote(e.detail)}
          on:toggle={(e) => {
            expanded.has(e.detail) ? expanded.delete(e.detail) : expanded.add(e.detail);
            expanded = expanded;
          }}
          on:move={(e) => moveEntry(e.detail.path, e.detail.dir)}
          on:startRename={(e) => (renaming = e.detail)}
          on:rename={(e) => renameEntry(e.detail.path, e.detail.name)}
          on:newNote={(e) => newNote(e.detail)}
          on:newFolder={(e) => newFolder(e.detail)}
          on:delete={(e) => deleteEntry(e.detail)}
        />
        {#if tree.length === 0}
          <p class="hint">No notes yet. Press + to create one.</p>
        {/if}
      {/if}
    </div>

    {#if tagCounts.length}
      <section class="tags">
        <button class="tags-head" on:click={() => (tagsOpen = !tagsOpen)}>{tagsOpen ? "▾" : "▸"} Tags</button>
        {#if tagsOpen}
          <div class="tag-list">
            {#each tagCounts as [tag, count] (tag)}
              <button class="tag-chip" class:active={tag === tagFilter} on:click={() => showTag(tag)}>
                #{tag}<span>{count}</span>
              </button>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    {#if conflicts.length}
      <button class="notice warn" on:click={() => (conflictsOpen = true)}>
        ⚠ {conflicts.length} sync conflict{conflicts.length === 1 ? "" : "s"}
      </button>
    {/if}
    {#if update}
      <button class="notice" disabled={installing} on:click={applyUpdate}>
        {installing ? "Installing update…" : `⬆ Update to ${update.version}`}
      </button>
    {/if}

    <div class="side-foot">
      <button class="vault" title={vault} on:click={chooseVault}>📁 {nameOf(vault) || "Notes folder"}</button>
      {#if git?.isRepo}
        <button
          class="gear sync"
          disabled={syncing}
          title={syncTitle(git)}
          on:click={syncNow}
        >{syncing ? "…" : "⟳"}{#if git.changes || git.ahead || git.behind}<span class="dot" />{/if}</button>
      {/if}
      {#if settings.aiEnabled}
        <button class="gear" class:on={aiOpen} title="AI help" on:click={() => (aiOpen = !aiOpen)}>✦</button>
      {/if}
      <button class="gear" title="Trash" on:click={() => (trashOpen = true)}>🗑</button>
      <button class="gear" title="Settings (⌘,)" on:click={() => (settingsOpen = true)}>⚙</button>
    </div>
  </aside>

  <main>
    {#if current}
      <header>
        <button class="list-toggle" title="Notes" on:click={() => (showList = true)}>☰</button>
        <div class="nav">
          <button title="Back (⌘[)" disabled={!canBack} on:click={() => go(-1)}>‹</button>
          <button title="Forward (⌘])" disabled={!canForward} on:click={() => go(1)}>›</button>
        </div>
        <input
          class="title"
          bind:this={titleInput}
          bind:value={title}
          on:keydown={(e) => e.key === "Enter" && startEditing()}
          on:blur={renameCurrent}
        />
      </header>
      {#if dirOf(current)}<div class="crumb">{dirOf(current)}</div>{/if}
      {#if markers}
        <div class="banner">
          <span>This note has changes from two places (between <code>&lt;&lt;&lt;&lt;&lt;&lt;&lt;</code> and <code>&gt;&gt;&gt;&gt;&gt;&gt;&gt;</code>). Edit it, or keep:</span>
          <button on:click={() => keepSide("mine")}>This computer's</button>
          <button on:click={() => keepSide("theirs")}>The other version</button>
          <button on:click={() => keepSide("both")}>Both</button>
        </div>
      {/if}

      <div class="body">
        {#if mode === "edit"}
          {#key current}
            <Editor
              bind:this={editor}
              value={content}
              {linkOptions}
              placeholder="Write markdown here. Type / or [[ to link another note."
              on:change={(e) => {
                content = e.detail;
                scheduleSave();
              }}
              on:blur={stopEditing}
              on:images={(e) => insertImages(e.detail.files)}
            />
          {/key}
        {:else}
          <!-- Rebuilt when the theme flips, so diagrams are drawn again in the new colours. -->
          {#key dark}
            <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
            <article class="preview" bind:this={previewEl} on:click={onPreviewClick}>
              {@html html}
            </article>
          {/key}
        {/if}
        {#if aiOpen && settings.aiEnabled && !focusMode}
          <AiPanel
            {current}
            {notes}
            hasKey={aiHasKey}
            on:close={() => (aiOpen = false)}
            on:open={(e) => openNote(e.detail)}
            on:settings={() => (settingsOpen = true)}
            on:insert={async (e) => {
              if (mode !== "edit") await startEditing();
              await tick();
              editor?.insert("\n" + e.detail.trim() + "\n", content.length);
            }}
          />
        {:else if settings.showOutline && !focusMode && outline.length >= 2}
          <nav class="outline" aria-label="Outline">
            <h2>Outline</h2>
            {#each outline as h (h.line)}
              <!-- mousedown|preventDefault keeps the editor focused, so jumping doesn't leave edit mode -->
              <button
                on:mousedown|preventDefault
                style="padding-left: {6 + (h.level - Math.min(...outline.map((o) => o.level))) * 12}px"
                on:click={() => jumpTo(h)}
              >
                {h.text}
              </button>
            {/each}
          </nav>
        {/if}
      </div>

      {#if incoming.length && !focusMode}
        <section class="backlinks">
          <h2>Linked from {incoming.length} {incoming.length === 1 ? "place" : "places"}</h2>
          {#each incoming as hit (hit.path + ":" + hit.line)}
            <button on:click={() => openNote(hit.path, undefined, hit.line)}>
              <span class="name">{nameOf(hit.path)}</span>
              {#if dirOf(hit.path)}<span class="where-inline">{dirOf(hit.path)}</span>{/if}
              <span class="snippet">{hit.text}</span>
            </button>
          {/each}
        </section>
      {/if}
    {:else}
      <div class="empty">
        <p>Select a note or create a new one.</p>
        <p class="hint">⌘N new note · ⌘K search · ⌘⇧P commands · ⌘⇧D today's note</p>
        <p class="hint">Click a note to edit, click away (or Esc) to preview · [[Note name]] links a note · #tag tags it</p>
      </div>
    {/if}
    {#if graphOpen}
      <Graph
        {notes}
        {index}
        {current}
        on:close={() => (graphOpen = false)}
        on:open={(e) => {
          graphOpen = false;
          openNote(e.detail);
        }}
      />
    {/if}
  </main>

  {#if toast}<div class="toast" class:info={toast.kind === "info"}>{toast.text}</div>{/if}

  {#if paletteOpen}
    <Palette
      {notes}
      {commands}
      initial={paletteInitial}
      on:close={() => (paletteOpen = false)}
      on:run={(e) => {
        paletteOpen = false;
        e.detail.run();
      }}
      on:choose={(e) => {
        paletteOpen = false;
        openNote(e.detail.path, undefined, e.detail.line);
      }}
    />
  {/if}

  {#if settingsOpen}
    <SettingsDialog
      {settings}
      hasKey={aiHasKey}
      on:change={(e) => updateSettings(e.detail)}
      on:keyChanged={(e) => (aiHasKey = e.detail)}
      on:close={() => (settingsOpen = false)}
    />
  {/if}

  {#if trashOpen}
    <Trash
      on:close={() => (trashOpen = false)}
      on:error={(e) => flash(e.detail)}
      on:restored={async (e) => {
        await refresh();
        flash(`Restored to ${e.detail}`, "info");
      }}
    />
  {/if}

  {#if conflictsOpen}
    <Conflicts
      {conflicts}
      on:close={() => (conflictsOpen = false)}
      on:error={(e) => flash(e.detail)}
      on:open={(e) => {
        conflictsOpen = false;
        openNote(e.detail, "edit");
      }}
      on:settled={async (e) => {
        await refresh();
        if (!conflicts.length) conflictsOpen = false;
        await openNote(e.detail);
      }}
    />
  {/if}
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
  }
  aside {
    width: 260px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--side);
    border-right: 1px solid var(--border);
  }
  .side-head {
    display: flex;
    gap: 4px;
    padding: 10px 8px 6px;
  }
  .side-head button {
    width: 28px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 5px;
    font-size: 14px;
  }
  .side-head button:hover {
    background: var(--hover);
  }
  .search {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
  }
  .tree {
    flex: 1;
    overflow-y: auto;
    padding: 4px 6px;
  }
  .tree.drop {
    background: var(--hover);
  }
  .result {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    color: var(--text);
    padding: 5px 8px;
    border-radius: 4px;
    font-size: 13px;
  }
  .result:hover {
    background: var(--hover);
  }
  .result.active {
    background: var(--active);
  }
  .where {
    display: block;
    font-size: 11px;
    color: var(--muted);
  }
  .hint {
    color: var(--muted);
    font-size: 12px;
    padding: 0 8px;
  }
  .vault {
    margin: 6px;
    padding: 6px 8px;
    border: none;
    background: none;
    color: var(--muted);
    text-align: left;
    font-size: 12px;
    border-radius: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vault:hover {
    background: var(--hover);
    color: var(--text);
  }

  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .app.focus aside {
    display: none;
  }
  .today {
    margin: 0 8px 4px;
    padding: 5px 8px;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    text-align: left;
    font-size: 13px;
  }
  .today:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tag-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }
  .tag-head button,
  .gear {
    border: none;
    background: none;
    color: var(--muted);
    border-radius: 4px;
    font-size: 14px;
    padding: 2px 6px;
  }
  .tag-head button:hover,
  .gear:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tags {
    max-height: 30%;
    overflow-y: auto;
    border-top: 1px solid var(--border);
    padding: 6px 8px;
  }
  .tags-head {
    border: none;
    background: none;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 2px 0 6px;
  }
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .tag-chip {
    border: none;
    border-radius: 10px;
    padding: 2px 8px;
    background: var(--hover);
    color: var(--text);
    font-size: 12px;
  }
  .tag-chip span {
    margin-left: 5px;
    color: var(--muted);
  }
  .tag-chip:hover,
  .tag-chip.active {
    background: var(--active);
    color: var(--accent);
  }
  .notice {
    margin: 0 8px 6px;
    padding: 6px 10px;
    border: none;
    border-radius: 6px;
    background: var(--active);
    color: var(--accent);
    text-align: left;
    font-size: 12px;
  }
  .notice.warn {
    background: color-mix(in srgb, var(--danger) 15%, transparent);
    color: var(--danger);
  }
  .gear.sync {
    position: relative;
  }
  .gear .dot {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .gear.on {
    color: var(--accent);
  }
  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 6px 28px 0;
    padding: 8px 12px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    font-size: 12px;
  }
  .banner span {
    flex: 1 1 260px;
  }
  .banner button {
    padding: 3px 9px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font-size: 12px;
  }
  .list-toggle {
    display: none;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 18px;
  }
  /* Phones: the note list and the note take turns filling the screen. */
  @media (max-width: 700px) {
    aside {
      width: 100%;
    }
    .app:not(.show-list) aside,
    .app.show-list main {
      display: none;
    }
    .list-toggle {
      display: block;
    }
    header {
      padding: 10px 12px 0;
    }
    .crumb,
    .banner {
      margin-left: 12px;
      margin-right: 12px;
      padding-left: 12px;
    }
    .preview {
      padding: 8px 16px 32px;
    }
    .backlinks {
      padding: 10px 10px 14px;
    }
    .body :global(.ai) {
      position: absolute;
      inset: 0;
      width: auto;
      background: var(--bg);
      z-index: 4;
    }
  }
  .side-foot {
    display: flex;
    align-items: center;
    border-top: 1px solid var(--border);
  }
  .side-foot .vault {
    flex: 1;
    min-width: 0;
  }
  @media print {
    aside,
    header .nav,
    .crumb,
    .outline,
    .backlinks,
    .toast {
      display: none !important;
    }
    .app,
    main,
    .body {
      display: block;
      height: auto;
    }
    .preview {
      overflow: visible;
      padding: 0;
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 28px 0;
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: 22px;
    font-weight: 600;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
    padding: 4px 0;
  }
  .crumb {
    padding: 0 28px;
    font-size: 12px;
    color: var(--muted);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .preview {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 8px 28px 40px;
    line-height: 1.65;
    font-size: var(--text-size);
  }
  /* Keep the line length readable without moving the scrollbar off the edge. */
  .preview > :global(*) {
    max-width: var(--measure);
  }
  .outline {
    width: 200px;
    flex-shrink: 0;
    overflow-y: auto;
    padding: 8px 10px 24px 4px;
    border-left: 1px solid var(--border);
  }
  .outline h2,
  .backlinks h2 {
    margin: 0 6px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .outline button {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    color: var(--muted);
    padding: 3px 6px;
    border-radius: 4px;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .outline button:hover {
    background: var(--hover);
    color: var(--text);
  }
  @media (max-width: 900px) {
    .outline {
      display: none;
    }
  }
  .empty {
    margin: auto;
    text-align: center;
    color: var(--muted);
  }
  .nav {
    display: flex;
    gap: 2px;
  }
  .nav button {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    font-size: 18px;
    line-height: 1;
  }
  .nav button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .nav button:disabled {
    opacity: 0.35;
  }
  .backlinks {
    flex-shrink: 0;
    max-height: 30%;
    overflow-y: auto;
    border-top: 1px solid var(--border);
    padding: 10px 22px 14px;
  }
  .backlinks button {
    display: block;
    width: 100%;
    max-width: 800px;
    text-align: left;
    border: none;
    background: none;
    color: var(--text);
    padding: 5px 6px;
    border-radius: 5px;
    font-size: 13px;
  }
  .backlinks button:hover {
    background: var(--hover);
  }
  .backlinks .name {
    font-weight: 500;
  }
  .where-inline {
    margin-left: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .snippet {
    display: block;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toast.info {
    background: var(--accent);
  }
  .toast {
    position: fixed;
    bottom: 16px;
    right: 16px;
    background: var(--danger);
    color: white;
    padding: 8px 14px;
    border-radius: 6px;
    font-size: 13px;
  }
</style>
