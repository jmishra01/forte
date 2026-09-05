<script lang="ts">
  import type { NoteMeta } from "../types";
  import { createEventDispatcher } from "svelte";

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
  }>();

  function formatDate(iso: string) {
    return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
</script>

<ul class="tree-level">
  {#each nodes as note (note.id)}
    {@const kids = childrenOf.get(note.id) ?? []}
    {@const isCollapsed = collapsed.has(note.id)}
    <li>
      <div class="row" class:active={note.id === selectedId} style="padding-left: {depth * 16}px">
        <button
          class="disclosure"
          class:invisible={kids.length === 0}
          on:click={() => onToggle(note.id)}
        >
          {isCollapsed ? "▸" : "▾"}
        </button>
        <button class="title-btn" on:click={() => dispatch("select", note.id)}>
          <span class="title">{note.title || "Untitled"}</span>
          <span class="meta">{formatDate(note.updatedAt)}</span>
        </button>
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
  .add-child,
  .delete {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 14px;
    padding: 4px 6px;
    visibility: hidden;
    flex-shrink: 0;
  }
  .delete {
    font-size: 16px;
  }
  .row:hover .add-child,
  .row:hover .delete {
    visibility: visible;
  }
  .add-child:hover {
    color: var(--accent);
  }
  .delete:hover {
    color: var(--danger);
  }
</style>
