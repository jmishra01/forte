<script lang="ts">
  import type { NoteMeta } from "../types";
  import { createEventDispatcher } from "svelte";
  import NoteTreeLevel from "./NoteTreeLevel.svelte";

  export let notes: NoteMeta[] = [];
  export let selectedId: string | null = null;

  const dispatch = createEventDispatcher<{
    select: string;
    create: void;
    "create-child": string;
    delete: string;
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
      list.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
    }
    return map;
  })();

  $: roots = childrenOf.get("__root__") ?? [];
</script>

<div class="panel">
  <button class="new-btn" on:click={() => dispatch("create")}>+ New note</button>
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
