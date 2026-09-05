<script lang="ts">
  import { createEventDispatcher, onDestroy, tick } from "svelte";
  import { renderMarkdown, resolveAttachmentImages } from "../markdown";
  import { notesApi } from "../api";
  import { exportNoteToPdf } from "../exportPdf";
  import type { Note, PdfMeta } from "../types";
  import NoteInfoPanel from "./NoteInfoPanel.svelte";

  export let note: Note;
  export let pdfs: PdfMeta[] = [];

  const dispatch = createEventDispatcher<{
    saved: void;
    "open-wikilink": string;
    "open-pdf": string;
    "select-note": string;
  }>();

  let title = note.title;
  let content = note.content;
  let tags = [...note.tags];
  let linkedPdfIds = [...note.linkedPdfIds];
  let mode: "edit" | "split" | "preview" = "split";
  let status: "saved" | "saving" | "dirty" = "saved";
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let currentId = note.id;

  let tagOptions: string[] = [];
  let tagDraft = "";
  let showInfoPanel = false;
  let exporting = false;

  let textareaEl: HTMLTextAreaElement;
  let exportPreviewEl: HTMLDivElement;

  $: rawHtml = renderMarkdown(content);
  let resolvedHtml = "";
  let renderToken = 0;
  $: {
    const token = ++renderToken;
    resolvedHtml = rawHtml;
    resolveAttachmentImages(rawHtml).then((h) => {
      if (token === renderToken) resolvedHtml = h;
    });
  }

  $: currentNoteSnapshot = ({
    ...note,
    title,
    content,
    tags,
    linkedPdfIds,
  } satisfies Note);

  loadOptions();

  async function loadOptions() {
    tagOptions = await notesApi.listTags();
  }

  function scheduleSave() {
    status = "dirty";
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(save, 500);
  }

  async function save() {
    status = "saving";
    await notesApi.update(currentId, title, content);
    status = "saved";
    dispatch("saved");
  }

  export function forceSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = undefined;
      save();
    }
  }

  export async function insertAtCursor(text: string) {
    if (!textareaEl) {
      content += text;
      scheduleSave();
      return;
    }
    const start = textareaEl.selectionStart;
    const end = textareaEl.selectionEnd;
    content = content.slice(0, start) + text + content.slice(end);
    scheduleSave();
    await tick();
    textareaEl.focus();
    textareaEl.selectionStart = textareaEl.selectionEnd = start + text.length;
  }

  function handleInput() {
    scheduleSave();
  }

  function handlePreviewClick(e: MouseEvent) {
    const target = (e.target as HTMLElement).closest("a.wikilink") as HTMLElement | null;
    if (!target) return;
    e.preventDefault();
    const linkTitle = target.getAttribute("data-wikilink");
    if (linkTitle) dispatch("open-wikilink", linkTitle);
  }

  async function addTag() {
    const t = tagDraft.trim().replace(/,$/, "");
    tagDraft = "";
    if (!t || tags.includes(t)) return;
    tags = [...tags, t];
    await notesApi.setTags(currentId, tags);
    dispatch("saved");
    loadOptions();
  }

  async function removeTag(t: string) {
    tags = tags.filter((x) => x !== t);
    await notesApi.setTags(currentId, tags);
    dispatch("saved");
  }

  async function handleLinkPdf(e: CustomEvent<string>) {
    const meta = await notesApi.linkPdf(currentId, e.detail);
    linkedPdfIds = meta.linkedPdfIds;
    dispatch("saved");
  }

  async function handleUnlinkPdf(e: CustomEvent<string>) {
    const meta = await notesApi.unlinkPdf(currentId, e.detail);
    linkedPdfIds = meta.linkedPdfIds;
    dispatch("saved");
  }

  function handleRestored(e: CustomEvent<Note>) {
    title = e.detail.title;
    content = e.detail.content;
    dispatch("saved");
  }

  async function handleExportPdf() {
    if (!exportPreviewEl) return;
    exporting = true;
    try {
      await tick();
      const saved = await exportNoteToPdf(title, exportPreviewEl);
      if (!saved) return;
    } catch (e) {
      alert(e instanceof Error ? `Export failed: ${e.message}` : "Export failed");
    } finally {
      exporting = false;
    }
  }

  onDestroy(() => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      save();
    }
  });
</script>

