<script lang="ts">
  import { tick } from "svelte";
  import * as pdfjsLib from "pdfjs-dist";
  import pdfjsWorker from "pdfjs-dist/build/pdf.worker.min.js?url";
  import type { PDFDocumentProxy } from "pdfjs-dist";
  import type { PdfAnnotation, PdfMeta } from "../types";
  import { pdfsApi } from "../api";

  pdfjsLib.GlobalWorkerOptions.workerSrc = pdfjsWorker;

  export let pdf: PdfMeta;

  const HIGHLIGHT_COLOR = "rgba(255, 213, 79, 0.45)";
  const MIN_RECT_SIZE = 0.01;

  let canvas: HTMLCanvasElement;
  let wrapper: HTMLDivElement;
  let doc: PDFDocumentProxy | null = null;
  let pageNum = 1;
  let numPages = 0;
  let scale = 1.2;
  let loading = true;
  let error: string | null = null;
  let currentId = "";
  let renderedWidth = 0;
  let renderedHeight = 0;

  let annotating = false;
  let allAnnotations: PdfAnnotation[] = [];
  let draft: { x: number; y: number; w: number; h: number } | null = null;
  let dragStart: { x: number; y: number } | null = null;
  let pendingRect: { x: number; y: number; w: number; h: number } | null = null;
  let pendingNoteText = "";

  $: pageAnnotations = allAnnotations.filter((a) => a.page === pageNum);

  $: if (pdf.id !== currentId) {
    currentId = pdf.id;
    pageNum = pdf.lastPage || 1;
    loadPdf(pdf.id);
    loadAnnotations(pdf.id);
  }

  async function loadPdf(id: string) {
    loading = true;
    error = null;
    doc = null;
    try {
      const url = await pdfsApi.assetUrl(id);
      const res = await fetch(url);
      if (!res.ok) throw new Error(`Failed to load file (${res.status})`);
      const buf = await res.arrayBuffer();
      doc = await pdfjsLib.getDocument({ data: buf }).promise;
      numPages = doc.numPages;
      if (pageNum > numPages) pageNum = 1;
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to open PDF";
    } finally {
      loading = false;
    }
    if (doc) {
      // The canvas only mounts once `loading` flips to false and Svelte
      // re-renders, so wait a tick before the first render or it's a no-op.
      await tick();
      await renderPage();
    }
  }

  async function loadAnnotations(id: string) {
    allAnnotations = await pdfsApi.listAnnotations(id);
  }

  async function renderPage() {
    if (!doc || !canvas) return;
    const page = await doc.getPage(pageNum);
    const viewport = page.getViewport({ scale });
    const ctx = canvas.getContext("2d")!;
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    renderedWidth = viewport.width;
    renderedHeight = viewport.height;
    await page.render({ canvasContext: ctx, viewport }).promise;
    pdfsApi.updateProgress(pdf.id, pageNum).catch(() => {});
  }

  function goToPage(n: number) {
    if (n < 1 || n > numPages) return;
    pageNum = n;
    draft = null;
    pendingRect = null;
    renderPage();
  }

  function prevPage() {
    goToPage(pageNum - 1);
  }

  function nextPage() {
    goToPage(pageNum + 1);
  }

  function zoomIn() {
    scale = Math.min(scale + 0.2, 4);
    renderPage();
  }

  function zoomOut() {
    scale = Math.max(scale - 0.2, 0.4);
    renderPage();
  }

  function relativePoint(e: MouseEvent) {
    const rect = wrapper.getBoundingClientRect();
    return {
      x: (e.clientX - rect.left) / renderedWidth,
      y: (e.clientY - rect.top) / renderedHeight,
    };
  }

  function handleMouseDown(e: MouseEvent) {
    if (!annotating || pendingRect) return;
    dragStart = relativePoint(e);
    draft = { x: dragStart.x, y: dragStart.y, w: 0, h: 0 };
  }

  function handleMouseMove(e: MouseEvent) {
    if (!annotating || !dragStart) return;
    const p = relativePoint(e);
    draft = {
      x: Math.min(dragStart.x, p.x),
      y: Math.min(dragStart.y, p.y),
      w: Math.abs(p.x - dragStart.x),
      h: Math.abs(p.y - dragStart.y),
    };
  }

  function handleMouseUp() {
    if (!annotating || !dragStart || !draft) return;
    dragStart = null;
    if (draft.w >= MIN_RECT_SIZE && draft.h >= MIN_RECT_SIZE) {
      pendingRect = draft;
      pendingNoteText = "";
    }
    draft = null;
  }

  async function confirmAnnotation() {
    if (!pendingRect) return;
    const rect = pendingRect;
    await pdfsApi.addAnnotation(
      pdf.id,
      pageNum,
      rect.x,
      rect.y,
      rect.w,
      rect.h,
      HIGHLIGHT_COLOR,
      pendingNoteText.trim() || undefined
    );
    pendingRect = null;
    pendingNoteText = "";
    loadAnnotations(pdf.id);
  }

  function cancelAnnotation() {
    pendingRect = null;
    pendingNoteText = "";
  }

  async function removeAnnotation(id: string) {
    await pdfsApi.deleteAnnotation(pdf.id, id);
    loadAnnotations(pdf.id);
  }
