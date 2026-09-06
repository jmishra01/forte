<script lang="ts">
  import { onDestroy, onMount, tick, createEventDispatcher } from "svelte";
  import DOMPurify from "dompurify";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { migrateLegacyContent } from "../markdown";
  import { wireTable } from "../editor/htmlTable";
  import * as cmds from "../editor/formattingCommands";

  export let value = "";
  export let placeholder = "";

  const dispatch = createEventDispatcher<{ change: string; wikilinkClick: string }>();

  let container: HTMLDivElement;

  let linkDialogOpen = false;
  let linkUrl = "";
  let linkText = "";
  let linkUrlInputEl: HTMLInputElement | undefined;
  let pendingLinkRange: Range | null = null;

  // Toolbar buttons live outside the contenteditable region, so clicking one
  // can shift focus/selection away before its command runs (behavior here
  // varies across webviews). Track the last real selection made inside the
  // editor so every command can restore it first, regardless of what's
  // focused at the moment the button is clicked.
  let savedRange: Range | null = null;

  function trackSelection() {
    const sel = document.getSelection();
    if (!sel || sel.rangeCount === 0 || !container.contains(sel.anchorNode)) return;
    savedRange = sel.getRangeAt(0).cloneRange();
  }

  function restoreSelection() {
    if (!savedRange) return;
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(savedRange);
  }

  function withSelection(fn: () => void) {
    if (!container) return;
    // Only fall back to the last-tracked range when the live selection has
    // actually moved outside the editor (e.g. focus jumped to a toolbar
    // control) — if it's still anchored inside, it's more current than
    // anything `selectionchange` has had a chance to record, and clobbering
    // it would silently apply the command to the wrong text.
    const sel = window.getSelection();
    const liveSelectionValid = !!sel && sel.rangeCount > 0 && !!sel.anchorNode && container.contains(sel.anchorNode);
    if (!liveSelectionValid) restoreSelection();
    fn();
  }

  // Undo/redo history. The browser's native contenteditable undo can't be
  // relied on here: several edits (tables, wikilinks, checkboxes, links)
  // mutate the DOM directly rather than through execCommand, so the native
  // stack never sees them, and `container.innerHTML = ...` on load wipes it
  // anyway. This is a small snapshot stack instead — rapid edits within a
  // short window collapse into a single undo step, the same way most
  // editors group fast typing rather than undoing one character at a time.
  const HISTORY_GROUP_MS = 500;
  const HISTORY_LIMIT = 100;
  let undoStack: string[] = [];
  let redoStack: string[] = [];
  let historyCurrent = "";
  let lastEditAt = 0;
  let isApplyingHistory = false;

  function initHistory(html: string) {
    undoStack = [];
    redoStack = [];
    historyCurrent = html;
    lastEditAt = 0;
  }

  function recordHistory(html: string) {
    if (isApplyingHistory || html === historyCurrent) return;
    const now = Date.now();
    if (!lastEditAt || now - lastEditAt >= HISTORY_GROUP_MS) {
      undoStack.push(historyCurrent);
      if (undoStack.length > HISTORY_LIMIT) undoStack.shift();
      redoStack = [];
    }
    historyCurrent = html;
    lastEditAt = now;
  }

  function placeCursorAtEnd() {
    const range = document.createRange();
    range.selectNodeContents(container);
    range.collapse(false);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
  }

  // Restoring a snapshot replaces the whole document, so (like initial
  // load) any tables in it need rewiring, and there's no meaningful DOM
  // position to map an old cursor offset onto — the caret just goes to the
  // end, a predictable spot rather than an attempt at precise restoration.
  function restoreHistorySnapshot(html: string) {
    isApplyingHistory = true;
    historyCurrent = html;
    lastEditAt = 0;
    container.innerHTML = html;
    container.querySelectorAll("table").forEach((table) => {
      if (!table.closest(".cm-html-table-widget")) wireTable(table, notifyChange);
    });
    container.focus();
    placeCursorAtEnd();
    dispatch("change", html);
    isApplyingHistory = false;
  }

  export function undo() {
    if (!undoStack.length) return;
    redoStack.push(historyCurrent);
    const prev = undoStack.pop()!;
    restoreHistorySnapshot(prev);
  }

  export function redo() {
    if (!redoStack.length) return;
    undoStack.push(historyCurrent);
    const next = redoStack.pop()!;
    restoreHistorySnapshot(next);
  }

  onMount(() => {
    loadContent(value);
    container.addEventListener("input", handleInput);
    container.addEventListener("paste", handlePaste);
    container.addEventListener("click", handleClick);
    container.addEventListener("contextmenu", handleContextMenu);
    container.addEventListener("change", handleCheckboxChange);
    container.addEventListener("keydown", handleKeydown);
    document.addEventListener("selectionchange", trackSelection);
  });

  onDestroy(() => {
    container?.removeEventListener("input", handleInput);
    container?.removeEventListener("paste", handlePaste);
    container?.removeEventListener("click", handleClick);
    container?.removeEventListener("contextmenu", handleContextMenu);
    container?.removeEventListener("change", handleCheckboxChange);
    container?.removeEventListener("keydown", handleKeydown);
    document.removeEventListener("selectionchange", trackSelection);
  });

  function loadContent(raw: string) {
    const html = migrateLegacyContent(raw) || "";
    container.innerHTML = html;
    container.querySelectorAll("table").forEach((table) => {
      if (!table.closest(".cm-html-table-widget")) wireTable(table, notifyChange);
    });
    initHistory(html);
  }

  function notifyChange() {
    const html = serializeForSave();
    recordHistory(html);
    dispatch("change", html);
  }

  // Tables carry their own hover-overlay UI (row/col insert & delete, the
  // align popup) marked with `data-ui`, live in the DOM only — never part of
  // the saved content. Saving unwraps each table back to plain `<table>`.
  function serializeForSave(): string {
    const clone = container.cloneNode(true) as HTMLElement;
    clone.querySelectorAll("[data-ui]").forEach((el) => el.remove());
    clone.querySelectorAll(".cm-html-table-widget").forEach((wrapper) => {
      const table = wrapper.querySelector("table");
      if (table) wrapper.replaceWith(table);
      else wrapper.remove();
    });
    return DOMPurify.sanitize(clone.innerHTML, { ADD_ATTR: ["data-wikilink", "contenteditable"] });
  }

  function handleInput(e: Event) {
    const data = (e as InputEvent).data;
    if (data === "]") cmds.linkifyWikilinkAtCursor(container);
    notifyChange();
  }

  function handlePaste(e: ClipboardEvent) {
    e.preventDefault();
    const html = e.clipboardData?.getData("text/html");
    const text = e.clipboardData?.getData("text/plain") ?? "";
    const clean = html
      ? DOMPurify.sanitize(html, { ADD_ATTR: ["data-wikilink", "contenteditable"] })
      : text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\n/g, "<br>");
    document.execCommand("insertHTML", false, clean);
  }

  function handleClick(e: MouseEvent) {
    const target = e.target as HTMLElement;

    const wikilink = target.closest(".wikilink") as HTMLElement | null;
    if (wikilink) {
      e.preventDefault();
      dispatch("wikilinkClick", wikilink.dataset.wikilink ?? "");
      return;
    }

    // A plain <a> inside the contenteditable would otherwise navigate the
    // app's own webview to that URL — open it in the system's default
    // browser instead, the same way any other note-taking app treats links.
    const anchor = target.closest("a[href]") as HTMLAnchorElement | null;
    if (anchor && container.contains(anchor)) {
      e.preventDefault();
      openUrl(anchor.href).catch((err) => console.error("Failed to open link", err));
    }
  }

  // The webview's native context menu on a link offers things like "Copy
  // Link" / "Open Link in New Window" that don't make sense here (there's
  // no in-app browsing, and links already open externally on click) — so
  // just suppress it over links, leaving the rest of the editor's own
  // right-click behavior (cut/copy/paste elsewhere) untouched.
  function handleContextMenu(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest("a")) e.preventDefault();
  }

  // Checkbox `checked` is a live DOM property, not reflected back into the
  // HTML attribute automatically — sync it manually or a task's checked
  // state would silently be lost on the next save/reload.
  function handleCheckboxChange(e: Event) {
    const target = e.target as HTMLElement;
    if (!(target instanceof HTMLInputElement) || target.type !== "checkbox") return;
    if (target.checked) target.setAttribute("checked", "");
    else target.removeAttribute("checked");
    notifyChange();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;
    const key = e.key.toLowerCase();
    if (key === "z" && e.shiftKey) {
      e.preventDefault();
      redo();
    } else if (key === "z") {
      e.preventDefault();
      undo();
    } else if (key === "y") {
      e.preventDefault();
      redo();
    } else if (key === "b") {
      e.preventDefault();
      cmds.toggleBold(container);
    } else if (key === "i") {
      e.preventDefault();
      cmds.toggleItalic(container);
    } else if (key === "k") {
      e.preventDefault();
      openLinkDialog();
    }
  }

  // Captures the current selection (if any, so its text can prefill the
  // "display text" field) before the dialog's own inputs steal focus, then
  // shows the dialog. Confirming applies the link to that saved range.
  function openLinkDialog() {
    const sel = window.getSelection();
    const hasSelection = !!sel && sel.rangeCount > 0 && container.contains(sel.anchorNode);
    pendingLinkRange = hasSelection ? sel!.getRangeAt(0).cloneRange() : null;
    linkText = pendingLinkRange ? pendingLinkRange.toString() : "";
    linkUrl = "";
    linkDialogOpen = true;
    tick().then(() => linkUrlInputEl?.focus());
  }

  function confirmLinkDialog() {
    const url = linkUrl.trim();
    if (!url) return;
    cmds.applyLink(container, pendingLinkRange, url, linkText.trim());
    closeLinkDialog();
  }

  function cancelLinkDialog() {
    closeLinkDialog();
  }

  function closeLinkDialog() {
    // Once the dialog's own <input> takes focus, `document.getSelection()`
    // stops reflecting the editor at all — it can't be used to recover the
    // caret position here. `pendingLinkRange` is the reliable source: it's
    // either still the original pre-dialog range (cancel), or was mutated
    // in place by `applyLink` to sit right after the newly inserted link
    // (confirm) — either way it's exactly where focus should return to.
    const toRestore = pendingLinkRange;

    linkDialogOpen = false;
    pendingLinkRange = null;
    linkUrl = "";
    linkText = "";
    container?.focus();

    if (toRestore) {
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(toRestore);
    }
  }

  function handleLinkDialogKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      cancelLinkDialog();
    } else if (e.key === "Enter") {
      e.preventDefault();
      confirmLinkDialog();
    }
  }

  export function insertAtCursor(text: string) {
    if (!container) return;
    withSelection(() => {
      container.focus();
      document.execCommand("insertText", false, text);
    });
    notifyChange();
  }

  export function focusEditor() {
    container?.focus();
  }

  export function toggleBold() {
    withSelection(() => cmds.toggleBold(container));
  }
  export function toggleItalic() {
    withSelection(() => cmds.toggleItalic(container));
  }
  export function toggleUnderline() {
    withSelection(() => cmds.toggleUnderline(container));
  }
  export function toggleStrikethrough() {
    withSelection(() => cmds.toggleStrikethrough(container));
  }
  export function toggleInlineCode() {
    withSelection(() => cmds.toggleInlineCode(container));
  }
  export function toggleHeading(level: number) {
    withSelection(() => cmds.toggleHeading(container, level));
  }
  export function toggleQuote() {
    withSelection(() => cmds.toggleQuote(container));
  }
  export function toggleBullet() {
    withSelection(() => cmds.toggleBullet(container));
  }
  export function toggleOrdered() {
    withSelection(() => cmds.toggleOrdered(container));
  }
  export function toggleTask() {
    withSelection(() => cmds.toggleTask(container));
  }
  export function insertLink() {
    withSelection(() => openLinkDialog());
  }
  export function insertCodeBlock() {
    withSelection(() => cmds.insertCodeBlock(container));
  }
  export function insertTable() {
    withSelection(() => cmds.insertTable(container));
  }
  export function setTextColor(color: string) {
    withSelection(() => cmds.setTextColor(container, color));
  }
  export function setBackgroundColor(color: string) {
    withSelection(() => cmds.setBackgroundColor(container, color));
  }
