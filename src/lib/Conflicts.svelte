<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { api, nameOf, type Conflict } from "./api";

  export let conflicts: Conflict[];

  const dispatch = createEventDispatcher<{ settled: string; open: string; error: string; close: void }>();
  /** The copy being compared, with both texts. */
  let comparing: { conflict: Conflict; mine: string; theirs: string } | null = null;

  async function compare(c: Conflict) {
    try {
      const [mine, theirs] = await Promise.all([c.original ? api.readNote(c.original) : "", api.readNote(c.path)]);
      comparing = { conflict: c, mine, theirs };
    } catch (e) {
      dispatch("error", String(e));
    }
  }

  async function settle(c: Conflict, keep: "original" | "copy" | "both") {
    try {
      const show = await api.settleConflict(c.path, keep);
      comparing = null;
      dispatch("settled", show);
    } catch (e) {
      dispatch("error", String(e));
    }
  }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="backdrop" on:mousedown|self={() => dispatch("close")} on:keydown={(e) => e.key === "Escape" && dispatch("close")}>
  <div class="dialog" class:wide={comparing} role="dialog" aria-label="Sync conflicts">
    {#if comparing}
      {@const c = comparing.conflict}
      <h2>{nameOf(c.original ?? c.path)}</h2>
      <p class="hint">Two versions of this note were saved on different devices. Choose which to keep.</p>
      <div class="compare">
        <section>
          <h3>This note</h3>
          <pre>{c.original ? comparing.mine : "(the original note no longer exists)"}</pre>
        </section>
        <section>
          <h3>Conflicting copy</h3>
          <pre>{comparing.theirs}</pre>
        </section>
      </div>
      <div class="actions">
        <button class="quiet" on:click={() => (comparing = null)}>Back</button>
        <span class="grow" />
        {#if c.original}<button on:click={() => settle(c, "original")}>Keep this note</button>{/if}
        <button on:click={() => settle(c, "copy")}>{c.original ? "Keep the copy" : "Keep it as the note"}</button>
        <button on:click={() => settle(c, "both")}>Keep both</button>
      </div>
      <p class="fine">The version you don't keep goes to the trash, so you can still get it back.</p>
    {:else}
      <h2>Sync conflicts</h2>
      {#if !conflicts.length}
        <p class="hint">No conflicts. Every note has a single version.</p>
      {/if}
      <ul>
        {#each conflicts as c (c.path)}
          <li>
            <div class="what">
              <span class="name">{nameOf(c.original ?? c.path)}</span>
              <span class="meta">
                {c.kind === "copy" ? `Conflicting copy: ${nameOf(c.path)}` : "Contains both versions from a Git merge"}
              </span>
            </div>
            {#if c.kind === "copy"}
              <button on:click={() => compare(c)}>Compare…</button>
            {:else}
              <button on:click={() => dispatch("open", c.path)}>Open</button>
            {/if}
          </li>
        {/each}
      </ul>
      <div class="actions"><span class="grow" /><button class="primary" on:click={() => dispatch("close")}>Done</button></div>
    {/if}
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
  .dialog.wide {
    width: min(960px, calc(100vw - 32px));
  }
  h2 {
    margin: 0 0 6px;
    font-size: 16px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .hint,
  .fine {
    color: var(--muted);
  }
  .fine {
    font-size: 11px;
    margin: 8px 0 0;
  }
  .compare {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  @media (max-width: 640px) {
    .compare {
      grid-template-columns: 1fr;
    }
  }
  .compare section {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  pre {
    flex: 1;
    margin: 0;
    max-height: 50vh;
    overflow: auto;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--side);
    font: 12px/1.5 ui-monospace, "SF Mono", Menlo, monospace;
    white-space: pre-wrap;
    word-break: break-word;
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
    font-weight: 500;
  }
  .meta {
    font-size: 11px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .grow {
    flex: 1;
  }
  button {
    padding: 5px 12px;
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
  button.primary {
    border: none;
    background: var(--accent);
    color: white;
  }
</style>
