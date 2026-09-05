<script lang="ts">
  import { createEventDispatcher } from "svelte";

  export let busy = false;
  export let errorMessage: string | null = null;

  let url = "";
  let title = "";

  const dispatch = createEventDispatcher<{ submit: { url: string; title: string }; cancel: void }>();

  function submit() {
    if (!url.trim()) return;
    dispatch("submit", { url: url.trim(), title: title.trim() });
  }
</script>

<div
  class="backdrop"
  role="presentation"
  on:click|self={() => dispatch("cancel")}
  on:keydown={(e) => e.key === "Escape" && dispatch("cancel")}
>
  <div class="dialog">
    <h3>Add PDF from URL</h3>
    <label>
      PDF URL
      <input
        type="text"
        placeholder="https://example.com/document.pdf"
        bind:value={url}
        on:keydown={(e) => e.key === "Enter" && submit()}
      />
    </label>
    <label>
      Title (optional)
      <input type="text" placeholder="My document" bind:value={title} />
    </label>
    {#if errorMessage}
      <p class="error">{errorMessage}</p>
    {/if}
    <div class="actions">
      <button class="cancel" on:click={() => dispatch("cancel")} disabled={busy}>Cancel</button>
      <button class="confirm" on:click={submit} disabled={busy || !url.trim()}>
        {busy ? "Downloading…" : "Add"}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }
  .dialog {
    background: var(--bg-alt);
    border-radius: 10px;
    padding: 20px;
    width: 380px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }
  h3 {
    margin: 0;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }
  input {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin: 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .actions button {
    padding: 7px 14px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 13px;
  }
  .cancel {
    background: var(--bg);
    color: var(--text);
  }
  .confirm {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .confirm:disabled,
  .cancel:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