</script>

<div
  class="cm-host"
  bind:this={container}
  contenteditable="true"
  data-placeholder={placeholder}
  role="textbox"
  aria-multiline="true"
  tabindex="0"
></div>

{#if linkDialogOpen}
  <div class="link-dialog-backdrop" role="presentation" on:mousedown={cancelLinkDialog}>
    <div
      class="link-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Insert link"
      on:mousedown|stopPropagation
      on:keydown={handleLinkDialogKeydown}
    >
      <label class="link-dialog-field">
        <span>URL</span>
        <input type="url" bind:value={linkUrl} bind:this={linkUrlInputEl} placeholder="https://" />
      </label>
      <label class="link-dialog-field">
        <span>Display text</span>
        <input type="text" bind:value={linkText} placeholder="Link text" />
      </label>
      <div class="link-dialog-actions">
        <button type="button" on:click={cancelLinkDialog}>Cancel</button>
        <button type="button" class="primary" on:click={confirmLinkDialog} disabled={!linkUrl.trim()}>Insert</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .cm-host {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    font-size: 14px;
    line-height: 1.6;
    padding: 16px 24px;
    outline: none;
    box-sizing: border-box;
  }
  .cm-host:empty::before {
    content: attr(data-placeholder);
    color: var(--text-muted);
    pointer-events: none;
  }
  .cm-host :global(h1),
  .cm-host :global(h2),
  .cm-host :global(h3),
  .cm-host :global(h4),
  .cm-host :global(h5),
  .cm-host :global(h6) {
    font-weight: 700;
    line-height: 1.3;
    margin: 0.6em 0 0.3em;
  }
  .cm-host :global(h1) {
    font-size: 1.6em;
  }
  .cm-host :global(h2) {
    font-size: 1.4em;
  }
  .cm-host :global(h3) {
    font-size: 1.2em;
  }
  .cm-host :global(h4),
  .cm-host :global(h5),
  .cm-host :global(h6) {
    font-size: 1.05em;
  }
  .cm-host :global(p) {
    margin: 0.4em 0;
  }
  .cm-host :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.92em;
  }
  .cm-host :global(pre) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.92em;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 12px;
    overflow-x: auto;
    margin: 0.4em 0;
  }
  .cm-host :global(pre code) {
    background: none;
    border: none;
    padding: 0;
  }
  .cm-host :global(blockquote) {
    margin: 0.4em 0;
    padding-left: 12px;
    border-left: 3px solid var(--border);
    color: var(--text-muted);
  }
  .cm-host :global(ul),
  .cm-host :global(ol) {
    margin: 0.4em 0;
    padding-left: 1.6em;
  }
  .cm-host :global(ul.task-list) {
    list-style: none;
    padding-left: 1.2em;
  }
  .cm-host :global(ul.task-list li) {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .cm-host :global(a.wikilink) {
    color: var(--accent);
    text-decoration: underline dotted;
    cursor: pointer;
    border-radius: 3px;
    padding: 0 2px;
  }
  .cm-host :global(a.wikilink:hover) {
    background: var(--bg-alt);
  }
  .cm-host :global(.cm-html-table-widget) {
    position: relative;
    margin: 8px 0;
  }
  .cm-host :global(.cm-table-scroll) {
    overflow-x: auto;
    max-width: 100%;
  }
  .cm-host :global(.cm-table-edit) {
    border-collapse: collapse;
    font-size: 0.95em;
    table-layout: fixed;
  }
  .cm-host :global(.cm-table-edit th),
  .cm-host :global(.cm-table-edit td) {
    position: relative;
    border: 1px solid var(--border);
    padding: 6px 12px;
    min-width: 80px;
    max-width: 320px;
    white-space: normal;
    overflow-wrap: break-word;
    vertical-align: top;
  }
  .cm-host :global(.cm-table-edit thead th) {
    background: var(--bg-alt);
    font-weight: 600;
    padding-right: 22px;
  }
  .cm-host :global(.cm-table-edit ul) {
    margin: 0;
    padding-left: 1.2em;
  }
  .cm-host :global(.cm-col-handle) {
    position: absolute;
    top: 2px;
    right: 2px;
    background: none;
    border: 1px solid transparent;
    border-radius: 3px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1;
    padding: 1px 3px;
    visibility: hidden;
  }
  .cm-host :global(.cm-table-edit th:hover .cm-col-handle) {
    visibility: visible;
  }
  .cm-host :global(.cm-col-handle:hover) {
    background: var(--bg-alt);
    border-color: var(--border);
    color: var(--text);
  }
  .cm-host :global(.cm-table-row-overlays) {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .cm-host :global(.cm-row-zone) {
    position: absolute;
    pointer-events: auto;
  }
  .cm-host :global(.cm-row-zone-delete),
  .cm-host :global(.cm-row-zone-table) {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    border-radius: 4px;
  }
  .cm-host :global(.cm-row-zone-table:hover) {
    background: var(--bg-alt);
  }
  .cm-host :global(.cm-row-delete-btn),
  .cm-host :global(.cm-table-handle) {
    visibility: hidden;
    background: none;
    border: 1px solid transparent;
    border-radius: 3px;
    font-size: 11px;
    line-height: 1;
    padding: 3px 4px;
  }
  .cm-host :global(.cm-row-delete-btn) {
    color: var(--danger);
  }
  .cm-host :global(.cm-table-handle) {
    color: var(--text-muted);
    cursor: pointer;
  }
  .cm-host :global(.cm-row-zone-delete:hover .cm-row-delete-btn),
  .cm-host :global(.cm-row-zone-table:hover .cm-table-handle) {
    visibility: visible;
  }
  .cm-host :global(.cm-row-delete-btn:hover) {
    background: var(--danger);
    color: white;
  }
  .cm-host :global(.cm-table-handle:hover) {
    background: var(--border);
    color: var(--text);
  }
  .cm-host :global(.cm-row-zone-insert) {
    display: flex;
    align-items: center;
  }
  .cm-host :global(.cm-row-zone-insert::after) {
    content: "";
    position: absolute;
    left: 20px;
    right: 0;
    top: 50%;
    height: 2px;
    margin-top: -1px;
    background: var(--accent);
    opacity: 0;
  }
  .cm-host :global(.cm-row-zone-insert:hover::after) {
    opacity: 1;
  }
  .cm-host :global(.cm-row-insert-btn) {
    visibility: hidden;
    position: relative;
    z-index: 1;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1;
    padding: 0;
  }
  .cm-host :global(.cm-row-zone-insert:hover .cm-row-insert-btn) {
    visibility: visible;
    border-color: var(--accent);
    color: var(--accent);
  }
  .cm-host :global(.cm-col-zone-delete) {
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }
  .cm-host :global(.cm-col-delete-btn) {
    visibility: hidden;
    background: var(--danger);
    border: 1px solid transparent;
    border-radius: 3px;
    color: white;
    font-size: 11px;
    line-height: 1;
    padding: 3px 4px;
  }
  .cm-host :global(.cm-col-zone-delete:hover .cm-col-delete-btn) {
    visibility: visible;
  }
  .cm-host :global(.cm-col-zone-insert) {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .cm-host :global(.cm-col-zone-insert::after) {
    content: "";
    position: absolute;
    top: 20px;
    bottom: 0;
    left: 50%;
    width: 2px;
    margin-left: -1px;
    background: var(--accent);
    opacity: 0;
  }
  .cm-host :global(.cm-col-zone-insert:hover::after) {
    opacity: 1;
  }
  .cm-host :global(.cm-col-insert-btn) {
    visibility: hidden;
    position: relative;
    z-index: 1;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1;
    padding: 0;
  }
  .cm-host :global(.cm-col-zone-insert:hover .cm-col-insert-btn) {
    visibility: visible;
    border-color: var(--accent);
    color: var(--accent);
  }
  .cm-host :global(.cm-table-edit tr.cm-row-highlight > td),
  .cm-host :global(.cm-table-edit tr.cm-row-highlight > th) {
    background: color-mix(in srgb, var(--danger) 14%, var(--bg));
  }
  .cm-host :global(.cm-table-edit td.cm-col-highlight),
  .cm-host :global(.cm-table-edit th.cm-col-highlight) {
    background: color-mix(in srgb, var(--danger) 14%, var(--bg));
  }
  .cm-host :global(.cm-table-popup) {
    position: absolute;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.18);
    padding: 4px;
    gap: 1px;
  }
  .cm-host :global(.cm-table-popup[hidden]) {
    display: none;
  }
  .cm-host :global(.cm-table-popup-row) {
    display: flex;
    gap: 1px;
  }
  .cm-host :global(.cm-table-popup button) {
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    padding: 4px 6px;
    flex: 1;
  }
  .cm-host :global(.cm-table-popup button:hover) {
    background: var(--bg);
  }
  .cm-host :global(.cm-table-popup button.active) {
    background: var(--accent);
    color: white;
  }
  .cm-host :global(.cm-table-popup button[data-action="delete-table"]) {
    color: var(--danger);
  }
  .cm-host :global(.cm-table-popup button[data-action="delete-table"]:hover) {
    background: var(--danger);
    color: white;
  }

  .link-dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .link-dialog {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 320px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    padding: 16px;
  }
  .link-dialog-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .link-dialog-field input {
    font-size: 13px;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg-alt);
    color: var(--text);
  }
  .link-dialog-field input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .link-dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .link-dialog-actions button {
    font-size: 12px;
    padding: 6px 12px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text);
  }
  .link-dialog-actions button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .link-dialog-actions button.primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
