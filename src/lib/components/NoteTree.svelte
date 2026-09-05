<script lang="ts">
  import type { NoteMeta } from "../types";
  import { createEventDispatcher, onDestroy } from "svelte";
  import NoteTreeLevel from "./NoteTreeLevel.svelte";
  import { posOf } from "../noteOrder";
  import { dragState, dropResult, ROOT_ZONE_ID, type DropResult } from "../dragState";

  export let notes: NoteMeta[] = [];
  export let selectedId: string | null = null;

  const dispatch = createEventDispatcher<{
    select: string;
    create: void;
    "create-child": string;
    delete: string;
    rename: { id: string; title: string };
    reparent: { noteId: string; parentId: string | null; position: number | null };
  }>();

  let collapsed = new Set<string>();

  function toggle(id: string) {
    if (collapsed.has(id)) collapsed.delete(id);
    else collapsed.add(id);
    collapsed = collapsed;
  }

  $: childrenOf = (() => {
    const map = new Map<string, NoteMeta[]>();
    const ids = new Set(notes.map((n) => n.id));
    for (const n of notes) {
      // A parent that's been filtered out or deleted falls back to root
      // so the note is never silently hidden.
      const key = n.parentId && ids.has(n.parentId) ? n.parentId : "__root__";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(n);
    }
    for (const list of map.values()) {
      list.sort((a, b) => posOf(a) - posOf(b));
    }
    return map;
  })();

  $: roots = childrenOf.get("__root__") ?? [];

  // NoteTreeLevel instances only report raw pointer/hover state (they don't
  // know about siblings outside their own recursion level); this is the one
  // place with full-tree data, so it owns the actual position computation.
  const unsubscribe = dropResult.subscribe((result) => {
    if (result) {
      handleDropResult(result);
      dropResult.set(null);
    }
  });
  onDestroy(unsubscribe);

  function handleDropResult({ draggedId, overId, overZone }: DropResult) {
    if (draggedId === overId) return;

    if (overId === ROOT_ZONE_ID) {
      const maxPos = roots.length ? Math.max(...roots.map(posOf)) : 0;
      dispatch("reparent", { noteId: draggedId, parentId: null, position: maxPos + 1000 });
      return;
    }

    const overNote = notes.find((n) => n.id === overId);
    if (!overNote) return;

    if (overZone === "inside") {
      dispatch("reparent", { noteId: draggedId, parentId: overNote.id, position: null });
      return;
    }

    const siblings = childrenOf.get(overNote.parentId ?? "__root__") ?? [];
    const idx = siblings.findIndex((n) => n.id === overId);
    const targetPos = posOf(overNote);
    let newPosition: number;
    if (overZone === "before") {
      const prev = idx > 0 ? siblings[idx - 1] : null;
      const prevPos = prev ? posOf(prev) : targetPos - 1000;
      newPosition = (prevPos + targetPos) / 2;
    } else {
      const next = idx < siblings.length - 1 ? siblings[idx + 1] : null;
      const nextPos = next ? posOf(next) : targetPos + 1000;
      newPosition = (targetPos + nextPos) / 2;
    }
    dispatch("reparent", { noteId: draggedId, parentId: overNote.parentId, position: newPosition });
  }
</script>

<div class="panel">
  <button class="new-btn" on:click={() => dispatch("create")}>+ New note</button>
  {#if $dragState.draggingId}
    <div
      class="root-drop-zone"
      class:active={$dragState.overId === ROOT_ZONE_ID}
      data-root-drop-zone
    >
      Drop here to move to top level
    </div>
  {/if}
  <div class="scroll">
    {#if roots.length}
      <NoteTreeLevel
        nodes={roots}
        {childrenOf}
        {selectedId}
        {collapsed}
        onToggle={toggle}
        on:select
        on:create-child
        on:delete
        on:rename
      />
    {:else}
      <p class="empty">No notes yet.</p>
    {/if}
  </div>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
  .new-btn {
    margin: 10px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--accent);
    color: white;
    font-size: 13px;
    font-weight: 600;
  }
  .new-btn:hover {
    background: var(--accent-hover);
  }
  .root-drop-zone {
    margin: 0 10px 8px;
    padding: 6px 8px;
    border: 1px dashed var(--border);
    border-radius: 6px;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }
  .root-drop-zone.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--bg-alt);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 0 6px 10px;
  }
  .empty {
    color: var(--text-muted);
    font-size: 13px;
    padding: 10px;
    margin: 0;
  }
</style>
