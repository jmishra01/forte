<script lang="ts">
  import type { PdfFolder, PdfMeta } from "../types";
  import { createEventDispatcher } from "svelte";

  export let pdfs: PdfMeta[] = [];
  export let folders: PdfFolder[] = [];
  export let selectedId: string | null = null;

  const dispatch = createEventDispatcher<{
    select: string;
    "add-file": void;
    "add-url": void;
    delete: string;
    rename: { id: string; title: string };
    "create-folder": void;
    "rename-folder": { id: string; name: string };
    "delete-folder": string;
    "move-to-folder": { id: string; folderId: string | null };
  }>();

  let renamingId: string | null = null;
  let renameDraft = "";

  let renamingFolderId: string | null = null;
  let renameFolderDraft = "";

  let collapsed = new Set<string>();

  function toggleFolder(id: string) {
    const next = new Set(collapsed);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    collapsed = next;
  }

  function startRename(pdf: PdfMeta) {
    renamingId = pdf.id;
    renameDraft = pdf.title;
  }

  function confirmRename() {
    if (!renamingId) return;
    const title = renameDraft.trim();
    if (title) dispatch("rename", { id: renamingId, title });
    renamingId = null;
  }

  function cancelRename() {
    renamingId = null;
  }

  function startRenameFolder(folder: PdfFolder) {
    renamingFolderId = folder.id;
    renameFolderDraft = folder.name;
  }

  function confirmRenameFolder() {
    if (!renamingFolderId) return;
    const name = renameFolderDraft.trim();
    if (name) dispatch("rename-folder", { id: renamingFolderId, name });
    renamingFolderId = null;
  }

  function cancelRenameFolder() {
    renamingFolderId = null;
  }

  function focusRenameInput(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  $: groups = folders.map((folder) => ({
    folder,
    items: pdfs.filter((p) => p.folderId === folder.id),
  }));
  $: unfiled = pdfs.filter((p) => !p.folderId || !folders.some((f) => f.id === p.folderId));
</script>

<div class="panel">
  <div class="actions">
    <button class="new-btn" title="Add PDF file" on:click={() => dispatch("add-file")}>
      <svg viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 13V3" />
        <path d="M6 7l4-4 4 4" />
        <path d="M4 13v2.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V13" />
      </svg>
    </button>
    <button class="new-btn secondary" title="Add from URL" on:click={() => dispatch("add-url")}>
      <svg viewBox="0 0 20 20" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M8.5 11.5l3-3" />
        <path d="M7 13.5L5.5 15a2.5 2.5 0 0 1-3.5-3.5L3.5 10a2.5 2.5 0 0 1 3.5 0" />
        <path d="M13 6.5L14.5 5a2.5 2.5 0 1 1 3.5 3.5L16.5 10a2.5 2.5 0 0 1-3.5 0" />
      </svg>
    </button>
    <button class="new-btn secondary" title="New folder" on:click={() => dispatch("create-folder")}>
      <svg viewBox="0 0 20 20" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
        <path d="M2.5 6a1 1 0 0 1 1-1h4l1.5 2h7a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-12.5a1 1 0 0 1-1-1V6z" />
        <line x1="10" y1="9" x2="10" y2="13" />
        <line x1="8" y1="11" x2="12" y2="11" />
      </svg>
    </button>
  </div>
  <ul>
    {#each groups as group (group.folder.id)}
      <li class="folder-row">
        <button class="folder-header" on:click={() => toggleFolder(group.folder.id)}>
          <span class="chevron">{collapsed.has(group.folder.id) ? "▸" : "▾"}</span>
          {#if renamingFolderId === group.folder.id}
            <input
              class="rename-input"
              bind:value={renameFolderDraft}
              use:focusRenameInput
              on:click|stopPropagation
              on:keydown={(e) => {
                if (e.key === "Enter") confirmRenameFolder();
                if (e.key === "Escape") cancelRenameFolder();
              }}
              on:blur={confirmRenameFolder}
            />
          {:else}
            <span class="folder-name">
              <svg class="folder-icon" viewBox="0 0 20 20" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                <path d="M2.5 6a1 1 0 0 1 1-1h4l1.5 2h7a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-12.5a1 1 0 0 1-1-1V6z" />
              </svg>
              {group.folder.name}
            </span>
            <span class="count">{group.items.length}</span>
          {/if}
        </button>
        <button
          class="rename"
          title="Rename folder"
          on:click={() => startRenameFolder(group.folder)}
        >✎</button>
        <button
          class="delete"
          title="Delete folder"
          on:click={() => dispatch("delete-folder", group.folder.id)}
        >×</button>
      </li>
      {#if !collapsed.has(group.folder.id)}
        {#each group.items as pdf (pdf.id)}
          <li class="pdf-row nested" class:active={pdf.id === selectedId} class:completed={pdf.completed}>
            {#if renamingId === pdf.id}
              <input
                class="rename-input"
                bind:value={renameDraft}
                use:focusRenameInput
                on:keydown={(e) => {
                  if (e.key === "Enter") confirmRename();
                  if (e.key === "Escape") cancelRename();
                }}
                on:blur={confirmRename}
              />
            {:else}
              <button class="row" on:click={() => dispatch("select", pdf.id)}>
                <span class="title">
                  {#if pdf.completed}<span class="done-mark" title="Reading complete">✓</span>{/if}
                  {pdf.title}
                </span>
                <span class="meta">{pdf.source === "url" ? "🔗 URL" : "📄 file"}</span>
                {#if pdf.tags.length}
                  <span class="tag-row">
                    {#each pdf.tags as tag (tag)}<span class="tag-chip">{tag}</span>{/each}
                  </span>
                {/if}
              </button>
            {/if}
            <select
              class="folder-select"
              title="Move to folder"
              value={pdf.folderId ?? ""}
              on:change={(e) =>
                dispatch("move-to-folder", { id: pdf.id, folderId: e.currentTarget.value || null })}
            >
              <option value="">No folder</option>
              {#each folders as f (f.id)}
                <option value={f.id}>{f.name}</option>
              {/each}
            </select>
            <button class="rename" title="Rename" on:click={() => startRename(pdf)}>✎</button>
            <button class="delete" title="Move to trash" on:click={() => dispatch("delete", pdf.id)}>×</button>
          </li>
        {/each}
      {/if}
    {/each}

    {#each unfiled as pdf (pdf.id)}
      <li class="pdf-row" class:active={pdf.id === selectedId} class:completed={pdf.completed}>
        {#if renamingId === pdf.id}
          <input
            class="rename-input"
            bind:value={renameDraft}
            use:focusRenameInput
            on:keydown={(e) => {
              if (e.key === "Enter") confirmRename();
              if (e.key === "Escape") cancelRename();
            }}
            on:blur={confirmRename}
          />
        {:else}
          <button class="row" on:click={() => dispatch("select", pdf.id)}>
            <span class="title">
              {#if pdf.completed}<span class="done-mark" title="Reading complete">✓</span>{/if}
              {pdf.title}
            </span>
            <span class="meta">{pdf.source === "url" ? "🔗 URL" : "📄 file"}</span>
            {#if pdf.tags.length}
              <span class="tag-row">
                {#each pdf.tags as tag (tag)}<span class="tag-chip">{tag}</span>{/each}
              </span>
            {/if}
          </button>
        {/if}
        <select
          class="folder-select"
          title="Move to folder"
          value={pdf.folderId ?? ""}
          on:change={(e) =>
            dispatch("move-to-folder", { id: pdf.id, folderId: e.currentTarget.value || null })}
        >
          <option value="">No folder</option>
          {#each folders as f (f.id)}
            <option value={f.id}>{f.name}</option>
          {/each}
        </select>
        <button class="rename" title="Rename" on:click={() => startRename(pdf)}>✎</button>
        <button class="delete" title="Move to trash" on:click={() => dispatch("delete", pdf.id)}>×</button>
      </li>
    {:else}
      {#if !groups.length}
        <li class="empty">No PDFs yet.</li>
      {/if}
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
    flex-direction: row;
    gap: 6px;
    margin: 10px;
  }
  .new-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--accent);
    color: white;
    font-size: 15px;
    line-height: 1;
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
  li.folder-row {
    margin-top: 6px;
  }
  .folder-header {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    padding: 6px 6px;
    text-align: left;
    min-width: 0;
    font-size: 12px;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .chevron {
    font-size: 10px;
    width: 10px;
  }
  .folder-name {
    flex: 1;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .folder-icon {
    flex-shrink: 0;
  }
  .count {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
  }
  li.pdf-row.nested {
    margin-left: 8px;
  }
  li.active {
    background: var(--bg-alt);
  }
  li.completed {
    background: rgba(46, 125, 50, 0.1);
  }
  li.completed.active {
    background: rgba(46, 125, 50, 0.18);
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
  .rename-input {
    flex: 1;
    font-size: 13px;
    padding: 6px 8px;
    margin: 2px 0;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
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
  .tag-row {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-top: 3px;
  }
  .tag-chip {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 8px;
    background: var(--bg-alt);
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .done-mark {
    color: #2e7d32;
    font-weight: 700;
    margin-right: 4px;
  }
  .folder-select {
    background: none;
    border: none;
    outline: none;
    box-shadow: none;
    color: var(--text-muted);
    font-size: 11px;
    max-width: 70px;
    visibility: hidden;
  }
  li:hover .folder-select {
    visibility: visible;
  }
  .rename,
  .delete {
    background: none;
    border: none;
    outline: none;
    box-shadow: none;
    color: var(--text-muted);
    font-size: 16px;
    line-height: 1;
    padding: 4px 8px;
    visibility: hidden;
  }
  li:hover .rename,
  li:hover .delete {
    visibility: visible;
  }
  .rename:hover {
    color: var(--accent);
  }
  .delete:hover {
    color: var(--danger);
  }
</style>
