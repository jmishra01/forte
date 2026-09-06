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

  let selectedTag = "";

  $: filteredNotes = selectedTag ? notes.filter((n) => n.tags.includes(selectedTag)) : notes;

  $: isFiltering = selectedTag !== "";

  const viewLabels: Record<typeof view, string> = {
    notes: "Notes",
    pdfs: "PDFs",
    trash: "Trash",
  };
</script>

<aside>
  <nav class="rail">
    <button class:active={view === "notes"} title="Notes" on:click={() => dispatch("switch-view", "notes")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <rect x="4" y="3" width="12" height="14" rx="1.5" />
        <line x1="7" y1="7" x2="13" y2="7" />
        <line x1="7" y1="10" x2="13" y2="10" />
        <line x1="7" y1="13" x2="11" y2="13" />
      </svg>
    </button>
    <button class:active={view === "pdfs"} title="PDFs" on:click={() => dispatch("switch-view", "pdfs")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M6 2.5h5l3 3v11a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1z" />
        <path d="M11 2.5v3h3" />
        <line x1="7.5" y1="11.5" x2="12.5" y2="11.5" />
        <line x1="7.5" y1="14" x2="11" y2="14" />
      </svg>
    </button>
    <button class:active={view === "trash"} title="Trash" on:click={() => dispatch("switch-view", "trash")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 6h12" />
        <path d="M8 6V4a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v2" />
        <path d="M6 6l1 11a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1l1-11" />
        <line x1="8.5" y1="9" x2="8.5" y2="14" />
        <line x1="11.5" y1="9" x2="11.5" y2="14" />
      </svg>
    </button>
    <div class="spacer" />
    <button title="Search (Ctrl/Cmd+K)" on:click={() => dispatch("open-palette")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="9" cy="9" r="5.5" />
        <line x1="13.2" y1="13.2" x2="17" y2="17" />
      </svg>
    </button>
    <button title="Settings" on:click={() => dispatch("open-settings")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="10" cy="10" r="2.6" />
        <line x1="10" y1="2.5" x2="10" y2="5" />
        <line x1="10" y1="15" x2="10" y2="17.5" />
        <line x1="2.5" y1="10" x2="5" y2="10" />
        <line x1="15" y1="10" x2="17.5" y2="10" />
        <line x1="4.8" y1="4.8" x2="6.5" y2="6.5" />
        <line x1="13.5" y1="13.5" x2="15.2" y2="15.2" />
        <line x1="4.8" y1="15.2" x2="6.5" y2="13.5" />
        <line x1="13.5" y1="6.5" x2="15.2" y2="4.8" />
      </svg>
    </button>
  </nav>

  <div class="panel">
    <div class="panel-header">{viewLabels[view]}</div>

    <div class="content">
      {#if view === "notes"}
        {#if tags.length}
          <div class="filters">
            <select class="tag-select" bind:value={selectedTag}>
              <option value="">All tags</option>
              {#each tags as t (t)}
                <option value={t}>{t}</option>
              {/each}
            </select>
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
  </div>
</aside>

<style>
  aside {
    width: 260px;
    min-width: 260px;
    border-right: 1px solid var(--border);
    display: flex;
    height: 100vh;
  }
  .rail {
    width: 40px;
    min-width: 40px;
    background: var(--bg-alt);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 10px 0;
    gap: 4px;
  }
  .rail button {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
    font-size: 14px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .rail button:hover {
    color: var(--text);
  }
  .rail button.active {
    background: var(--accent);
    color: white;
  }
  .rail .spacer {
    flex: 1;
  }
  .panel {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    padding: 12px 12px 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
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
    flex-shrink: 0;
  }
  .tag-select {
    width: 100%;
    font-size: 12px;
    padding: 4px 6px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .trash-hint {
    padding: 16px;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
