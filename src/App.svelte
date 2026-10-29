<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, dirOf, nameOf, type Entry, type NoteLinks } from "./lib/api";
  import { relativeHref, renderMarkdown, resolveLink, resolveRelative } from "./lib/markdown";
  import { backlinks, linkFor, rewriteLinks } from "./lib/links";
  import Tree from "./lib/Tree.svelte";
  import Palette from "./lib/Palette.svelte";
  import Editor, { type LinkOption } from "./lib/Editor.svelte";
  import { renderDiagrams } from "./lib/diagrams";

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
  let index: NoteLinks[] = [];
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
  $: html =
    current && mode === "preview"
      ? renderMarkdown(content, (t: string) => resolveLink(t, current!, notes), imageSrc)
      : "";
  $: incoming = current ? backlinks(current, index, notes) : [];
  $: canBack = history.slice(0, place).some((p) => notes.includes(p));
  $: canForward = history.slice(place + 1).some((p) => notes.includes(p));

  /** Relative images point at files in the vault, served over Tauri's asset protocol. */
  function imageSrc(href: string) {
    const rel = current && resolveRelative(href, current);
    return rel ? convertFileSrc(`${vault}/${rel}`) : href;
  }
  $: if (html) tick().then(() => previewEl && renderDiagrams(previewEl));

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
    [tree, index] = await Promise.all([run(api.listTree).then((t) => t ?? []), api.linkIndex().catch(() => index)]);
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
    const oldIndex = await api.linkIndex().catch(() => index);
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

  async function startEditing() {
    mode = "edit";
    await tick();
    editor?.focus();
  }

  function stopEditing() {
    flush();
    if (content.trim()) mode = "preview";
  }

  function onPreviewClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) {
      // Clicking the text (but not selecting it) starts editing.
      if (!window.getSelection()?.toString()) startEditing();
      return;
    }
    e.preventDefault();
    const wiki = a.getAttribute("data-wikilink");
    const href = a.getAttribute("href") ?? "";
    if (wiki) followLink(wiki);
    else if (/^(https?|mailto):/i.test(href)) openUrl(href);
    else if (/\.md(#.*)?$/i.test(href)) followLink(decodeURIComponent(href));
  }

  // --- keyboard -------------------------------------------------------------

  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && (e.key === "k" || (e.shiftKey && e.key.toLowerCase() === "f"))) {
      e.preventDefault();
      paletteOpen = true;
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

  onMount(async () => {
    vault = (await run(api.getVault)) ?? "";
    await refresh();
  });
</script>

<svelte:window on:keydown={onKey} on:beforeunload={flush} />

<div class="app">
  <aside>
    <div class="side-head">
      <input class="search" placeholder="Filter by name" bind:value={filter} />
      <button title="New note (⌘N)" on:click={() => newNote()}>+</button>
      <button title="New folder" on:click={() => newFolder()}>⊞</button>
    </div>

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
      {#if filter.trim()}
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

    <button class="vault" title={vault} on:click={chooseVault}>📁 {nameOf(vault) || "Notes folder"}</button>
  </aside>

  <main>
    {#if current}
      <header>
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
        <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
        <article class="preview" bind:this={previewEl} on:click={onPreviewClick}>
          {@html html}
        </article>
      {/if}

      {#if incoming.length}
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
        <p class="hint">⌘N new note · ⌘K search · click a note to edit, click away (or Esc) to preview · [[Note name]] links a note</p>
      </div>
    {/if}
  </main>

  {#if toast}<div class="toast" class:info={toast.kind === "info"}>{toast.text}</div>{/if}

  {#if paletteOpen}
    <Palette
      {notes}
      on:close={() => (paletteOpen = false)}
      on:choose={(e) => {
        paletteOpen = false;
        openNote(e.detail.path, undefined, e.detail.line);
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
  .preview {
    flex: 1;
    overflow-y: auto;
    padding: 8px 28px 40px;
    line-height: 1.65;
    font-size: 15px;
    max-width: 820px;
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
  .backlinks h2 {
    margin: 0 6px 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
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