<div class="editor">
  <div class="toolbar">
    <input class="title-input" bind:value={title} on:input={handleInput} placeholder="Untitled" />
    <div class="mode-switch">
      <button class:active={mode === "edit"} on:click={() => (mode = "edit")}>Edit</button>
      <button class:active={mode === "split"} on:click={() => (mode = "split")}>Split</button>
      <button class:active={mode === "preview"} on:click={() => (mode = "preview")}>Preview</button>
    </div>
    <span class="status">{status === "dirty" ? "Unsaved…" : status === "saving" ? "Saving…" : "Saved"}</span>
  </div>

  <div class="meta-bar">
    <div class="tags">
      {#each tags as tag (tag)}
        <span class="tag-chip">
          {tag}
          <button on:click={() => removeTag(tag)}>×</button>
        </span>
      {/each}
      <input
        class="tag-input"
        list="tag-options"
        placeholder="+ tag"
        bind:value={tagDraft}
        on:keydown={(e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            addTag();
          }
        }}
      />
      <datalist id="tag-options">
        {#each tagOptions as t (t)}<option value={t}></option>{/each}
      </datalist>
    </div>

    <div class="meta-actions">
      <button class:active={showInfoPanel} on:click={() => (showInfoPanel = !showInfoPanel)}>Info</button>
      <button on:click={handleExportPdf} disabled={exporting}>
        {exporting ? "Exporting…" : "Export PDF"}
      </button>
    </div>
  </div>

  <div class="main-row">
    <div class="panes" class:single={mode !== "split"}>
      {#if mode !== "preview"}
        <textarea
          bind:this={textareaEl}
          bind:value={content}
          on:input={handleInput}
          spellcheck="false"
          placeholder="Write markdown here… Use [[Note Title]] to link other notes."
        ></textarea>
      {/if}
      {#if mode !== "edit"}
        <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
        <div class="preview" role="document" on:click={handlePreviewClick}>
          {@html resolvedHtml}
        </div>
      {/if}
    </div>

    {#if showInfoPanel}
      <NoteInfoPanel
        note={currentNoteSnapshot}
        {pdfs}
        on:link-pdf={handleLinkPdf}
        on:unlink-pdf={handleUnlinkPdf}
        on:open-pdf={(e) => dispatch("open-pdf", e.detail)}
        on:open-note={(e) => dispatch("select-note", e.detail)}
        on:restored={handleRestored}
      />
    {/if}
  </div>
</div>

<div class="export-offscreen" bind:this={exportPreviewEl}>
  <h1>{title}</h1>
  {@html resolvedHtml}
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    height: 100vh;
    flex: 1;
    min-width: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
  }
  .title-input {
    flex: 1;
    font-size: 16px;
    font-weight: 600;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
  }
  .mode-switch {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .mode-switch button {
    background: none;
    border: none;
    padding: 5px 10px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .mode-switch button.active {
    background: var(--accent);
    color: white;
  }
  .status {
    font-size: 12px;
    color: var(--text-muted);
    min-width: 70px;
    text-align: right;
  }
  .meta-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 16px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
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
  .meta-actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .meta-actions button {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .meta-actions button.active {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .meta-actions button:disabled {
    opacity: 0.6;
  }
  .main-row {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .panes {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-width: 0;
  }
  .panes.single {
    display: block;
  }
  textarea {
    flex: 1;
    min-width: 0;
    height: 100%;
    resize: none;
    border: none;
    outline: none;
    padding: 16px 20px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 14px;
    line-height: 1.6;
    background: var(--bg);
    color: var(--text);
    border-right: 1px solid var(--border);
  }
  .panes.single textarea {
    border-right: none;
    width: 100%;
  }
  .preview {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow-y: auto;
    padding: 16px 24px;
    line-height: 1.6;
  }
  .panes.single .preview {
    width: 100%;
  }
  .preview :global(h1),
  .preview :global(h2),
  .preview :global(h3) {
    line-height: 1.3;
  }
  .preview :global(pre) {
    background: var(--bg-alt);
    padding: 10px 14px;
    border-radius: 6px;
    overflow-x: auto;
  }
  .preview :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }
  .preview :global(blockquote) {
    margin: 0;
    padding-left: 12px;
    border-left: 3px solid var(--border);
    color: var(--text-muted);
  }
  .preview :global(img) {
    max-width: 100%;
  }
  .preview :global(a.wikilink) {
    color: var(--accent);
    text-decoration: underline dotted;
  }
  .export-offscreen {
    position: fixed;
    top: 0;
    left: -99999px;
    width: 700px;
    padding: 24px;
    background: white;
    color: #1b1b1f;
  }
</style>
