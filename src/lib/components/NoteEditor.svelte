<script lang="ts">
  import { createEventDispatcher, onDestroy, tick } from "svelte";
  import { resolveAttachmentImages } from "../markdown";
  import { notesApi } from "../api";
  import { exportNoteToPdf } from "../exportPdf";
  import type { Note, PdfMeta } from "../types";
  import NoteInfoPanel from "./NoteInfoPanel.svelte";
  import InlineMarkdownEditor from "./InlineMarkdownEditor.svelte";

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
  let status: "saved" | "saving" | "dirty" = "saved";
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let currentId = note.id;

  let tagOptions: string[] = [];
  let tagDraft = "";
  let showInfoPanel = false;
  let exporting = false;

  let inlineEditorRef: InlineMarkdownEditor | undefined;
  let exportPreviewEl: HTMLDivElement;

  // Only used to feed the offscreen export-to-PDF snapshot — the visible
  // editor's content is already rich-text HTML, so this is used as-is.
  $: rawHtml = content;
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
    if (!inlineEditorRef) {
      content += text;
      scheduleSave();
      return;
    }
    inlineEditorRef.insertAtCursor(text);
  }

  function handleTitleInput() {
    scheduleSave();
  }

  function handleEditorChange(e: CustomEvent<string>) {
    content = e.detail;
    scheduleSave();
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

  function handleTextColorInput(e: Event) {
    inlineEditorRef?.setTextColor((e.currentTarget as HTMLInputElement).value);
  }

  function handleBackgroundColorInput(e: Event) {
    inlineEditorRef?.setBackgroundColor((e.currentTarget as HTMLInputElement).value);
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
    <input class="title-input" bind:value={title} on:input={handleTitleInput} placeholder="Untitled" />
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

  <div class="format-toolbar">
    <button title="Undo (Ctrl+Z)" on:click={() => inlineEditorRef?.undo()}>⟲</button>
    <button title="Redo (Ctrl+Shift+Z)" on:click={() => inlineEditorRef?.redo()}>⟳</button>
    <span class="sep"></span>
    <button title="Bold (Ctrl+B)" on:click={() => inlineEditorRef?.toggleBold()}><strong>B</strong></button>
    <button title="Italic (Ctrl+I)" on:click={() => inlineEditorRef?.toggleItalic()}><em>I</em></button>
    <button title="Underline" on:click={() => inlineEditorRef?.toggleUnderline()}><u>U</u></button>
    <button title="Strikethrough" on:click={() => inlineEditorRef?.toggleStrikethrough()}><s>S</s></button>
    <button title="Inline code" on:click={() => inlineEditorRef?.toggleInlineCode()}><code>{"</>"}</code></button>
    <span class="sep"></span>
    <label class="color-input-wrap" title="Text color">
      A
      <input type="color" on:input={handleTextColorInput} />
    </label>
    <label class="color-input-wrap bg" title="Background color">
      A
      <input type="color" on:input={handleBackgroundColorInput} />
    </label>
    <span class="sep"></span>
    <button title="Heading 1" on:click={() => inlineEditorRef?.toggleHeading(1)}>H1</button>
    <button title="Heading 2" on:click={() => inlineEditorRef?.toggleHeading(2)}>H2</button>
    <button title="Heading 3" on:click={() => inlineEditorRef?.toggleHeading(3)}>H3</button>
    <span class="sep"></span>
    <button title="Bullet list" on:click={() => inlineEditorRef?.toggleBullet()}>•</button>
    <button title="Numbered list" on:click={() => inlineEditorRef?.toggleOrdered()}>1.</button>
    <button title="Task list" on:click={() => inlineEditorRef?.toggleTask()}>☑</button>
    <button title="Quote" on:click={() => inlineEditorRef?.toggleQuote()}>"</button>
    <span class="sep"></span>
    <button title="Link (Ctrl+K)" on:click={() => inlineEditorRef?.insertLink()}>🔗</button>
    <button title="Code block" on:click={() => inlineEditorRef?.insertCodeBlock()}>{"{ }"}</button>
    <button title="Table" on:click={() => inlineEditorRef?.insertTable()}>⊞</button>
  </div>

  <div class="main-row">
    <div class="editor-pane">
      <InlineMarkdownEditor
        bind:this={inlineEditorRef}
        value={content}
        placeholder="Start writing… Use [[Note Title]] to link other notes."
        on:change={handleEditorChange}
        on:wikilinkClick={(e) => dispatch("open-wikilink", e.detail)}
      />
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
    font-size: 14px;
    font-weight: 600;
    border: none;
    background: none;
    color: var(--text);
    outline: none;
    max-width: 240px;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .title-input:hover,
  .title-input:focus {
    background: var(--bg-alt);
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
  .format-toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 16px;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .format-toolbar button {
    background: none;
    border: 1px solid transparent;
    border-radius: 4px;
    color: var(--text);
    font-size: 13px;
    line-height: 1;
    padding: 6px 8px;
    min-width: 28px;
  }
  .format-toolbar button:hover {
    background: var(--bg-alt);
    border-color: var(--border);
  }
  .format-toolbar .sep {
    width: 1px;
    height: 18px;
    background: var(--border);
    margin: 0 6px;
  }
  .color-input-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 26px;
    border: 1px solid transparent;
    border-radius: 4px;
    font-size: 13px;
    line-height: 1;
    color: var(--text);
    cursor: pointer;
  }
  .color-input-wrap:hover {
    background: var(--bg-alt);
    border-color: var(--border);
  }
  .color-input-wrap.bg {
    text-decoration: underline;
    text-decoration-color: var(--accent);
    text-decoration-thickness: 3px;
  }
  .color-input-wrap input[type="color"] {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    border: none;
    opacity: 0;
    cursor: pointer;
  }
  .main-row {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .editor-pane {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-width: 0;
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
  .export-offscreen :global(h1),
  .export-offscreen :global(h2),
  .export-offscreen :global(h3) {
    line-height: 1.3;
  }
  .export-offscreen :global(pre) {
    background: #f0f0f0;
    padding: 10px 14px;
    border-radius: 6px;
    overflow-x: auto;
  }
  .export-offscreen :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }
  .export-offscreen :global(code:not(pre code)) {
    background: #f0f0f0;
    border: 1px solid #d5d5d5;
    border-radius: 4px;
    padding: 1px 5px;
  }
  .export-offscreen :global(blockquote) {
    margin: 0;
    padding-left: 12px;
    border-left: 3px solid #ccc;
    color: #555;
  }
  .export-offscreen :global(img) {
    max-width: 100%;
  }
</style>
