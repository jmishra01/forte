<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { Entry } from "./api";

  export let entries: Entry[];
  export let expanded: Set<string>;
  export let current: string | null;
  export let renaming: string | null;
  export let depth = 0;

  const dispatch = createEventDispatcher();
  let dropTarget: string | null = null;

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
    tabindex="-1"
    on:dragstart={(e) => e.dataTransfer?.setData("text/plain", entry.path)}
    on:dragover={(e) => {
      if (!entry.isDir) return;
      e.preventDefault();
      dropTarget = entry.path;
    }}
    on:dragleave={() => (dropTarget = null)}
    on:drop={(e) => entry.isDir && onDrop(e, entry.path)}
    on:click={() => dispatch(entry.isDir ? "toggle" : "open", entry.path)}
    on:keydown={() => {}}
  >
    <span class="icon">{entry.isDir ? (expanded.has(entry.path) ? "▾" : "▸") : ""}</span>
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
          <button title="New note" on:click|stopPropagation={() => dispatch("newNote", entry.path)}>+</button>
          <button title="New folder" on:click|stopPropagation={() => dispatch("newFolder", entry.path)}>⊞</button>
        {/if}
        <button title="Rename" on:click|stopPropagation={() => dispatch("startRename", entry.path)}>✎</button>
        <button title="Delete" on:click|stopPropagation={() => dispatch("delete", entry.path)}>×</button>
      </span>
    {/if}
  </div>
  {#if entry.isDir && expanded.has(entry.path)}
    <svelte:self
      entries={entry.children}
      {expanded}
      {current}
      {renaming}
      depth={depth + 1}
      on:open
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
    width: 12px;
    flex-shrink: 0;
    color: var(--muted);
    font-size: 10px;
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
    gap: 2px;
  }
  .row:hover .actions {
    display: flex;
  }
  .actions button {
    border: none;
    background: none;
    color: var(--muted);
    padding: 0 4px;
    font-size: 13px;
    line-height: 20px;
    border-radius: 3px;
  }
  .actions button:hover {
    color: var(--text);
    background: var(--border);
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
