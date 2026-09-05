<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import type { Note, NoteMeta, NoteVersion, PdfMeta } from "../types";
  import { notesApi } from "../api";

  export let note: Note;
  export let pdfs: PdfMeta[] = [];

  const dispatch = createEventDispatcher<{
    "link-pdf": string;
    "unlink-pdf": string;
    "open-pdf": string;
    "open-note": string;
    restored: Note;
  }>();

  let tab: "pdfs" | "backlinks" | "history" = "pdfs";
  let attachChoice = "";
  let backlinks: NoteMeta[] = [];
  let history: NoteVersion[] = [];
  let previewTimestamp: string | null = null;
  let previewContent = "";
  let loadingPreview = false;

  $: linkedPdfs = pdfs.filter((p) => note.linkedPdfIds.includes(p.id));
  $: availablePdfs = pdfs.filter((p) => !note.linkedPdfIds.includes(p.id));

  onMount(() => {
    loadBacklinks();
    loadHistory();
  });

  async function loadBacklinks() {
    backlinks = await notesApi.backlinks(note.title);
    backlinks = backlinks.filter((n) => n.id !== note.id);
  }

  async function loadHistory() {
    history = await notesApi.history(note.id);
    previewTimestamp = null;
  }

  function attach() {
    if (!attachChoice) return;
    dispatch("link-pdf", attachChoice);
    attachChoice = "";
  }

  async function showVersion(ts: string) {
    previewTimestamp = ts;
    loadingPreview = true;
    const content = await notesApi.historyContent(note.id, ts);
    previewContent = content.content;
    loadingPreview = false;
  }

  async function restoreVersion(ts: string) {
    const restored = await notesApi.restoreVersion(note.id, ts);
    dispatch("restored", restored);
    previewTimestamp = null;
    loadHistory();
  }

  function formatDateTime(iso: string) {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<aside class="info-panel">
  <div class="tabs">
    <button class:active={tab === "pdfs"} on:click={() => (tab = "pdfs")}>PDFs</button>
    <button class:active={tab === "backlinks"} on:click={() => (tab = "backlinks")}>
      Backlinks{backlinks.length ? ` (${backlinks.length})` : ""}
    </button>
    <button class:active={tab === "history"} on:click={() => (tab = "history")}>History</button>
  </div>

  <div class="body">
    {#if tab === "pdfs"}
      <div class="section">
        {#if linkedPdfs.length}
          <ul class="plain-list">
            {#each linkedPdfs as pdf (pdf.id)}
              <li>
                <button class="link" on:click={() => dispatch("open-pdf", pdf.id)}>{pdf.title}</button>
                <button class="small-x" title="Detach" on:click={() => dispatch("unlink-pdf", pdf.id)}>×</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted">No PDFs attached yet.</p>
        {/if}
        {#if availablePdfs.length}
          <div class="attach-row">
            <select bind:value={attachChoice}>
              <option value="">Attach existing PDF…</option>
              {#each availablePdfs as pdf (pdf.id)}
                <option value={pdf.id}>{pdf.title}</option>
              {/each}
            </select>
            <button on:click={attach} disabled={!attachChoice}>Attach</button>
          </div>
        {/if}
      </div>
    {:else if tab === "backlinks"}
      <div class="section">
        {#if backlinks.length}
          <ul class="plain-list">
            {#each backlinks as n (n.id)}
              <li>
                <button class="link" on:click={() => dispatch("open-note", n.id)}>{n.title}</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted">No notes link here yet. Use [[{note.title}]] in another note.</p>
        {/if}
      </div>
    {:else}
      <div class="section history">
        {#if history.length}
          <ul class="plain-list">
            {#each history as v (v.timestamp)}
              <li>
                <button
                  class="link"
                  class:active={previewTimestamp === v.timestamp}
                  on:click={() => showVersion(v.timestamp)}
                >
                  {formatDateTime(v.timestamp)}
                </button>
              </li>
            {/each}
          </ul>
          {#if previewTimestamp}
            <div class="version-preview">
              {#if loadingPreview}
                <p class="muted">Loading…</p>
              {:else}
                <pre>{previewContent}</pre>
                <button class="restore" on:click={() => previewTimestamp && restoreVersion(previewTimestamp)}>
                  Restore this version
                </button>
              {/if}
            </div>
          {/if}
        {:else}
          <p class="muted">No earlier versions yet. Checkpoints are saved automatically as you edit.</p>
        {/if}
      </div>
    {/if}
  </div>
</aside>

<style>
  .info-panel {
    width: 280px;
    min-width: 280px;
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg-alt);
  }
  .tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    flex: 1;
    background: none;
    border: none;
    padding: 10px 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    border-bottom: 2px solid transparent;
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .body {
    flex: 1;
    overflow-y: auto;
  }
  .section {
    padding: 12px;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }
  .plain-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .plain-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent);
    font-size: 13px;
    text-align: left;
    padding: 6px 4px;
    flex: 1;
    border-radius: 4px;
  }
  .link:hover {
    background: var(--bg);
  }
  .link.active {
    background: var(--bg);
    font-weight: 600;
  }
  .small-x {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 15px;
    padding: 2px 6px;
  }
  .small-x:hover {
    color: var(--danger);
  }
  .attach-row {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }
  .attach-row select {
    flex: 1;
    font-size: 12px;
    padding: 5px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .attach-row button {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--accent);
    color: white;
  }
  .attach-row button:disabled {
    opacity: 0.5;
  }
  .version-preview {
    margin-top: 10px;
    border-top: 1px solid var(--border);
    padding-top: 10px;
  }
  .version-preview pre {
    white-space: pre-wrap;
    font-size: 11px;
    max-height: 240px;
    overflow-y: auto;
    background: var(--bg);
    padding: 8px;
    border-radius: 6px;
    margin: 0 0 8px;
  }
  .restore {
    width: 100%;
    padding: 6px;
    border-radius: 5px;
    border: 1px solid var(--accent);
    background: none;
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
  .restore:hover {
    background: var(--accent);
    color: white;
  }
</style>
