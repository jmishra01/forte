<script lang="ts">
  import { createEventDispatcher, onMount, tick } from "svelte";
  import type { NoteMeta, PdfMeta } from "../types";

  export let notes: NoteMeta[] = [];
  export let pdfs: PdfMeta[] = [];

  const dispatch = createEventDispatcher<{
    "open-note": string;
    "open-pdf": string;
    close: void;
  }>();

  type Item = { kind: "note" | "pdf"; id: string; title: string };

  let query = "";
  let activeIndex = 0;
  let inputEl: HTMLInputElement;

  $: items = (() => {
    const all: Item[] = [
      ...notes.map((n) => ({ kind: "note" as const, id: n.id, title: n.title || "Untitled" })),
      ...pdfs.map((p) => ({ kind: "pdf" as const, id: p.id, title: p.title })),
    ];
    const q = query.trim().toLowerCase();
    if (!q) return all.slice(0, 30);
    return all.filter((i) => i.title.toLowerCase().includes(q)).slice(0, 30);
  })();

  $: if (activeIndex >= items.length) activeIndex = Math.max(0, items.length - 1);

  onMount(async () => {
    await tick();
    inputEl?.focus();
  });

  function choose(item: Item) {
    if (item.kind === "note") dispatch("open-note", item.id);
    else dispatch("open-pdf", item.id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      dispatch("close");
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = Math.min(activeIndex + 1, items.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = Math.max(activeIndex - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = items[activeIndex];
      if (item) choose(item);
    }
  }
</script>

<div class="backdrop" role="presentation" on:click|self={() => dispatch("close")}>
  <div class="palette">
    <input
      bind:this={inputEl}
      bind:value={query}
      on:keydown={handleKeydown}
      placeholder="Jump to a note or PDF…"
    />
    <ul>
      {#each items as item, i (item.kind + item.id)}
        <li>
          <button class:active={i === activeIndex} on:click={() => choose(item)} on:mouseenter={() => (activeIndex = i)}>
            <span class="kind">{item.kind === "note" ? "📝" : "📄"}</span>
            <span class="label">{item.title}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No matches.</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    z-index: 20;
  }
  .palette {
    background: var(--bg-alt);
    border-radius: 10px;
    width: 480px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  input {
    padding: 14px 16px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: none;
    color: var(--text);
    font-size: 14px;
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: none;
    border: none;
    border-radius: 6px;
    text-align: left;
    font-size: 13px;
    color: var(--text);
  }
  li button.active {
    background: var(--accent);
    color: white;
  }
  .empty {
    padding: 12px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
