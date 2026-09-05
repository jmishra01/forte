<script lang="ts">
  import type { NoteMeta, PdfMeta } from "../types";
  import NoteList from "./NoteList.svelte";
  import NoteTree from "./NoteTree.svelte";
  import PdfList from "./PdfList.svelte";
  import { createEventDispatcher } from "svelte";

  export let view: "notes" | "pdfs" | "trash";
  export let notes: NoteMeta[];
  export let pdfs: PdfMeta[];
  export let tags: string[] = [];
  export let selectedNoteId: string | null;
  export let selectedPdfId: string | null;

  const dispatch = createEventDispatcher<{
    "switch-view": "notes" | "pdfs" | "trash";
    "select-note": string;
    "create-note": void;
    "create-child-note": string;
    "delete-note": string;
    "rename-note": { id: string; title: string };
    "reparent-note": { noteId: string; parentId: string | null; position: number | null };
    "select-pdf": string;
    "add-pdf-file": void;
    "add-pdf-url": void;
    "delete-pdf": string;
    "rename-pdf": { id: string; title: string };
    "open-settings": void;
    "open-palette": void;
  }>();

  let tagFilter: string[] = [];

  $: filteredNotes = notes.filter((n) => {
    if (tagFilter.length && !tagFilter.every((t) => n.tags.includes(t))) return false;
    return true;
  });

  function toggleTag(t: string) {
    tagFilter = tagFilter.includes(t) ? tagFilter.filter((x) => x !== t) : [...tagFilter, t];
  }

  $: isFiltering = tagFilter.length > 0;
</script>

<aside>
  <div class="top-row">
    <div class="tabs">
      <button class:active={view === "notes"} on:click={() => dispatch("switch-view", "notes")}>
        Notes
      </button>
      <button class:active={view === "pdfs"} on:click={() => dispatch("switch-view", "pdfs")}>
        PDFs
      </button>
      <button class:active={view === "trash"} on:click={() => dispatch("switch-view", "trash")}>
        Trash
      </button>
    </div>
    <button class="icon-btn" title="Search (Ctrl/Cmd+K)" on:click={() => dispatch("open-palette")}>⌕</button>
    <button class="icon-btn" title="Settings" on:click={() => dispatch("open-settings")}>⚙</button>
  </div>

  <div class="content">
    {#if view === "notes"}
      {#if tags.length}
        <div class="filters">
          <div class="tag-filters">
            {#each tags as t (t)}
              <button class="tag-pill" class:active={tagFilter.includes(t)} on:click={() => toggleTag(t)}>
                {t}
              </button>
            {/each}
          </div>
        </div>
      {/if}
      {#if isFiltering}
        <NoteList
          notes={filteredNotes}
          selectedId={selectedNoteId}
          on:select={(e) => dispatch("select-note", e.detail)}
          on:create={() => dispatch("create-note")}
          on:delete={(e) => dispatch("delete-note", e.detail)}
        />
      {:else}
        <NoteTree
          {notes}
          selectedId={selectedNoteId}
          on:select={(e) => dispatch("select-note", e.detail)}
          on:create={() => dispatch("create-note")}
          on:create-child={(e) => dispatch("create-child-note", e.detail)}
          on:delete={(e) => dispatch("delete-note", e.detail)}
          on:rename={(e) => dispatch("rename-note", e.detail)}
          on:reparent={(e) => dispatch("reparent-note", e.detail)}
        />
      {/if}
    {:else if view === "pdfs"}
      <PdfList
        {pdfs}
        selectedId={selectedPdfId}
        on:select={(e) => dispatch("select-pdf", e.detail)}
        on:add-file={() => dispatch("add-pdf-file")}
        on:add-url={() => dispatch("add-pdf-url")}
        on:delete={(e) => dispatch("delete-pdf", e.detail)}
        on:rename={(e) => dispatch("rename-pdf", e.detail)}
      />
    {:else}
      <div class="trash-hint">See the Trash panel in the main area.</div>
    {/if}
  </div>
</aside>

<style>
  aside {
    width: 260px;
    min-width: 260px;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .top-row {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
  }
  .tabs {
    display: flex;
    flex: 1;
  }
  .tabs button {
    flex: 1;
    padding: 12px 4px;
    background: none;
    border: none;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    border-bottom: 2px solid transparent;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .icon-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 15px;
    padding: 8px;
  }
  .icon-btn:hover {
    color: var(--text);
  }
  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .filters {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .tag-pill {
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-muted);
  }
  .tag-pill.active {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .trash-hint {
    padding: 16px;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
