import { insertNodeAtCursor, insertBlockNode, insertTable as insertTableWidget } from "./htmlTable";

// All commands act on `root`, the note's single contenteditable element.
// Where a native `document.execCommand` exists (bold/italic/lists/headings/
// color/...), we use it — the browser fires its own `input` event for these,
// which the host editor already listens for. Everything execCommand can't
// do (inline-code wrap, links, code blocks, wikilinks, tables) mutates the
// DOM directly and dispatches `input` on `root` itself to trigger a save.

function selectionInRoot(root: HTMLElement): Selection | null {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.anchorNode || !root.contains(sel.anchorNode)) return null;
  return sel;
}

function notify(root: HTMLElement) {
  root.dispatchEvent(new Event("input", { bubbles: true }));
}

function exec(root: HTMLElement, command: string, value?: string) {
  root.focus();
  document.execCommand(command, false, value);
}

// `lib.dom`'s execCommand typing declares `value` as a plain string, but
// styleWithCSS's own value is a boolean — cast through unknown to keep the
// real runtime semantics (a string like "false" is still truthy).
function setStyleWithCSS(on: boolean) {
  document.execCommand("styleWithCSS", false, on as unknown as string);
}

function withStyleWithCSS(fn: () => void) {
  setStyleWithCSS(true);
  try {
    fn();
  } finally {
    setStyleWithCSS(false);
  }
}

function currentBlockTag(root: HTMLElement): string | null {
  const sel = selectionInRoot(root);
  if (!sel) return null;
  let node: Node | null = sel.anchorNode;
  while (node && node !== root) {
    if (node instanceof HTMLElement && /^(H1|H2|H3|H4|H5|H6|BLOCKQUOTE|P|DIV)$/.test(node.tagName)) {
      return node.tagName;
    }
    node = node.parentNode;
  }
  return null;
}

export function toggleBold(root: HTMLElement) {
  exec(root, "bold");
}
export function toggleItalic(root: HTMLElement) {
  exec(root, "italic");
}
export function toggleUnderline(root: HTMLElement) {
  exec(root, "underline");
}
export function toggleStrikethrough(root: HTMLElement) {
  exec(root, "strikeThrough");
}

export function toggleInlineCode(root: HTMLElement) {
  const sel = selectionInRoot(root);
  if (!sel || sel.isCollapsed) {
    root.focus();
    return;
  }
  root.focus();
  const range = sel.getRangeAt(0);
  const startEl = range.commonAncestorContainer instanceof HTMLElement
    ? range.commonAncestorContainer
    : range.commonAncestorContainer.parentElement;
  const existing = startEl?.closest("code");
  if (existing && root.contains(existing)) {
    const parent = existing.parentNode;
    if (parent) {
      const frag = document.createDocumentFragment();
      while (existing.firstChild) frag.appendChild(existing.firstChild);
      parent.replaceChild(frag, existing);
    }
  } else {
    const wrapper = document.createElement("code");
    wrapper.appendChild(range.extractContents());
    range.insertNode(wrapper);
    sel.removeAllRanges();
    const after = document.createRange();
    after.selectNodeContents(wrapper);
    sel.addRange(after);
  }
  notify(root);
}

export function toggleHeading(root: HTMLElement, level: number) {
  if (!selectionInRoot(root)) return;
  const tag = `H${level}`;
  const current = currentBlockTag(root);
  exec(root, "formatBlock", current === tag ? "P" : tag);
}

export function toggleQuote(root: HTMLElement) {
  if (!selectionInRoot(root)) return;
  const current = currentBlockTag(root);
  exec(root, "formatBlock", current === "BLOCKQUOTE" ? "P" : "BLOCKQUOTE");
}

export function toggleBullet(root: HTMLElement) {
  exec(root, "insertUnorderedList");
}

export function toggleOrdered(root: HTMLElement) {
  exec(root, "insertOrderedList");
}

function enclosingList(range: Range, root: HTMLElement): HTMLElement | null {
  const node = range.commonAncestorContainer;
  const el = node instanceof HTMLElement ? node : node.parentElement;
  const list = el?.closest("ul,ol") as HTMLElement | null;
  return list && root.contains(list) ? list : null;
}

