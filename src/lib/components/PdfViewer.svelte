<script lang="ts">
  import { onDestroy, createEventDispatcher } from "svelte";
  import type { PdfFolder, PdfMeta } from "../types";
  import { pdfsApi } from "../api";

  export let pdf: PdfMeta;
  export let folders: PdfFolder[] = [];
  export let tagOptions: string[] = [];

  const dispatch = createEventDispatcher<{
    rename: { id: string; title: string };
    complete: { id: string; completed: boolean };
    "set-tags": { id: string; tags: string[] };
    "move-to-folder": { id: string; folderId: string | null };
  }>();

  let tagDraft = "";

  function addTag() {
    const t = tagDraft.trim().replace(/,$/, "");
    tagDraft = "";
    if (!t || pdf.tags.includes(t)) return;
    dispatch("set-tags", { id: pdf.id, tags: [...pdf.tags, t] });
  }

  function removeTag(t: string) {
    dispatch("set-tags", { id: pdf.id, tags: pdf.tags.filter((x) => x !== t) });
  }

  function changeFolder(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value;
    dispatch("move-to-folder", { id: pdf.id, folderId: value || null });
  }

  let title = pdf.title;
  let renameTimer: ReturnType<typeof setTimeout> | undefined;

  function scheduleRename() {
    if (renameTimer) clearTimeout(renameTimer);
    renameTimer = setTimeout(async () => {
      const trimmed = title.trim();
      if (!trimmed) {
        title = pdf.title;
        return;
      }
      await pdfsApi.rename(pdf.id, trimmed);
      dispatch("rename", { id: pdf.id, title: trimmed });
    }, 500);
  }

  let currentId = "";
  let src = "";
  let loading = true;
  let error: string | null = null;

  $: if (pdf.id !== currentId) {
    currentId = pdf.id;
    title = pdf.title;
    loadPdf(pdf.id);
  }

  async function loadPdf(id: string) {
    loading = true;
    error = null;
    src = "";
    try {
      const url = await pdfsApi.assetUrl(id);
      src = `${url}#page=${pdf.lastPage || 1}`;
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to open PDF";
    } finally {
      loading = false;
    }
  }

  function toggleCompleted() {
    dispatch("complete", { id: pdf.id, completed: !pdf.completed });
  }

  onDestroy(() => {
    if (renameTimer) {
      clearTimeout(renameTimer);
      const trimmed = title.trim();
      if (trimmed && trimmed !== pdf.title) {
        pdfsApi.rename(pdf.id, trimmed).then(() => dispatch("rename", { id: pdf.id, title: trimmed }));
      }
    }
  });
</script>

<div class="viewer">
  <div class="toolbar">
    <input
      class="title-input"
      bind:value={title}
      on:input={scheduleRename}
      title={pdf.title}
      placeholder="Untitled"
    />
    {#if pdf.sourceUrl}
      <span class="source" title={pdf.sourceUrl}>from URL</span>
    {/if}
    <div class="spacer"></div>
    <button class="complete-toggle" class:active={pdf.completed} on:click={toggleCompleted}>
      {pdf.completed ? "✓ Read" : "Mark as read"}
    </button>
  </div>

  <div class="meta-bar">
    <select class="folder-select" value={pdf.folderId ?? ""} on:change={changeFolder}>
      <option value="">No folder</option>
      {#each folders as f (f.id)}
        <option value={f.id}>{f.name}</option>
      {/each}
    </select>

    <div class="tags">
      {#each pdf.tags as tag (tag)}
        <span class="tag-chip">
          {tag}
          <button on:click={() => removeTag(tag)}>×</button>
        </span>
      {/each}
      <input
        class="tag-input"
        list="pdf-tag-options"
        placeholder="+ tag"
        bind:value={tagDraft}
        on:keydown={(e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            addTag();
          }
        }}
      />
      <datalist id="pdf-tag-options">
        {#each tagOptions as t (t)}<option value={t}></option>{/each}
      </datalist>
    </div>
  </div>

  <div class="body">
    {#if loading}
      <p class="status">Loading PDF…</p>
    {:else if error}
      <p class="status error">{error}</p>
    {:else}
      <iframe class="pdf-frame" title={pdf.title} src={src}></iframe>
    {/if}
  </div>
</div>

<style>
  .viewer {
    display: flex;
    flex-direction: column;
    height: 100vh;
    flex: 1;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
  }
  .title-input {
    font-size: 14px;
    font-weight: 600;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
    max-width: 340px;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .title-input:hover,
  .title-input:focus {
    background: var(--bg-alt);
  }
  .source {
    font-size: 11px;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1px 8px;
  }
  .spacer {
    flex: 1;
  }
  .meta-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 16px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .folder-select {
    font-size: 12px;
    padding: 3px 6px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .tags {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
    flex: 1;
  }
  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 2px 4px 2px 8px;
    font-size: 11px;
  }
  .tag-chip button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 13px;
    padding: 0 4px;
  }
  .tag-input {
    font-size: 12px;
    padding: 4px 8px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    background: none;
    color: var(--text);
    width: 80px;
  }
  .complete-toggle {
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 12px;
  }
  .complete-toggle.active {
    background: #2e7d32;
    color: white;
    border-color: #2e7d32;
  }
  .body {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .pdf-frame {
    flex: 1;
    width: 100%;
    height: 100%;
    border: none;
    background: var(--bg);
  }
  .status {
    color: var(--text-muted);
    margin-top: 40px;
    margin-left: auto;
    margin-right: auto;
  }
  .status.error {
    color: var(--danger);
  }
</style>
