<script lang="ts">
  import type { NoteMeta } from "../types";
  import { createEventDispatcher } from "svelte";
  import { get } from "svelte/store";
  import { dragState, dropResult, type DropZone } from "../dragState";

  export let nodes: NoteMeta[];
  export let childrenOf: Map<string, NoteMeta[]>;
  export let selectedId: string | null;
  export let collapsed: Set<string>;
  export let onToggle: (id: string) => void;
  export let depth = 0;

  const dispatch = createEventDispatcher<{
    select: string;
    "create-child": string;
    delete: string;
    rename: { id: string; title: string };
  }>();

  const MOVE_THRESHOLD = 5;

  let renamingId: string | null = null;
  let renameDraft = "";
  let renameInputEl: HTMLInputElement | undefined;

  function formatDate(iso: string) {
    return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }

  function startRename(note: NoteMeta) {
    renamingId = note.id;
    renameDraft = note.title;
  }

  function confirmRename() {
    if (!renamingId) return;
    const title = renameDraft.trim();
    if (title) dispatch("rename", { id: renamingId, title });
    renamingId = null;
  }

  function cancelRename() {
    renamingId = null;
  }

  function focusRenameInput(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  function zoneForRow(rowEl: HTMLElement, clientY: number): DropZone {
    const rect = rowEl.getBoundingClientRect();
    const ratio = (clientY - rect.top) / rect.height;
    if (ratio < 0.25) return "before";
    if (ratio > 0.75) return "after";
    return "inside";
  }

  // Native HTML5 drag-and-drop (draggable + dragstart/dragover/drop) gets
  // intercepted at the GTK level by Tauri's native file-drop handling before
  // it reaches the page, so dragover/drop never fire here. Plain pointer
  // events aren't part of that native drag session, so they work reliably.
  function handlePointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let dragging = false;

    function onMove(ev: PointerEvent) {
      if (!dragging) {
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < MOVE_THRESHOLD) return;
        dragging = true;
        dragState.set({ draggingId: id, overId: null, overZone: null });
      }
      const el = document.elementFromPoint(ev.clientX, ev.clientY);
      const rootZone = el?.closest("[data-root-drop-zone]");
      if (rootZone) {
        dragState.set({ draggingId: id, overId: "__root__", overZone: "inside" });
        return;
      }
      const rowEl = el?.closest("[data-note-row]") as HTMLElement | null;
      if (!rowEl) {
        dragState.set({ draggingId: id, overId: null, overZone: null });
        return;
      }
      const overId = rowEl.dataset.noteId!;
      if (overId === id) {
        dragState.set({ draggingId: id, overId: null, overZone: null });
        return;
      }
      dragState.set({ draggingId: id, overId, overZone: zoneForRow(rowEl, ev.clientY) });
    }

    function onUp() {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      if (dragging) {
        const final = get(dragState);
        if (final.overId && final.overZone) {
          dropResult.set({ draggedId: id, overId: final.overId, overZone: final.overZone });
        }
      }
      dragState.set({ draggingId: null, overId: null, overZone: null });
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }
</script>

<ul class="tree-level">
  {#each nodes as note (note.id)}
    {@const kids = childrenOf.get(note.id) ?? []}
    {@const isCollapsed = collapsed.has(note.id)}
    <li>
      <div
        class="row"
        class:active={note.id === selectedId}
        class:drop-inside={$dragState.overId === note.id && $dragState.overZone === "inside"}
        class:drop-before={$dragState.overId === note.id && $dragState.overZone === "before"}
        class:drop-after={$dragState.overId === note.id && $dragState.overZone === "after"}
        style="padding-left: {depth * 16}px"
        data-note-row
        data-note-id={note.id}
      >
        <span
          class="drag-handle"
          title="Drag to move"
          on:pointerdown={(e) => handlePointerDown(e, note.id)}>⠿</span
        >
        <button
          class="disclosure"
          class:invisible={kids.length === 0}
          on:click={() => onToggle(note.id)}
        >
          {isCollapsed ? "▸" : "▾"}
        </button>
        {#if renamingId === note.id}
          <input
            class="rename-input"
            bind:value={renameDraft}
            bind:this={renameInputEl}
            use:focusRenameInput
            on:keydown={(e) => {
              if (e.key === "Enter") confirmRename();
              if (e.key === "Escape") cancelRename();
            }}
            on:blur={confirmRename}
          />
        {:else}
          <button class="title-btn" on:click={() => dispatch("select", note.id)}>
            <span class="title">{note.title || "Untitled"}</span>
            <span class="meta">{formatDate(note.updatedAt)}</span>
          </button>
        {/if}
        <button class="rename" title="Rename" on:click={() => startRename(note)}>✎</button>
        <button class="add-child" title="New sub-page" on:click={() => dispatch("create-child", note.id)}>
          +
        </button>
        <button class="delete" title="Move to trash" on:click={() => dispatch("delete", note.id)}>
          ×
        </button>
      </div>
      {#if kids.length && !isCollapsed}
        <svelte:self
          nodes={kids}
          {childrenOf}
          {selectedId}
          {collapsed}
          {onToggle}
          depth={depth + 1}
          on:select
          on:create-child
          on:delete
          on:rename
        />
      {/if}
    </li>
  {/each}
</ul>

<style>
  .tree-level {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    border-radius: 6px;
  }
  .row.active {
    background: var(--bg-alt);
  }
  .row.drop-inside {
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-alt));
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row.drop-before,
  .row.drop-after {
    position: relative;
  }
  .row.drop-before::before,
  .row.drop-after::after {
    content: "";
    position: absolute;
    left: 2px;
    right: 2px;
    height: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .row.drop-before::before {
    top: -3px;
  }
  .row.drop-after::after {
    bottom: -3px;
  }
  .drag-handle {
    cursor: grab;
    color: var(--text-muted);
    font-size: 12px;
    padding: 4px 2px;
    visibility: hidden;
    flex-shrink: 0;
    touch-action: none;
  }
  .row:hover .drag-handle {
    visibility: visible;
  }
  .disclosure {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 10px;
    width: 18px;
    flex-shrink: 0;
    padding: 8px 0;
  }
  .disclosure.invisible {
    visibility: hidden;
  }
  .title-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    background: none;
    border: none;
    padding: 8px 4px;
    text-align: left;
    min-width: 0;
  }
  .title {
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
    color: var(--text);
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
  }
  .rename-input {
    flex: 1;
    font-size: 13px;
    padding: 6px 4px;
    margin: 2px 0;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    min-width: 0;
  }
  .rename,
  .add-child,
  .delete {
    background: none;
    border: none;
    outline: none;
    box-shadow: none;
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    padding: 4px 6px;
    visibility: hidden;
    flex-shrink: 0;
  }
  .delete {
    font-size: 16px;
  }
  .row:hover .rename,
  .row:hover .add-child,
  .row:hover .delete {
    visibility: visible;
  }
  .rename:hover,
  .add-child:hover {
    color: var(--accent);
  }
  .delete:hover {
    color: var(--danger);
  }
</style>
