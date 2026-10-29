<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, dirOf, nameOf, type Entry } from "./lib/api";
  import { renderMarkdown, resolveLink } from "./lib/markdown";
  import Tree from "./lib/Tree.svelte";
  import { caretCoords } from "./lib/caret";
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
  let error = "";
  let rootDrop = false;
  let editor: HTMLTextAreaElement;
  let previewEl: HTMLElement;
  let titleInput: HTMLInputElement;

  $: notes = flatten(tree);
  $: filtered = filter.trim()
    ? notes.filter((p) => nameOf(p).toLowerCase().includes(filter.trim().toLowerCase()))
    : [];
  $: html = current && mode === "preview" ? renderMarkdown(content, (t: string) => resolveLink(t, current!, notes)) : "";
  $: if (html) tick().then(() => previewEl && renderDiagrams(previewEl));

  function flatten(entries: Entry[]): string[] {
    return entries.flatMap((e) => (e.isDir ? flatten(e.children) : [e.path]));
  }

  function flash(message: string) {
    error = message;
    setTimeout(() => (error = ""), 4000);
  }

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      flash(String(e));
    }
  }

  async function refresh() {
    tree = (await run(api.listTree)) ?? [];
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

  async function openNote(path: string, forceMode?: "edit" | "preview") {
    await flush();
    const text = await run(() => api.readNote(path));
    if (text === undefined) return;
    current = path;
    title = nameOf(path);
    content = text;
    saved = text;
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

  async function startEditing() {
    mode = "edit";
    await tick();
    editor?.focus();
  }

  function stopEditing() {
    picker = null;
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
    if (mod && e.key === "n") {
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

  type PickerItem = { label: string; hint: string; link: string };
  let picker: { start: number; trigger: string; query: string; index: number; x: number; y: number } | null =
    null;

  $: pickerItems = picker ? pickerMatches(picker.trigger, picker.query) : [];

  /** Shortest link text that still points at `path`: the bare name unless another note shares it. */
  function linkFor(path: string) {
    const name = nameOf(path);
    return notes.filter((n) => nameOf(n) === name).length === 1 ? name : path.replace(/\.md$/, "");
  }

  function pickerMatches(trigger: string, query: string): PickerItem[] {
    const q = query.trim().toLowerCase();
    const starts = (p: string) => (nameOf(p).toLowerCase().startsWith(q) ? 0 : 1);
    const items: PickerItem[] = notes
      .filter((p) => p !== current && p.toLowerCase().replace(/\.md$/, "").includes(q))
      .sort((a, b) => starts(a) - starts(b) || nameOf(a).localeCompare(nameOf(b)))
      .slice(0, 8)
      .map((p) => ({ label: nameOf(p), hint: dirOf(p), link: linkFor(p) }));
    // "[[" is clearly a link, so offer to link a note that doesn't exist yet.
    // A plain "/" is too common in normal text (paths, dates) to do that.
    if (trigger === "[[" && q && !items.some((i) => i.label.toLowerCase() === q)) {
      items.push({ label: `New note "${query.trim()}"`, hint: "", link: query.trim() });
    }
    return items;
  }

  function updatePicker(typed: boolean) {
    const pos = editor.selectionStart;
    if (picker) {
      const query = content.slice(picker.start + picker.trigger.length, pos);
      const done = pos !== editor.selectionEnd || pos < picker.start + picker.trigger.length;
      if (done || /[\n\]]/.test(query) || /^\s/.test(query) || query.length > 60) picker = null;
      else picker = { ...picker, query, index: query === picker.query ? picker.index : 0 };
    } else if (typed) {
      const before = content.slice(0, pos);
      const trigger = before.endsWith("[[") ? "[[" : before.endsWith("/") && /(^|\s)\/$/.test(before) ? "/" : "";
      if (!trigger) return;
      const start = pos - trigger.length;
      const { x, y, lineHeight } = caretCoords(editor, start);
      picker = { start, trigger, query: "", index: 0, x: Math.min(x, editor.clientWidth - 300), y: y + lineHeight };
    }
  }

  async function choose(item: PickerItem) {
    if (!picker) return;
    const end = editor.selectionStart;
    const text = `[[${item.link}]]`;
    content = content.slice(0, picker.start) + text + content.slice(end);
    const caret = picker.start + text.length;
    picker = null;
    scheduleSave();
    await tick();
    editor.setSelectionRange(caret, caret);
  }

  function onEditorKey(e: KeyboardEvent) {
    if (picker && pickerItems.length) {
      const n = pickerItems.length;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        picker.index = (picker.index + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        choose(pickerItems[picker.index]);
        return;
      }
    }
    if (e.key === "Escape") {
      if (picker) picker = null;
      else editor.blur();
      return;
    }
    if (e.key !== "Tab") return;
    e.preventDefault();
    const { selectionStart: s, selectionEnd: end } = editor;
    content = content.slice(0, s) + "  " + content.slice(end);
    scheduleSave();
    tick().then(() => editor.setSelectionRange(s + 2, s + 2));
  }

  const unlisten = api.onVaultChanged(onDiskChange);
  onDestroy(() => unlisten.then((stop) => stop()));

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
          on:keydown={(e) => e.key === "Enter" && startEditing()}
          on:blur={renameCurrent}
        />
      </header>
      {#if dirOf(current)}<div class="crumb">{dirOf(current)}</div>{/if}

      {#if mode === "edit"}
        <div class="editor-wrap">
          <textarea
            bind:this={editor}
            bind:value={content}
            on:input={() => {
              scheduleSave();
              updatePicker(true);
            }}
            on:keydown={onEditorKey}
            on:keyup={() => updatePicker(false)}
            on:click={() => updatePicker(false)}
            on:blur={stopEditing}
            spellcheck="false"
            placeholder="Write markdown here. Type / or [[ to link another note."
          />
          {#if picker && pickerItems.length}
            <ul class="picker" style="left: {picker.x}px; top: {picker.y}px">
              {#each pickerItems as item, i}
                <!-- mousedown + preventDefault keeps focus in the editor -->
                <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
                <li
                  class:sel={i === picker.index}
                  on:mousedown|preventDefault={() => choose(item)}
                  on:mouseenter={() => picker && (picker.index = i)}
                >
                  {item.label}
                  {#if item.hint}<span class="where">{item.hint}</span>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {:else}
        <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
        <article class="preview" bind:this={previewEl} on:click={onPreviewClick}>
          {@html html}
        </article>
      {/if}
    {:else}
      <div class="empty">
        <p>Select a note or create a new one.</p>
        <p class="hint">⌘N new note · click a note to edit, click away (or Esc) to preview · [[Note name]] links a note</p>
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
  .editor-wrap {
    flex: 1;
    display: flex;
    position: relative;
    min-height: 0;
  }
  .editor-wrap textarea {
    flex: 1;
  }
  .picker {
    position: absolute;
    width: 280px;
    margin: 4px 0 0;
    padding: 4px;
    list-style: none;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    font-size: 13px;
    z-index: 10;
  }
  .picker li {
    padding: 5px 8px;
    border-radius: 4px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .picker li.sel {
    background: var(--active);
  }
  .picker .where {
    display: inline;
    margin-left: 6px;
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
