<script lang="ts">
  import type { ComponentType } from "svelte";

  /** An icon from ./icons. */
  export let icon: ComponentType;
  /** What the button does; read out by screen readers and shown on hover. */
  export let label: string;
  export let disabled = false;
  /** For toggles: whether it is on. */
  export let pressed: boolean | undefined = undefined;
  /** "md" (28px, 16px icon) for toolbars, "sm" (22px, 14px icon) inside rows. */
  export let size: "md" | "sm" = "md";
  /** Spin the icon, e.g. while syncing. */
  export let busy = false;
</script>

<button
  type="button"
  class="icon-btn {size}"
  class:busy
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  {disabled}
  on:click
>
  <svelte:component this={icon} size={size === "md" ? 16 : 14} strokeWidth={1.75} aria-hidden="true" />
  <slot />
</button>

<style>
  .icon-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .md {
    width: 28px;
    height: 28px;
  }
  .sm {
    width: 22px;
    height: 22px;
    border-radius: 4px;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .icon-btn[aria-pressed="true"] {
    color: var(--accent);
  }
  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .busy :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .busy :global(svg) {
      animation: none;
    }
  }
</style>
