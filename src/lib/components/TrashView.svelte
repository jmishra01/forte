<script lang="ts">
  import { onMount, createEventDispatcher } from "svelte";
  import { notesApi, pdfsApi } from "../api";
  import type { NoteMeta, PdfMeta } from "../types";

  const dispatch = createEventDispatcher<{ changed: void }>();

  let trashedNotes: NoteMeta[] = [];
  let trashedPdfs: PdfMeta[] = [];

  onMount(refresh);

  async function refresh() {
    trashedNotes = await notesApi.listTrashed();
    trashedPdfs = await pdfsApi.listTrashed();
  }

  async function restoreNote(id: string) {
    await notesApi.restore(id);
    await refresh();
    dispatch("changed");
  }

  async function deleteNoteForever(id: string) {
    if (!confirm("Permanently delete this note? This cannot be undone.")) return;
    await notesApi.permanentlyDelete(id);
    await refresh();
    dispatch("changed");
  }

  async function restorePdf(id: string) {
    await pdfsApi.restore(id);
    await refresh();
    dispatch("changed");
  }

  async function deletePdfForever(id: string) {
    if (!confirm("Permanently delete this PDF? This cannot be undone.")) return;
    await pdfsApi.permanentlyDelete(id);
    await refresh();
    dispatch("changed");
  }

  async function emptyAll() {
    if (!trashedNotes.length && !trashedPdfs.length) return;
    if (!confirm("Empty trash? All trashed notes and PDFs will be permanently deleted.")) return;
    await notesApi.emptyTrash();
    await pdfsApi.emptyTrash();
    await refresh();
    dispatch("changed");
  }
</script>

<div class="trash">
  <div class="header">
    <h3>Trash</h3>
    <button on:click={emptyAll} disabled={!trashedNotes.length && !trashedPdfs.length}>Empty trash</button>
  </div>

  <section>
    <h4>Notes</h4>
    {#if trashedNotes.length}
      <ul>
        {#each trashedNotes as n (n.id)}
          <li>
            <span class="name">{n.title || "Untitled"}</span>
            <div class="actions">
              <button on:click={() => restoreNote(n.id)}>Restore</button>
              <button class="danger" on:click={() => deleteNoteForever(n.id)}>Delete forever</button>
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No trashed notes.</p>
    {/if}
  </section>

  <section>
    <h4>PDFs</h4>
    {#if trashedPdfs.length}
      <ul>
        {#each trashedPdfs as p (p.id)}
          <li>
            <span class="name">{p.title}</span>
            <div class="actions">
              <button on:click={() => restorePdf(p.id)}>Restore</button>
              <button class="danger" on:click={() => deletePdfForever(p.id)}>Delete forever</button>
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No trashed PDFs.</p>
    {/if}
  </section>
</div>

<style>
  .trash {
    flex: 1;
    padding: 24px 32px;
    overflow-y: auto;
    max-width: 640px;
  }
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  .header h3 {
    margin: 0;
  }
  .header button {
    font-size: 12px;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--danger);
    background: none;
    color: var(--danger);
  }
  .header button:disabled {
    opacity: 0.4;
    border-color: var(--border);
    color: var(--text-muted);
  }
  section {
    margin-bottom: 24px;
  }
  h4 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin: 0 0 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .actions button {
    font-size: 12px;
    padding: 4px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg-alt);
    color: var(--text);
  }
  .actions button.danger {
    color: var(--danger);
    border-color: var(--danger);
  }
  .muted {
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
