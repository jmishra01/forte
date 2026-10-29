<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Entry } from "./api";
  import IconButton from "./IconButton.svelte";
  import { Collapsed, Expanded, NewFolder, NewNote, Rename, Trash } from "./icons";

  export let entries: Entry[];
  export let expanded: Set<string>;
  export let current: string | null;
  export let renaming: string | null;
  export let depth = 0;
  /** The one row reachable with Tab (the open note, or the first row); arrow keys move from there. */
  export let focusable: string | null = null;

  const dispatch = createEventDispatcher();
  let dropTarget: string | null = null;

  const parentOf = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");

  /** Arrow keys move between rows; Enter opens; F2 renames; Delete moves to the trash. */
  function onRowKey(e: KeyboardEvent, entry: Entry) {
    if (e.target !== e.currentTarget) return; // typing in the rename box
    const row = e.currentTarget as HTMLElement;
    const rows = Array.from(row.closest('[role="tree"]')?.querySelectorAll<HTMLElement>(".row") ?? []);
    const at = rows.indexOf(row);
    const focus = (r: HTMLElement | undefined) => r && (e.preventDefault(), r.focus());
    const open = expanded.has(entry.path);
    switch (e.key) {
      case "ArrowDown":
        return focus(rows[at + 1]);
      case "ArrowUp":
        return focus(rows[at - 1]);
      case "Home":
        return focus(rows[0]);
      case "End":
        return focus(rows[rows.length - 1]);
      case "ArrowRight":
        e.preventDefault();
        if (entry.isDir && !open) dispatch("toggle", entry.path);
        else if (entry.isDir) focus(rows[at + 1]);
        return;
      case "ArrowLeft":
        e.preventDefault();
        if (entry.isDir && open) dispatch("toggle", entry.path);
        else focus(rows.find((r) => r.dataset.path === parentOf(entry.path)));
        return;
      case "Enter":
      case " ":
        e.preventDefault();
        return dispatch(entry.isDir ? "toggle" : e.metaKey || e.ctrlKey ? "openTab" : "open", entry.path);
      case "F2":
        e.preventDefault();
        return dispatch("startRename", entry.path);
      case "Delete":
      case "Backspace":
        if (e.key === "Backspace" && !(e.metaKey || e.ctrlKey)) return;
        e.preventDefault();
        return dispatch("delete", entry.path);
    }
  }

  /** A row button's click handler: it acts on the entry without also opening the row. */
  const act = (event: string, path: string) => (e: MouseEvent) => {
    e.stopPropagation();
    dispatch(event, path);
  };

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  function onRenameKey(e: KeyboardEvent, entry: Entry) {
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === "Enter") input.blur();
    if (e.key === "Escape") {
      input.value = entry.name;
      input.blur();
    }
  }

  function onDrop(e: DragEvent, dir: string) {
    e.preventDefault();
    e.stopPropagation();
    dropTarget = null;
    const path = e.dataTransfer?.getData("text/plain");
    if (path) dispatch("move", { path, dir });
  }
</script>

{#each entries as entry (entry.path)}
  <div
    class="row"
    class:active={entry.path === current}
    class:drop={dropTarget === entry.path}
    style="padding-left: {8 + depth * 14}px"
    draggable={renaming !== entry.path}
    role="treeitem"
    aria-selected={entry.path === current}
    aria-expanded={entry.isDir ? expanded.has(entry.path) : undefined}
    aria-level={depth + 1}
    data-path={entry.path}
    tabindex={entry.path === focusable ? 0 : -1}
    on:dragstart={(e) => e.dataTransfer?.setData("text/plain", entry.path)}
    on:dragover={(e) => {
      if (!entry.isDir) return;
      e.preventDefault();
      dropTarget = entry.path;
    }}
    on:dragleave={() => (dropTarget = null)}
    on:drop={(e) => entry.isDir && onDrop(e, entry.path)}
    on:click={(e) => dispatch(entry.isDir ? "toggle" : e.metaKey || e.ctrlKey ? "openTab" : "open", entry.path)}
    on:auxclick={(e) => !entry.isDir && e.button === 1 && dispatch("openTab", entry.path)}
    on:keydown={(e) => onRowKey(e, entry)}
  >
    <span class="icon" aria-hidden="true">
      {#if entry.isDir}<svelte:component this={expanded.has(entry.path) ? Expanded : Collapsed} size={14} strokeWidth={2} />{/if}
    </span>
    {#if renaming === entry.path}
      <input
        class="rename"
        value={entry.name}
        use:focusSelect
        on:click|stopPropagation
        on:keydown={(e) => onRenameKey(e, entry)}
        on:blur={(e) => dispatch("rename", { path: entry.path, name: e.currentTarget.value })}
      />
    {:else}
      <span class="name" class:folder={entry.isDir}>{entry.name}</span>
      <span class="actions">
        {#if entry.isDir}
          <IconButton size="sm" icon={NewNote} label="New note in {entry.name}" on:click={act("newNote", entry.path)} />
          <IconButton size="sm" icon={NewFolder} label="New folder in {entry.name}" on:click={act("newFolder", entry.path)} />
        {/if}
        <IconButton size="sm" icon={Rename} label="Rename {entry.name}" on:click={act("startRename", entry.path)} />
        <IconButton size="sm" icon={Trash} label="Move {entry.name} to the trash" on:click={act("delete", entry.path)} />
      </span>
    {/if}
  </div>
  {#if entry.isDir && expanded.has(entry.path)}
    <svelte:self
      entries={entry.children}
      {expanded}
      {current}
      {renaming}
      {focusable}
      depth={depth + 1}
      on:open
      on:openTab
      on:toggle
      on:move
      on:rename
      on:startRename
      on:newNote
      on:newFolder
      on:delete
    />
  {/if}
{/each}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding-right: 6px;
    border-radius: 4px;
    cursor: pointer;
    user-select: none;
    font-size: 13px;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.active {
    background: var(--active);
  }
  .row.drop {
    outline: 1px dashed var(--accent);
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    color: var(--muted);
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.folder {
    font-weight: 500;
  }
  .actions {
    display: none;
    align-items: center;
    gap: 1px;
  }
  .row:hover .actions,
  .row:focus-within .actions {
    display: flex;
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .rename {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    padding: 2px 4px;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
  }
</style>
