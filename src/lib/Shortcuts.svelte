<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import IconButton from "./IconButton.svelte";
  import { Close } from "./icons";
  import { trapFocus } from "./trapFocus";

  const dispatch = createEventDispatcher<{ close: void }>();
  const mac = /Mac|iPhone|iPad/.test(navigator.platform);
  // Written with the Mac symbols; other platforms get their names.
  const keys = (k: string) =>
    mac ? k : k.replace(/⌘/g, "Ctrl+").replace(/⇧/g, "Shift+").replace(/⌥/g, "Alt+").replace(/⌃/g, "Ctrl+");

  const groups: [string, [string, string][]][] = [
    [
      "Everywhere",
      [
        ["⌘K", "Search notes"],
        ["⌘⇧P", "Commands"],
        ["⌘N", "New note"],
        ["⌘⇧D", "Today's note"],
        ["⌘,", "Settings"],
        ["⌘.", "Focus mode"],
        ["⌘?", "This list"],
      ],
    ],
    [
      "Notes",
      [
        ["⌘E", "Switch between editing and preview"],
        ["Esc", "Stop editing"],
        ["⌘S", "Save now (it also saves by itself)"],
        ["⌘[  ⌘]", "Back and forward"],
      ],
    ],
    [
      "Tabs",
      [
        ["⌘-click", "Open a note in a new tab"],
        ["⌃Tab  ⌃⇧Tab", "Next and previous tab"],
        ["⌘1 … ⌘8", "Go to a tab; ⌘9 for the last"],
      ],
    ],
    [
      "Writing",
      [
        ["[[  or  /", "Link to another note"],
        ["Tab  ⇧Tab", "Indent and outdent"],
        ["⌘F", "Find in this note"],
        ["⌘Z  ⌘⇧Z", "Undo and redo"],
        ["⌘V", "Paste an image into the note"],
      ],
    ],
  ];

  let dialog: HTMLElement;
  onMount(() => dialog.focus());
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div class="backdrop" on:mousedown|self={() => dispatch("close")} on:keydown={(e) => e.key === "Escape" && dispatch("close")}>
  <div class="dialog" role="dialog" aria-modal="true" use:trapFocus aria-label="Keyboard shortcuts" tabindex="-1" bind:this={dialog}>
    <header>
      <h2>Keyboard shortcuts</h2>
      <IconButton icon={Close} label="Close" on:click={() => dispatch("close")} />
    </header>
    <div class="groups">
      {#each groups as [name, rows]}
        <section>
          <h3>{name}</h3>
          <dl>
            {#each rows as [k, what]}
              <dt>
                {#each keys(k).split(/\s{2}/) as part}{#if part === "or"}<span class="or">or</span>{:else}<kbd>{part}</kbd>{/if}{/each}
              </dt>
              <dd>{what}</dd>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
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
    width: min(640px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow-y: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    padding: 14px 18px 18px;
    font-size: 13px;
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 8px 28px;
  }
  h3 {
    margin: 14px 0 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    align-items: center;
    gap: 6px 12px;
    margin: 0;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .or {
    color: var(--muted);
    font-size: 12px;
  }
  dd {
    margin: 0;
    color: var(--muted);
  }
  kbd {
    display: inline-block;
    min-width: 22px;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 5px;
    background: var(--side);
    font: 12px/1.4 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    text-align: center;
    color: var(--text);
  }
</style>