function applyTaskToggle(ul: HTMLElement, range: Range) {
  const allLis = Array.from(ul.querySelectorAll(":scope > li"));
  const targets = allLis.filter((li) => range.intersectsNode(li));
  const scoped = targets.length ? targets : allLis;
  const allAlreadyTasks = scoped.every((li) => li.querySelector(":scope > input[type='checkbox']"));

  scoped.forEach((li) => {
    const existing = li.querySelector(":scope > input[type='checkbox']");
    if (allAlreadyTasks) {
      existing?.remove();
    } else if (!existing) {
      const cb = document.createElement("input");
      cb.type = "checkbox";
      li.insertBefore(cb, li.firstChild);
    }
  });
  ul.classList.toggle("task-list", !allAlreadyTasks);
}

export function toggleTask(root: HTMLElement) {
  const sel = selectionInRoot(root);
  if (!sel) return;
  root.focus();
  const range = sel.getRangeAt(0);

  // Only ask the browser to build a list when the selection isn't already
  // inside one: `insertUnorderedList` is itself a toggle, so calling it on a
  // selection that's already listed would *remove* the list instead of
  // leaving it in place for us to add checkboxes to.
  const existingList = enclosingList(range, root);
  if (existingList) {
    applyTaskToggle(existingList, range);
    notify(root);
    return;
  }

  exec(root, "insertUnorderedList");
  const sel2 = window.getSelection();
  if (!sel2 || sel2.rangeCount === 0) return;
  const range2 = sel2.getRangeAt(0);
  const newList = enclosingList(range2, root);
  if (!newList) return;
  applyTaskToggle(newList, range2);
  notify(root);
}

// Replaces `range` (or inserts at the current cursor if there's no range —
// e.g. focus already left the editor for the dialog's own inputs) with a
// real `<a>`, built from the link dialog's two fields.
export function applyLink(root: HTMLElement, range: Range | null, url: string, text: string) {
  root.focus();
  const a = document.createElement("a");
  a.href = url;
  a.textContent = text || url;
  if (range) {
    range.deleteContents();
    range.insertNode(a);
    range.setStartAfter(a);
    range.collapse(true);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
  } else {
    insertNodeAtCursor(root, a);
  }
  notify(root);
}

export function insertCodeBlock(root: HTMLElement) {
  const sel = selectionInRoot(root);
  root.focus();
  const text = sel && !sel.isCollapsed ? sel.toString() : "";
  const pre = document.createElement("pre");
  const code = document.createElement("code");
  code.textContent = text;
  pre.appendChild(code);
  if (sel && !sel.isCollapsed) {
    sel.getRangeAt(0).deleteContents();
  }
  insertBlockNode(root, pre);
  notify(root);
}

export function insertTable(root: HTMLElement) {
  root.focus();
  const table = insertTableWidget(root, () => notify(root));
  notify(root);
  requestAnimationFrame(() => {
    (table.querySelector("th") as HTMLElement | null)?.focus();
  });
}

export function setTextColor(root: HTMLElement, color: string) {
  const sel = selectionInRoot(root);
  if (!sel || sel.isCollapsed) {
    root.focus();
    return;
  }
  root.focus();
  withStyleWithCSS(() => document.execCommand("foreColor", false, color));
}

export function setBackgroundColor(root: HTMLElement, color: string) {
  const sel = selectionInRoot(root);
  if (!sel || sel.isCollapsed) {
    root.focus();
    return;
  }
  root.focus();
  withStyleWithCSS(() => document.execCommand("hiliteColor", false, color));
}

// Converts a just-completed "[[Title]]" typed immediately before the caret
// into a clickable, non-editable wikilink chip — the rich-text equivalent
// of the old markdown live-preview widget, minus any markdown parsing.
export function linkifyWikilinkAtCursor(root: HTMLElement) {
  const sel = window.getSelection();
  if (!sel || !sel.isCollapsed || sel.rangeCount === 0) return;
  const node = sel.anchorNode;
  if (!node || node.nodeType !== Node.TEXT_NODE || !root.contains(node)) return;
  const offset = sel.anchorOffset;
  const text = node.textContent ?? "";
  const before = text.slice(0, offset);
  const match = /\[\[([^[\]]+)\]\]$/.exec(before);
  if (!match) return;
  const title = match[1].trim();
  if (!title) return;

  const start = match.index;
  const range = document.createRange();
  range.setStart(node, start);
  range.setEnd(node, offset);
  range.deleteContents();

  const a = document.createElement("a");
  a.className = "wikilink";
  a.href = "#";
  a.contentEditable = "false";
  a.dataset.wikilink = title;
  a.textContent = title;
  range.insertNode(a);

  const space = document.createTextNode(" ");
  a.after(space);
  const after = document.createRange();
  after.setStart(space, 1);
  after.collapse(true);
  sel.removeAllRanges();
  sel.addRange(after);
}
