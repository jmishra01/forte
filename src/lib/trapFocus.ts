// Keeps Tab and Shift+Tab inside a dialog while it is open, and gives focus
// back to whatever had it when the dialog closes.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function trapFocus(node: HTMLElement) {
  const before = document.activeElement as HTMLElement | null;

  function onKey(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const items = Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (!items.length) return e.preventDefault();
    const [first, last] = [items[0], items[items.length - 1]];
    const at = document.activeElement;
    if (e.shiftKey && (at === first || !node.contains(at))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (at === last || !node.contains(at))) {
      e.preventDefault();
      first.focus();
    }
  }

  node.addEventListener("keydown", onKey);
  return {
    destroy() {
      node.removeEventListener("keydown", onKey);
      if (before?.isConnected) before.focus();
    },
  };
}
