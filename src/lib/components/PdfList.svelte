<script lang="ts">
  import type { PdfMeta } from "../types";
  import { createEventDispatcher } from "svelte";

  export let pdfs: PdfMeta[] = [];
  export let selectedId: string | null = null;

  const dispatch = createEventDispatcher<{
    select: string;
    "add-file": void;
    "add-url": void;
    delete: string;
  }>();
</script>

<div class="panel">
  <div class="actions">
    <button class="new-btn" on:click={() => dispatch("add-file")}>+ Add PDF file</button>
    <button class="new-btn secondary" on:click={() => dispatch("add-url")}>+ Add from URL</button>
  </div>
  <ul>
    {#each pdfs as pdf (pdf.id)}
      <li class:active={pdf.id === selectedId}>
        <button class="row" on:click={() => dispatch("select", pdf.id)}>
          <span class="title">{pdf.title}</span>
          <span class="meta">{pdf.source === "url" ? "🔗 URL" : "📄 file"}</span>
        </button>
        <button
          class="delete"
          title="Move to trash"
          on:click={() => dispatch("delete", pdf.id)}
        >×</button>
      </li>
    {:else}
      <li class="empty">No PDFs yet.</li>
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
  .actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 10px;
  }
  .new-btn {
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
  .new-btn.secondary {
    background: var(--bg-alt);
    color: var(--text);
  }
  .new-btn.secondary:hover {
    background: var(--border);
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