</script>

<div class="viewer">
  <div class="toolbar">
    <span class="title" title={pdf.title}>{pdf.title}</span>
    {#if pdf.sourceUrl}
      <span class="source" title={pdf.sourceUrl}>from URL</span>
    {/if}
    <div class="spacer"></div>
    <button
      class="annotate-toggle"
      class:active={annotating}
      on:click={() => (annotating = !annotating)}
    >
      {annotating ? "Done highlighting" : "Highlight"}
    </button>
    <button on:click={prevPage} disabled={pageNum <= 1}>‹</button>
    <span class="page-indicator">{numPages ? `${pageNum} / ${numPages}` : "–"}</span>
    <button on:click={nextPage} disabled={pageNum >= numPages}>›</button>
    <button on:click={zoomOut}>−</button>
    <span class="zoom">{Math.round(scale * 100)}%</span>
    <button on:click={zoomIn}>+</button>
  </div>

  <div class="body">
    <div class="canvas-area">
      {#if loading}
        <p class="status">Loading PDF…</p>
      {:else if error}
        <p class="status error">{error}</p>
      {:else}
        <div
          class="page-wrap"
          class:annotating
          role="img"
          aria-label="PDF page {pageNum}"
          bind:this={wrapper}
          style="width:{renderedWidth}px; height:{renderedHeight}px;"
          on:mousedown={handleMouseDown}
          on:mousemove={handleMouseMove}
          on:mouseup={handleMouseUp}
        >
          <canvas bind:this={canvas}></canvas>
          {#each pageAnnotations as a (a.id)}
            <div
              class="highlight"
              style="left:{a.x * renderedWidth}px; top:{a.y * renderedHeight}px; width:{a.w * renderedWidth}px; height:{a.h * renderedHeight}px; background:{a.color};"
              title={a.note ?? ""}
            ></div>
          {/each}
          {#if draft}
            <div
              class="highlight draft"
              style="left:{draft.x * renderedWidth}px; top:{draft.y * renderedHeight}px; width:{draft.w * renderedWidth}px; height:{draft.h * renderedHeight}px;"
            ></div>
          {/if}
          {#if pendingRect}
            <div
              class="annotation-form"
              style="left:{pendingRect.x * renderedWidth}px; top:{(pendingRect.y + pendingRect.h) * renderedHeight + 6}px;"
            >
              <input
                type="text"
                placeholder="Optional note…"
                bind:value={pendingNoteText}
                on:keydown={(e) => e.key === "Enter" && confirmAnnotation()}
              />
              <div class="annotation-form-actions">
                <button on:click={cancelAnnotation}>Cancel</button>
                <button class="save" on:click={confirmAnnotation}>Save</button>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    {#if pageAnnotations.length}
      <div class="annotations-list">
        <h4>Highlights on this page</h4>
        {#each pageAnnotations as a (a.id)}
          <div class="annotation-item">
            <span class="swatch" style="background:{a.color};"></span>
            <span class="note-text">{a.note || "(no note)"}</span>
            <button on:click={() => removeAnnotation(a.id)}>×</button>
          </div>
        {/each}
      </div>
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
  .title {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
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
  .toolbar button {
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text);
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 14px;
  }
  .toolbar button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .annotate-toggle {
    font-size: 12px !important;
  }
  .annotate-toggle.active {
    background: var(--accent) !important;
    color: white !important;
    border-color: var(--accent) !important;
  }
  .page-indicator,
  .zoom {
    font-size: 12px;
    color: var(--text-muted);
    min-width: 50px;
    text-align: center;
  }
  .body {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .canvas-area {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 20px;
    background: var(--bg);
  }
  .page-wrap {
    position: relative;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.25);
    height: fit-content;
  }
  .page-wrap.annotating {
    cursor: crosshair;
  }
  canvas {
    display: block;
  }
  .highlight {
    position: absolute;
    pointer-events: none;
    border-radius: 2px;
  }
  .highlight.draft {
    background: rgba(45, 90, 168, 0.25);
    border: 1px dashed var(--accent);
  }
  .annotation-form {
    position: absolute;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 200px;
    z-index: 5;
  }
  .annotation-form input {
    font-size: 12px;
    padding: 5px 7px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .annotation-form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .annotation-form-actions button {
    font-size: 11px;
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .annotation-form-actions button.save {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .annotations-list {
    width: 220px;
    min-width: 220px;
    border-left: 1px solid var(--border);
    padding: 12px;
    overflow-y: auto;
  }
  .annotations-list h4 {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .annotation-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .note-text {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .annotation-item button {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 15px;
  }
  .annotation-item button:hover {
    color: var(--danger);
  }
  .status {
    color: var(--text-muted);
    margin-top: 40px;
  }
  .status.error {
    color: var(--danger);
  }
</style>
