<script lang="ts">
  import { onMount, tick } from "svelte";
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, dirOf, nameOf, type Entry } from "./lib/api";
  import { renderMarkdown, resolveLink } from "./lib/markdown";
  import Tree from "./lib/Tree.svelte";

  let vault = "";
  let tree: Entry[] = [];
  let expanded = new Set<string>();
  let current: string | null = null;
  let title = "";
  let content = "";
  let mode: "edit" | "preview" = "edit";
  let renaming: string | null = null;
  let filter = "";
  let error = "";
  let rootDrop = false;
  let editor: HTMLTextAreaElement;
  let titleInput: HTMLInputElement;

  $: notes = flatten(tree);
  $: filtered = filter.trim()
    ? notes.filter((p) => nameOf(p).toLowerCase().includes(filter.trim().toLowerCase()))
    : [];
  $: html = current && mode === "preview" ? renderMarkdown(content, (t: string) => resolveLink(t, current!, notes)) : "";

  function flatten(entries: Entry[]): string[] {
    return entries.flatMap((e) => (e.isDir ? flatten(e.children) : [e.path]));
  }

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      error = String(e);
      setTimeout(() => (error = ""), 4000);
    }
  }

  async function refresh() {
    tree = (await run(api.listTree)) ?? [];
  }

  // --- saving ---------------------------------------------------------------

  let saveTimer: ReturnType<typeof setTimeout>;
  let pending: { path: string; content: string } | null = null;

  function scheduleSave() {
    if (!current) return;
    pending = { path: current, content };
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 400);
  }

  async function flush() {
    clearTimeout(saveTimer);
    if (!pending) return;
    const { path, content } = pending;
    pending = null;
    await run(() => api.writeNote(path, content));
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

  async function openNote(path: string, forceMode?: "edit" | "preview") {
    await flush();
    const text = await run(() => api.readNote(path));
    if (text === undefined) return;
    current = path;
    title = nameOf(path);
    content = text;
    mode = forceMode ?? (text.trim() ? "preview" : "edit");
    expandTo(path);
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

  /** Keep `current` pointing at the same note after `from` was renamed or moved to `to`. */
  function repoint(from: string, to: string) {
    if (!current) return;
    if (current === from) current = to;
    else if (current.startsWith(from + "/")) current = to + current.slice(from.length);
    else return;
    if (pending) pending.path = current;
    title = nameOf(current);
    expandTo(current);
  }

  async function renameEntry(path: string, name: string) {
    renaming = null;
    if (!name.trim() || name.trim() === nameOf(path)) return;
    await flush();
    const to = await run(() => api.rename(path, name));
    if (!to) return;
    if (expanded.delete(path)) expanded.add(to);
    repoint(path, to);
    await refresh();
  }

  async function renameCurrent() {
    if (current && title.trim() !== nameOf(current)) await renameEntry(current, title);
    if (current) title = nameOf(current);
  }

  async function moveEntry(path: string, dir: string) {
    if (dirOf(path) === dir) return;
    await flush();
    const to = await run(() => api.move(path, dir));
    if (!to) return;
    if (dir) expanded.add(dir);
    repoint(path, to);
    await refresh();
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

  function onPreviewClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
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
    if (mod && e.key === "n") {
      e.preventDefault();
      newNote(current ? dirOf(current) : "");
    } else if (mod && e.key === "e" && current) {
      e.preventDefault();
      mode = mode === "edit" ? "preview" : "edit";
      if (mode === "edit") tick().then(() => editor?.focus());
    } else if (mod && e.key === "s") {
      e.preventDefault();
      flush();
    }
  }

  function onEditorKey(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    e.preventDefault();
    const { selectionStart: s, selectionEnd: end } = editor;
    content = content.slice(0, s) + "  " + content.slice(end);
    scheduleSave();
    tick().then(() => editor.setSelectionRange(s + 2, s + 2));
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
      <input class="search" placeholder="Search notes" bind:value={filter} />
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
        <input
          class="title"
          bind:this={titleInput}
          bind:value={title}
          on:keydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          on:blur={renameCurrent}
        />
        <div class="modes">
          <button class:on={mode === "edit"} on:click={() => (mode = "edit")}>Edit</button>
          <button class:on={mode === "preview"} on:click={() => (mode = "preview")}>Preview</button>
        </div>
      </header>
      {#if dirOf(current)}<div class="crumb">{dirOf(current)}</div>{/if}

      {#if mode === "edit"}
        <textarea
          bind:this={editor}
          bind:value={content}
          on:input={scheduleSave}
          on:keydown={onEditorKey}
          spellcheck="false"
          placeholder="Write markdown here. Link another note with [[Note name]]."
        />
      {:else}
        <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
        <article class="preview" on:click={onPreviewClick} on:dblclick={() => (mode = "edit")}>
          {@html html}
        </article>
      {/if}
    {:else}
      <div class="empty">
        <p>Select a note or create a new one.</p>
        <p class="hint">⌘N new note · ⌘E switch edit / preview · [[Note name]] links a note</p>
      </div>
    {/if}
  </main>

  {#if error}<div class="toast">{error}</div>{/if}
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
  .modes {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .modes button {
    border: none;
    background: none;
    color: var(--muted);
    padding: 4px 12px;
    font-size: 12px;
  }
  .modes button.on {
    background: var(--active);
    color: var(--text);
  }
  .crumb {
    padding: 0 28px;
    font-size: 12px;
    color: var(--muted);
  }
  textarea {
    flex: 1;
    border: none;
    outline: none;
    resize: none;
    padding: 16px 28px 40px;
    background: var(--bg);
    color: var(--text);
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 14px;
    line-height: 1.6;
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
