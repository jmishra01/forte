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
  <button class="new-btn" on:click={() => dispatch("create")}>
    <svg viewBox="0 0 20 20" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
      <path d="M6 3h5l3 3v10a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z" />
      <path d="M11 3v3h3" />
      <line x1="10" y1="10.5" x2="10" y2="14.5" />
      <line x1="8" y1="12.5" x2="12" y2="12.5" />
    </svg>
    New note
  </button>
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
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
