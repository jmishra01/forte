<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { api } from "./api";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import type { Settings } from "./settings";
  import { PERMISSION_TEXT, type PluginInfo } from "./plugins";
  import { trapFocus } from "./trapFocus";

  export let settings: Settings;
  /** Whether an API key is in the keychain. */
  export let hasKey = false;
  /** Plugins found in the vault's .forte/plugins folder. */
  export let plugins: PluginInfo[] = [];
  /** The encrypted sync folder for this vault, if set up. */
  export let encRemote: string | null = null;

  let syncFolder = "";
  let passphrase = "";
  let passphraseAgain = "";
  let syncError = "";
  let connecting = false;
  /** What the chosen folder is: new (type the passphrase twice), or already a Forte sync folder. */
  let folderKind: "new" | "existing" | "other" | null = null;

  async function chooseSyncFolder() {
    const dir = await openDialog({ directory: true });
    if (typeof dir !== "string") return;
    syncFolder = dir;
    syncError = "";
    folderKind = await api.encSyncProbe(dir).catch(() => null);
    if (folderKind === "other") syncError = "That folder has other files in it. Choose an empty folder, or one Forte already syncs to.";
  }

  async function connectSync() {
    syncError = "";
    if (folderKind === "new" && passphrase !== passphraseAgain) return (syncError = "The passphrases don't match.");
    connecting = true;
    try {
      await api.encSyncConnect(syncFolder, passphrase);
      passphrase = passphraseAgain = "";
      dispatch("encSyncChanged", syncFolder);
    } catch (e) {
      syncError = String(e);
    }
    connecting = false;
  }

  async function disconnectSync() {
    await api.encSyncDisconnect().catch((e) => (syncError = String(e)));
    dispatch("encSyncChanged", null);
  }

  const dispatch = createEventDispatcher<{
    change: Settings;
    keyChanged: boolean;
    togglePlugin: { plugin: PluginInfo; on: boolean };
    reloadPlugins: void;
    encSyncChanged: string | null;
    close: void;
  }>();
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
  <div class="dialog" role="dialog" aria-modal="true" use:trapFocus aria-label="Settings" bind:this={dialog}>
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
      <span>Sync automatically every 5 minutes</span>
    </label>
    <p class="hint">Git sync runs when the notes folder is a Git repository, using your computer's own Git login.</p>

    <h3>Encrypted sync</h3>
    {#if encRemote}
      <p class="hint">Syncing through <code>{encRemote}</code>. Everything there is encrypted with your passphrase.</p>
      <div class="key"><span /><button class="link" on:click={disconnectSync}>Stop encrypted sync</button></div>
    {:else}
      <p class="hint">
        Keep your notes in step between computers through a folder that is already synced, like iCloud Drive or Dropbox.
        The folder only ever holds encrypted files, so that service can't read your notes.
      </p>
      <div class="key">
        <span class="path">{syncFolder || "No folder chosen"}</span>
        <button on:click={chooseSyncFolder}>Choose folder…</button>
      </div>
      {#if syncFolder && (folderKind === "new" || folderKind === "existing")}
        <form on:submit|preventDefault={connectSync}>
          {#if folderKind === "existing"}
            <p class="hint">This folder already has encrypted notes. Enter the passphrase you chose for it.</p>
          {/if}
          <input
            type="password"
            bind:value={passphrase}
            placeholder={folderKind === "new" ? "Choose a passphrase (at least 8 characters)" : "Passphrase"}
            autocomplete={folderKind === "new" ? "new-password" : "current-password"}
          />
          {#if folderKind === "new"}
            <input type="password" bind:value={passphraseAgain} placeholder="Passphrase again" autocomplete="new-password" />
          {/if}
          <button type="submit" disabled={connecting || passphrase.length < 8}>{connecting ? "Checking…" : "Start encrypted sync"}</button>
        </form>
        <p class="hint">
          Use the same passphrase on every computer. If you forget it, the encrypted copy can't be opened by anyone,
          including you (your notes on this computer are unaffected).
        </p>
      {/if}
    {/if}
    {#if syncError}<p class="error">{syncError}</p>{/if}

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

    <h3>Plugins</h3>
    {#each plugins as p (p.id)}
      {@const approved = settings.plugins[p.id]}
      <div class="plugin">
        <label class="check">
          <input
            type="checkbox"
            checked={approved === p.hash && !p.error}
            disabled={!!p.error}
            on:change={(e) => dispatch("togglePlugin", { plugin: p, on: e.currentTarget.checked })}
          />
          <span><strong>{p.name}</strong>{#if p.version} <span class="muted">{p.version}</span>{/if}</span>
        </label>
        {#if p.description}<p class="hint">{p.description}</p>{/if}
        {#if p.error}
          <p class="error">Can't load: {p.error}</p>
        {:else}
          <p class="hint">
            {p.permissions.length ? "Can: " + p.permissions.map((x) => PERMISSION_TEXT[x]).join("; ") : "Can only add commands"}
          </p>
          {#if approved && approved !== p.hash}<p class="error">Changed since you turned it on. Turn it on again to approve this version.</p>{/if}
        {/if}
      </div>
    {:else}
      <p class="hint">No plugins. Put them in <code>.forte/plugins</code> in your notes folder.</p>
    {/each}
    <p class="hint">
      Plugins run separately from your notes and can't use the internet. Only turn on plugins you trust.
      <button class="link-btn" on:click={() => dispatch("reloadPlugins")}>Look for plugins again</button>
    </p>

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
  .plugin {
    margin: 8px 0 10px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 6px 0;
  }
  form input {
    width: 100% !important;
  }
  form button {
    align-self: flex-end;
    padding: 5px 12px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
  }
  .plugin label {
    margin: 0;
  }
  .muted {
    color: var(--muted);
    font-weight: normal;
  }
  .link-btn {
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
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
