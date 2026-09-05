<script lang="ts">
  import type { NoteMeta } from "../types";
  import { createEventDispatcher } from "svelte";

  export let notes: NoteMeta[] = [];
  export let selectedId: string | null = null;

  const dispatch = createEventDispatcher<{
    select: string;
    create: void;
    delete: string;
  }>();

  function formatDate(iso: string) {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
</script>

<div class="panel">
  <button class="new-btn" on:click={() => dispatch("create")}>+ New note</button>
  <ul>
    {#each notes as note (note.id)}
      <li class:active={note.id === selectedId}>
        <button class="row" on:click={() => dispatch("select", note.id)}>
          <span class="title">{note.title || "Untitled"}</span>
          <span class="meta">{formatDate(note.updatedAt)}</span>
        </button>
        <button
          class="delete"
          title="Move to trash"
          on:click={() => dispatch("delete", note.id)}
        >×</button>
      </li>
    {:else}
      <li class="empty">No notes yet.</li>
    {/each}
  </ul>
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
  ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 10px;
    overflow-y: auto;
    flex: 1;
  }
  li {
    display: flex;
    align-items: center;
    border-radius: 6px;
  }
  li.active {
    background: var(--bg-alt);
  }
  li.empty {
    color: var(--text-muted);
    padding: 10px;
    font-size: 13px;
  }
  .row {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    background: none;
    border: none;
    padding: 8px 10px;
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
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
  }
  .delete {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 16px;
    padding: 4px 8px;
    visibility: hidden;
  }
  li:hover .delete {
    visibility: visible;
  }
  .delete:hover {
    color: var(--danger);
  }
</style>
