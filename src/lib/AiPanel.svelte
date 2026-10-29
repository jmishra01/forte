<script lang="ts">
  import { createEventDispatcher, onDestroy } from "svelte";
  import { api, nameOf, type AiTask } from "./api";
  import { renderMarkdown, resolveLink } from "./markdown";
  import IconButton from "./IconButton.svelte";
  import { Close } from "./icons";

  /** The open note, if any (summaries and link ideas need one). */
  export let current: string | null;
  export let notes: string[];
  export let hasKey: boolean;

  const dispatch = createEventDispatcher<{ open: string; insert: string; settings: void; close: void }>();
  let task: AiTask | null = null;
  let question = "";
  let output = "";
  let sources: string[] = [];
  let running = false;
  let error = "";

  $: from = current ?? notes[0] ?? "";
  $: html = output ? renderMarkdown(output, (t) => resolveLink(t, from, notes)) : "";

  async function start(t: AiTask) {
    if (running) api.aiStop();
    task = t;
    output = "";
    sources = [];
    error = "";
    running = true;
    try {
      await api.aiRun(
        t,
        (e) => {
          if (e.type === "sources") sources = e.paths;
          else if (e.type === "text") output += e.text;
          else if (e.type === "error") error = e.message;
          if (e.type === "done" || e.type === "error") running = false;
        },
        current ?? undefined,
        t === "ask" ? question : undefined,
      );
    } catch (e) {
      error = String(e);
      running = false;
    }
  }

  function stop() {
    api.aiStop();
    running = false;
  }

  function onClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const target = a.getAttribute("data-wikilink");
    const found = target && resolveLink(target, from, notes);
    if (found) dispatch("open", found);
  }

  onDestroy(() => running && api.aiStop());
</script>

<aside class="ai" aria-label="AI help">
  <header>
    <h2>AI help</h2>
    <IconButton size="sm" icon={Close} label="Close AI help" on:click={() => dispatch("close")} />
  </header>

  {#if !hasKey}
    <p class="hint">Add your Anthropic API key in Settings to summarize notes, find related notes and ask questions about your notes.</p>
    <button class="primary" on:click={() => dispatch("settings")}>Open Settings</button>
  {:else}
    <div class="actions">
      <button disabled={!current} on:click={() => start("summarize")}>Summarize this note</button>
      <button disabled={!current} on:click={() => start("links")}>Suggest related notes</button>
    </div>
    <form on:submit|preventDefault={() => question.trim() && start("ask")}>
      <input bind:value={question} placeholder="Ask your notes a question…" />
      <button type="submit" disabled={!question.trim()}>Ask</button>
    </form>

    {#if sources.length}
      <p class="sent">
        Sent to Anthropic: {sources.map(nameOf).join(", ")}
      </p>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}

    {#if output || running}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
      <article class="preview answer" on:click={onClick}>
        {@html html}
        {#if running && !output}<p class="hint">Thinking…</p>{/if}
      </article>
      <div class="actions">
        {#if running}
          <button on:click={stop}>Stop</button>
        {:else if output && current && task !== "ask"}
          <button on:click={() => dispatch("insert", output)}>Add to note</button>
        {/if}
      </div>
    {/if}
    <p class="fine">Uses your API key with Claude Opus 5.5. Only the notes listed above leave this computer.</p>
  {/if}
</aside>

<style>
  .ai {
    width: 320px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    padding: 10px 14px 16px;
    border-left: 1px solid var(--border);
    font-size: 13px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  form {
    display: flex;
    gap: 6px;
  }
  input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  button {
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  button:hover:not(:disabled) {
    background: var(--hover);
  }
  button:disabled {
    opacity: 0.45;
  }
  button.primary {
    border: none;
    background: var(--accent);
    color: white;
    align-self: flex-start;
  }
  .answer {
    font-size: 13px;
    line-height: 1.55;
    padding: 4px 0;
  }
  .hint,
  .sent,
  .fine {
    color: var(--muted);
    margin: 0;
  }
  .sent {
    font-size: 12px;
  }
  .fine {
    font-size: 11px;
    margin-top: auto;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
</style>
