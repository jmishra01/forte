<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { api } from "./api";
  import type { Settings } from "./settings";

  export let settings: Settings;
  /** Whether an API key is in the keychain. */
  export let hasKey = false;

  const dispatch = createEventDispatcher<{ change: Settings; keyChanged: boolean; close: void }>();
  let dialog: HTMLElement;
  let keyInput = "";
  let keyError = "";

  async function saveKey() {
    keyError = "";
    try {
      await api.aiSetKey(keyInput);
      keyInput = "";
      hasKey = true;
      dispatch("keyChanged", true);
    } catch (e) {
      keyError = String(e);
    }
  }

  async function removeKey() {
    await api.aiClearKey().catch((e) => (keyError = String(e)));
    hasKey = false;
    dispatch("keyChanged", false);
  }

  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    settings = { ...settings, [key]: value };
    dispatch("change", settings);
  }

  /** Change handler for a <select> whose option values are the setting's allowed values. */
  const pick =
    <K extends keyof Settings>(key: K) =>
    (e: Event) =>
      set(key, (e.currentTarget as HTMLSelectElement).value as Settings[K]);

  /** Folder names are vault-relative; strip slashes so "Daily/" and "/Daily" both work. */
  const folder = (v: string) => v.trim().replace(/^\/+|\/+$/g, "");

  onMount(() => dialog.querySelector<HTMLElement>("select, input")?.focus());
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="backdrop" on:mousedown|self={() => dispatch("close")} on:keydown={(e) => e.key === "Escape" && dispatch("close")}>
  <div class="dialog" role="dialog" aria-label="Settings" bind:this={dialog}>
    <h2>Settings</h2>

    <label>
      <span>Theme</span>
      <select value={settings.theme} on:change={pick("theme")}>
        <option value="system">Match system</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </select>
    </label>

    <label>
      <span>Text size</span>
      <select value={settings.textSize} on:change={pick("textSize")}>
        <option value="small">Small</option>
        <option value="medium">Medium</option>
        <option value="large">Large</option>
      </select>
    </label>

    <label>
      <span>Line width</span>
      <select value={settings.width} on:change={pick("width")}>
        <option value="narrow">Narrow</option>
        <option value="medium">Medium</option>
        <option value="wide">Wide</option>
        <option value="full">Full window</option>
      </select>
    </label>

    <label>
      <span>Editor font</span>
      <select value={settings.editorFont} on:change={pick("editorFont")}>
        <option value="mono">Monospace</option>
        <option value="sans">Same as preview</option>
      </select>
    </label>

    <label class="check">
      <input type="checkbox" checked={settings.showOutline} on:change={(e) => set("showOutline", e.currentTarget.checked)} />
      <span>Show outline beside notes</span>
    </label>

    <label>
      <span>Daily notes folder</span>
      <input value={settings.dailyFolder} on:change={(e) => set("dailyFolder", folder(e.currentTarget.value))} />
    </label>

    <label>
      <span>Templates folder</span>
      <input value={settings.templatesFolder} on:change={(e) => set("templatesFolder", folder(e.currentTarget.value))} />
    </label>
    <p class="hint">
      Notes in the templates folder can be used for new notes. A note named "Daily" there is used for daily notes.
      {"{{title}}"}, {"{{date}}"} and {"{{time}}"} are filled in.
    </p>

    <h3>Sync</h3>
    <label class="check">
      <input type="checkbox" checked={settings.gitAutoSync} on:change={(e) => set("gitAutoSync", e.currentTarget.checked)} />
      <span>Sync with Git every 5 minutes</span>
    </label>
    <p class="hint">Only when the notes folder is a Git repository. Uses your computer's own Git login.</p>

    <h3>AI help</h3>
    <label class="check">
      <input type="checkbox" checked={settings.aiEnabled} on:change={(e) => set("aiEnabled", e.currentTarget.checked)} />
      <span>Turn on AI help</span>
    </label>
    {#if settings.aiEnabled}
      {#if hasKey}
        <div class="key">
          <span>API key saved in your system keychain.</span>
          <button class="link" on:click={removeKey}>Remove</button>
        </div>
      {:else}
        <form class="key" on:submit|preventDefault={saveKey}>
          <input type="password" bind:value={keyInput} placeholder="Anthropic API key (sk-ant-…)" autocomplete="off" />
          <button type="submit" disabled={!keyInput.trim()}>Save</button>
        </form>
      {/if}
      {#if keyError}<p class="error">{keyError}</p>{/if}
      <p class="hint">
        Uses your own key from console.anthropic.com, billed to your account. A note (or, for questions, the
        notes that best match) is sent to Anthropic only when you ask for help.
      </p>
    {/if}

    <div class="actions"><button on:click={() => dispatch("close")}>Done</button></div>
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
    width: min(440px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    padding: 18px 20px 16px;
    font-size: 13px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 16px;
  }
  label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 10px 0;
  }
  label.check {
    justify-content: flex-start;
  }
  select,
  input:not([type="checkbox"]) {
    width: 180px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  h3 {
    margin: 18px 0 2px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .key {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  .key span {
    flex: 1;
  }
  .key input {
    flex: 1;
    width: auto !important;
  }
  .key button {
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .key button.link {
    border: none;
    color: var(--danger);
  }
  .error {
    color: var(--danger);
    margin: 4px 0;
  }
  .hint {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.5;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 16px;
  }
  .actions button {
    padding: 6px 16px;
    border: none;
    border-radius: 6px;
    background: var(--accent);
    color: white;
    font: inherit;
  }
</style>
