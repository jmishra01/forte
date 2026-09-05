<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { syncApi, exportApi } from "../api";
  import type { GitStatus } from "../types";

  const dispatch = createEventDispatcher<{ close: void; "data-dir-changed": void }>();

  let dataDir = "";
  let git: GitStatus | null = null;
  let remoteInput = "";
  let busy = false;
  let message: string | null = null;
  let messageIsError = false;

  onMount(refresh);

  async function refresh() {
    dataDir = await syncApi.getDataDir();
    git = await syncApi.gitStatus();
    remoteInput = git.remoteUrl ?? "";
  }

  function report(text: string, isError = false) {
    message = text;
    messageIsError = isError;
  }

  async function changeFolder() {
    const path = await syncApi.pickDataDir();
    if (!path) return;
    const migrate = confirm(
      "Move your existing notes and PDFs into this folder?\n\nOK = copy existing data there\nCancel = use this folder as-is (e.g. it already has data, or you're starting fresh)"
    );
    busy = true;
    try {
      await syncApi.chooseDataDir(path, migrate);
      await refresh();
      report("Data folder updated.");
      dispatch("data-dir-changed");
    } catch (e) {
      report(e instanceof Error ? e.message : "Failed to change folder", true);
    } finally {
      busy = false;
    }
  }

  async function initGit() {
    busy = true;
    try {
      const result = await syncApi.gitInit();
      report(result);
      await refresh();
    } catch (e) {
      report(e instanceof Error ? e.message : "git init failed", true);
    } finally {
      busy = false;
    }
  }

  async function setRemote() {
    if (!remoteInput.trim()) return;
    busy = true;
    try {
      const result = await syncApi.gitSetRemote(remoteInput.trim());
      report(result);
      await refresh();
    } catch (e) {
      report(e instanceof Error ? e.message : "Failed to set remote", true);
    } finally {
      busy = false;
    }
  }

  async function syncNow() {
    busy = true;
    try {
      const result = await syncApi.gitSync();
      report(result);
      await refresh();
    } catch (e) {
      report(e instanceof Error ? e.message : "Sync failed", true);
    } finally {
      busy = false;
    }
  }

  async function exportZip() {
    const path = await exportApi.pickZipSavePath("notes-export.zip");
    if (!path) return;
    busy = true;
    try {
      const count = await exportApi.exportAllNotes(path);
      report(`Exported ${count} note${count === 1 ? "" : "s"} to ${path}`);
    } catch (e) {
      report(e instanceof Error ? e.message : "Export failed", true);
    } finally {
      busy = false;
    }
  }
</script>

<div class="backdrop" role="presentation" on:click|self={() => dispatch("close")}>
  <div class="panel">
    <h3>Settings</h3>

    <section>
      <h4>Data folder</h4>
      <p class="path">{dataDir}</p>
      <button on:click={changeFolder} disabled={busy}>Change folder…</button>
      <p class="hint">
        Point this at a folder synced by Dropbox, Syncthing, etc. to keep your notes and PDFs
        available on other devices.
      </p>
    </section>

    <section>
      <h4>Export</h4>
      <button on:click={exportZip} disabled={busy}>Export all notes as .zip</button>
    </section>

    <section>
      <h4>Git sync</h4>
      {#if git}
        {#if !git.available}
          <p class="hint">git is not installed or not on PATH.</p>
        {:else if !git.isRepo}
          <button on:click={initGit} disabled={busy}>Initialize git repo here</button>
        {:else}
          <p class="status-line">
            {git.dirty ? "Uncommitted changes" : "Clean"}
            {#if git.lastCommitAt}
              · last commit {new Date(git.lastCommitAt).toLocaleString()}
            {/if}
          </p>
          <div class="remote-row">
            <input type="text" placeholder="git remote URL (optional)" bind:value={remoteInput} />
            <button on:click={setRemote} disabled={busy || !remoteInput.trim()}>Set remote</button>
          </div>
          <button class="sync-btn" on:click={syncNow} disabled={busy}>Sync now</button>
        {/if}
      {/if}
    </section>

    {#if message}
      <p class="message" class:error={messageIsError}>{message}</p>
    {/if}

    <div class="footer">
      <button class="close" on:click={() => dispatch("close")}>Close</button>
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
    z-index: 20;
  }
  .panel {
    background: var(--bg-alt);
    border-radius: 10px;
    padding: 20px;
    width: 440px;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }
  h3 {
    margin: 0 0 12px;
  }
  section {
    margin-bottom: 18px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  section:last-of-type {
    border-bottom: none;
  }
  h4 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .path {
    font-size: 12px;
    font-family: ui-monospace, monospace;
    background: var(--bg);
    padding: 6px 8px;
    border-radius: 5px;
    word-break: break-all;
    margin: 0 0 8px;
  }
  .hint {
    font-size: 11px;
    color: var(--text-muted);
    margin: 6px 0 0;
  }
  .status-line {
    font-size: 12px;
    color: var(--text-muted);
    margin: 0 0 8px;
  }
  .remote-row {
    display: flex;
    gap: 6px;
    margin-bottom: 8px;
  }
  .remote-row input {
    flex: 1;
    font-size: 12px;
    padding: 6px 8px;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  button {
    font-size: 12px;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
  }
  .sync-btn {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  button:disabled {
    opacity: 0.5;
  }
  .message {
    font-size: 12px;
    background: var(--bg);
    padding: 8px 10px;
    border-radius: 6px;
    white-space: pre-wrap;
    max-height: 120px;
    overflow-y: auto;
  }
  .message.error {
    color: var(--danger);
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 12px;
  }
  .close {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
</style>
