<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, type TrashItem } from "./api";

  const dispatch = createEventDispatcher<{ restored: string; error: string; close: void }>();
  let items: TrashItem[] = [];
  let loaded = false;

  async function load() {
    try {
      items = await api.listTrash();
    } catch (e) {
      dispatch("error", String(e));
    }
    loaded = true;
  }

  async function restore(item: TrashItem) {
    try {
      dispatch("restored", await api.restoreFromTrash(item.name));
      await load();
    } catch (e) {
      dispatch("error", String(e));
    }
  }

  async function deleteForever(item: TrashItem) {
    const ok = await ask(`Delete "${label(item)}" permanently?\n\nThis can't be undone.`, {
      title: "Delete forever",
      kind: "warning",
      okLabel: "Delete forever",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await api.deleteFromTrash(item.name);
      await load();
    } catch (e) {
      dispatch("error", String(e));
    }
  }

  async function emptyAll() {
    const ok = await ask(`Permanently delete all ${items.length} items in the trash?\n\nThis can't be undone.`, {
      title: "Empty trash",
      kind: "warning",
      okLabel: "Empty trash",
      cancelLabel: "Cancel",
    });
    if (!ok) return;
    try {
      await api.emptyTrash();
      await load();
    } catch (e) {
      dispatch("error", String(e));
    }
  }

  const label = (item: TrashItem) => (item.isDir ? item.name : item.name.replace(/\.md$/, ""));

  function when(secs: number | null) {
    if (!secs) return "";
    const d = new Date(secs * 1000);
    const days = Math.floor((Date.now() - d.getTime()) / 86_400_000);
    if (days < 1) return `today ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
    if (days < 2) return "yesterday";
    return d.toLocaleDateString();
  }

  onMount(load);
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="backdrop" on:mousedown|self={() => dispatch("close")} on:keydown={(e) => e.key === "Escape" && dispatch("close")}>
  <div class="dialog" role="dialog" aria-label="Trash">
    <header>
      <h2>Trash</h2>
      {#if items.length}<button class="danger" on:click={emptyAll}>Empty trash</button>{/if}
    </header>
    {#if loaded && !items.length}
      <p class="hint">The trash is empty. Deleted notes and folders go here until you empty it.</p>
    {/if}
    <ul>
      {#each items as item (item.name)}
        <li>
          <div class="what">
            <span class="name">{item.isDir ? "📁 " : ""}{label(item)}</span>
            <span class="meta">
              {#if item.original}from {item.original.includes("/") ? item.original.slice(0, item.original.lastIndexOf("/")) : "top level"}{/if}
              {#if item.deletedAt} · deleted {when(item.deletedAt)}{/if}
            </span>
          </div>
          <button on:click={() => restore(item)}>Restore</button>
          <button class="quiet" title="Delete forever" on:click={() => deleteForever(item)}>Delete</button>
        </li>
      {/each}
    </ul>
    <div class="actions"><button class="primary" on:click={() => dispatch("close")}>Done</button></div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 20;
  }
  .dialog {
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    padding: 16px 18px;
    font-size: 13px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 4px;
    border-bottom: 1px solid var(--border);
  }
  .what {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 11px;
    color: var(--muted);
  }
  .hint {
    color: var(--muted);
  }
  button {
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  button:hover {
    background: var(--hover);
  }
  button.quiet {
    border-color: transparent;
    color: var(--muted);
  }
  button.danger {
    color: var(--danger);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 12px;
  }
  button.primary {
    border: none;
    background: var(--accent);
    color: white;
    padding: 6px 16px;
  }
</style>
