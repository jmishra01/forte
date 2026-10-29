<script lang="ts" context="module">
  /** An action offered when the query starts with ">". */
  export type Command = { label: string; hint?: string; run: () => void };
</script>

<script lang="ts">
  import { createEventDispatcher, onMount, tick } from "svelte";
  import { api, dirOf, nameOf, type Hit } from "./api";
  import { highlight, parseQuery } from "./query";
  import { trapFocus } from "./trapFocus";

  /** All note paths, for matching by name. */
  export let notes: string[];
  export let commands: Command[] = [];
  /** Starting text, e.g. ">" to open in command mode. */
  export let initial = "";

  type Item = { path: string; line?: number; text?: string };

  const dispatch = createEventDispatcher<{ choose: Item; run: Command; close: void }>();
  let query = initial;
  let hits: Hit[] = [];
  let index = 0;
  let input: HTMLInputElement;
  let list: HTMLElement;
  let searchTimer: ReturnType<typeof setTimeout>;
  let searchId = 0;

  $: commandMode = query.startsWith(">");
  $: q = (commandMode ? query.slice(1) : query).trim().toLowerCase();
  $: matchingCommands = commandMode ? commands.filter((c) => c.label.toLowerCase().includes(q)) : [];
  $: parsed = parseQuery(q);
  // Notes whose names have every word (prefix matches first), then matching lines.
  // Filters only make sense for content search, so names are left out then.
  $: byName = !q
    ? notes.slice(0, 20)
    : parsed.filtered || !parsed.words.length
      ? []
      : notes
          .filter((p) => parsed.words.every((w) => nameOf(p).toLowerCase().includes(w)))
          .sort((a, b) => rank(a) - rank(b) || nameOf(a).localeCompare(nameOf(b)))
          .slice(0, 20);
  $: items = commandMode ? [] : ([...byName.map((path): Item => ({ path })), ...hits] as Item[]);
  $: count = commandMode ? matchingCommands.length : items.length;
  $: if (index >= count) index = Math.max(0, count - 1);
  $: search(commandMode ? "" : q);

  const rank = (p: string) => (nameOf(p).toLowerCase().startsWith(parsed.words[0] ?? "") ? 0 : 1);

  function search(q: string) {
    clearTimeout(searchTimer);
    const id = ++searchId;
    if (!q) {
      hits = [];
      return;
    }
    searchTimer = setTimeout(async () => {
      const found = await api.search(q).catch(() => []);
      if (id === searchId) hits = found;
    }, 120);
  }

  async function move(by: number) {
    if (!count) return;
    index = (index + by + count) % count;
    await tick();
    list?.querySelector(".sel")?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter" && commandMode && matchingCommands[index]) {
      e.preventDefault();
      dispatch("run", matchingCommands[index]);
    } else if (e.key === "Enter" && items[index]) {
      e.preventDefault();
      dispatch("choose", items[index]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      dispatch("close");
    }
  }

  onMount(() => {
    input.focus();
    input.setSelectionRange(query.length, query.length);
  });
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="backdrop" on:mousedown|self={() => dispatch("close")}>
  <div class="palette" role="dialog" aria-modal="true" use:trapFocus aria-label="Search notes">
    <input
      bind:this={input}
      bind:value={query}
      on:keydown={onKey}
      placeholder="Search notes, or type > for commands"
      aria-label="Search notes, or type > for commands"
      spellcheck="false"
    />
    <ul bind:this={list} role="listbox">
      {#if commandMode}
        {#each matchingCommands as cmd, i (cmd.label)}
          <li
            role="option"
            aria-selected={i === index}
            class:sel={i === index}
            on:mousemove={() => (index = i)}
            on:click={() => dispatch("run", cmd)}
          >
            <span class="name">{#each highlight(cmd.label, [q]) as p}{#if p.match}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
            {#if cmd.hint}<span class="where">{cmd.hint}</span>{/if}
          </li>
        {:else}
          <li class="none">No matching commands</li>
        {/each}
      {:else}
        {#each items as item, i (item.path + ":" + (item.line ?? ""))}
          {#if i === byName.length && hits.length}<li class="group" role="presentation">In note text</li>{/if}
          <li
            role="option"
            aria-selected={i === index}
            class:sel={i === index}
            on:mousemove={() => (index = i)}
            on:click={() => dispatch("choose", item)}
          >
            <span class="name">
              {#each highlight(nameOf(item.path), item.line ? [] : parsed.words) as p}{#if p.match}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
            </span>
            {#if dirOf(item.path)}<span class="where">{dirOf(item.path)}</span>{/if}
            {#if item.text}
              <span class="text">
                {#each highlight(item.text, parsed.words) as p}{#if p.match}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
              </span>
            {/if}
          </li>
        {:else}
          <li class="none">
            No matches. Try fewer words, or <code>tag:name</code>, <code>in:folder</code>, <code>"exact phrase"</code>,
            <code>-word</code>.
          </li>
        {/each}
      {/if}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    z-index: 20;
  }
  .palette {
    width: min(600px, calc(100vw - 32px));
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }
  input {
    padding: 12px 14px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: none;
    color: var(--text);
    font-size: 15px;
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow-y: auto;
  }
  li {
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  li.sel {
    background: var(--active);
  }
  .name {
    font-weight: 500;
  }
  .where {
    margin-left: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .text {
    display: block;
    margin-top: 2px;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  mark {
    background: none;
    color: var(--accent);
    font-weight: 600;
  }
  .group {
    padding: 8px 10px 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    cursor: default;
  }
  .none {
    color: var(--muted);
    cursor: default;
  }
</style>
