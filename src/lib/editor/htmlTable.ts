// Editable HTML tables, live inside the note's contenteditable root (no
// separate markdown/serialization layer — the table's DOM *is* the content).
// A table carries its own hover-overlay UI (row/column insert & delete, a
// column-align popup) built from plain DOM, marked with `data-ui` so
// `stripEditingChrome` (in InlineMarkdownEditor.svelte) can remove it before
// the container's innerHTML is saved.

const GUTTER = 20;

function colCount(table: HTMLTableElement): number {
  return table.tHead?.rows[0]?.cells.length ?? table.tBodies[0]?.rows[0]?.cells.length ?? 1;
}

function markUi<T extends HTMLElement>(el: T): T {
  el.dataset.ui = "1";
  return el;
}

function makeColHandle(): HTMLButtonElement {
  const handle = document.createElement("button");
  handle.type = "button";
  handle.className = "cm-col-handle";
  handle.dataset.action = "open-col-menu";
  handle.title = "Column options";
  handle.textContent = "⋮";
  return markUi(handle);
}

function makeCell(tag: "th" | "td", html: string, align: string): HTMLTableCellElement {
  const cell = document.createElement(tag);
  cell.style.textAlign = align;
  cell.innerHTML = html || "";
  if (tag === "th") cell.appendChild(makeColHandle());
  return cell;
}

function buildTableElement(headers: string[], body: string[][], aligns: string[]): HTMLTableElement {
  const table = document.createElement("table");
  table.className = "cm-table-edit";

  const thead = document.createElement("thead");
  const headerRow = document.createElement("tr");
  headers.forEach((h, i) => headerRow.appendChild(makeCell("th", h, aligns[i] ?? "left")));
  thead.appendChild(headerRow);
  table.appendChild(thead);

  const tbody = document.createElement("tbody");
  body.forEach((row) => {
    const tr = document.createElement("tr");
    row.forEach((cell, i) => tr.appendChild(makeCell("td", cell, aligns[i] ?? "left")));
    tbody.appendChild(tr);
  });
  table.appendChild(tbody);

  return table;
}

// Raw HTML tables coming out of legacy-content migration (or pasted from
// elsewhere) may lack an explicit <thead>: promote the first row instead of
// assuming a shape our own overlay code depends on everywhere.
function normalizeTableShape(table: HTMLTableElement) {
  table.classList.add("cm-table-edit");
  if (table.tHead) {
    Array.from(table.tHead.rows[0]?.cells ?? []).forEach((c) => {
      if (!c.querySelector("[data-action='open-col-menu']")) c.appendChild(makeColHandle());
    });
    return;
  }
  const body = table.tBodies[0];
  const firstRow = body?.rows[0];
  if (!firstRow) return;
  const thead = document.createElement("thead");
  thead.appendChild(firstRow);
  table.insertBefore(thead, body);
  const headerCells = Array.from(firstRow.cells).map((c) => {
    if (c.tagName === "TH") return c;
    const th = document.createElement("th");
    th.innerHTML = c.innerHTML;
    th.style.textAlign = c.style.textAlign;
    c.replaceWith(th);
    return th;
  });
  headerCells.forEach((th) => th.appendChild(makeColHandle()));
}

function insertColumn(table: HTMLTableElement, index: number) {
  const headerRow = table.tHead!.rows[0];
  headerRow.insertBefore(makeCell("th", "", "left"), headerRow.cells[index] ?? null);
  Array.from(table.tBodies[0].rows).forEach((r) => r.insertBefore(makeCell("td", "", "left"), r.cells[index] ?? null));
}

function deleteColumn(table: HTMLTableElement, index: number) {
  if (colCount(table) <= 1) return;
  table.tHead!.rows[0].deleteCell(index);
  Array.from(table.tBodies[0].rows).forEach((r) => r.deleteCell(index));
}

function alignsOfTable(table: HTMLTableElement): string[] {
  return Array.from(table.tHead!.rows[0].cells).map((c) => (c as HTMLElement).style.textAlign || "left");
}

function makeBodyRow(table: HTMLTableElement): HTMLTableRowElement {
  const tr = document.createElement("tr");
  alignsOfTable(table).forEach((align) => tr.appendChild(makeCell("td", "", align)));
  return tr;
}

function insertRowBefore(table: HTMLTableElement, ref: HTMLTableRowElement) {
  table.tBodies[0].insertBefore(makeBodyRow(table), ref);
}

function insertRowAfter(table: HTMLTableElement, ref: HTMLTableRowElement) {
  table.tBodies[0].insertBefore(makeBodyRow(table), ref.nextElementSibling);
}

function deleteRow(table: HTMLTableElement, tr: HTMLTableRowElement) {
  if (table.tBodies[0].rows.length <= 1) return;
  tr.remove();
}

function setColumnAlign(table: HTMLTableElement, index: number, align: string) {
  table.tHead!.rows[0].cells[index].style.textAlign = align;
  Array.from(table.tBodies[0].rows).forEach((r) => (r.cells[index].style.textAlign = align));
}

type PopupContext = { type: "col"; colIndex: number; align: string } | { type: "table" };

function popupButton(label: string, action: string, active = false): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = label;
  b.dataset.action = action;
  if (active) b.classList.add("active");
  return b;
}

function fillPopup(popup: HTMLElement, ctx: PopupContext) {
  popup.innerHTML = "";
  if (ctx.type === "col") {
    const alignRow = document.createElement("div");
    alignRow.className = "cm-table-popup-row";
    alignRow.append(
      popupButton("Left", "align-left", ctx.align === "left"),
      popupButton("Center", "align-center", ctx.align === "center"),
      popupButton("Right", "align-right", ctx.align === "right")
    );
    popup.appendChild(alignRow);
  } else {
    popup.append(popupButton("✕ Delete table", "delete-table"));
  }
}

function positionPopup(popup: HTMLElement, anchor: HTMLElement, wrapper: HTMLElement, placement: "below" | "above") {
  const wrapperRect = wrapper.getBoundingClientRect();
  const anchorRect = anchor.getBoundingClientRect();
  popup.style.left = `${Math.max(0, anchorRect.left - wrapperRect.left)}px`;
  if (placement === "below") {
    popup.style.top = `${anchorRect.bottom - wrapperRect.top + 4}px`;
  } else {
    popup.style.top = `${anchorRect.top - wrapperRect.top - popup.offsetHeight - 4}px`;
  }
}

// Wires hover overlays (row/col insert & delete, column-align popup) onto an
// already-in-the-DOM `table`, wrapping it in a `.cm-html-table-widget` in
// place. `notifyChange` is called after every structural edit so the host
// editor can re-derive its saved content (these edits don't go through
// execCommand, so no native `input` event fires on their own).
export function wireTable(table: HTMLTableElement, notifyChange: () => void) {
  normalizeTableShape(table);

  const wrapper = document.createElement("div");
  wrapper.className = "cm-html-table-widget";
  table.replaceWith(wrapper);
  const scroll = document.createElement("div");
  scroll.className = "cm-table-scroll";
  scroll.appendChild(table);
  wrapper.appendChild(scroll);

  const overlays = markUi(document.createElement("div"));
  overlays.className = "cm-table-row-overlays";
  wrapper.appendChild(overlays);

  function layoutOverlays() {
    overlays.innerHTML = "";
    const wrapperRect = wrapper.getBoundingClientRect();
    const tableRect = table.getBoundingClientRect();
    const gutterLeft = tableRect.left - wrapperRect.left - GUTTER;
    const gutterTop = tableRect.top - wrapperRect.top - GUTTER;
    const fullWidth = GUTTER + tableRect.width;
    const fullHeight = GUTTER + tableRect.height;

    function makeZone(top: number, left: number, height: number, width: number, extraClass: string): HTMLElement {
      const zone = document.createElement("div");
      zone.className = `cm-row-zone ${extraClass}`;
      zone.style.top = `${top}px`;
      zone.style.left = `${left}px`;
      zone.style.height = `${height}px`;
      zone.style.width = `${width}px`;
      overlays.appendChild(zone);
      return zone;
    }

    const headerRow = table.tHead!.rows[0];
    const headerRect = headerRow.getBoundingClientRect();
    const tableZone = makeZone(headerRect.top - wrapperRect.top, gutterLeft, headerRect.height, GUTTER + 6, "cm-row-zone-table");
    const tableBtn = document.createElement("button");
    tableBtn.type = "button";
    tableBtn.className = "cm-table-handle";
    tableBtn.dataset.action = "open-table-menu";
    tableBtn.title = "Table options";
    tableBtn.textContent = "⠿";
    tableZone.appendChild(tableBtn);

    const dataRows = Array.from(table.tBodies[0].rows);

    dataRows.forEach((row, i) => {
      const rect = row.getBoundingClientRect();
      const top = rect.top - wrapperRect.top;

      const delZone = makeZone(top, gutterLeft, rect.height, GUTTER + 6, "cm-row-zone-delete");
      const delBtn = document.createElement("button");
      delBtn.type = "button";
      delBtn.className = "cm-row-delete-btn";
      delBtn.dataset.action = "delete-row";
      delBtn.dataset.rowIndex = String(i);
      delBtn.title = "Remove row";
      delBtn.textContent = "✕";
      delBtn.addEventListener("mouseenter", () => row.classList.add("cm-row-highlight"));
      delBtn.addEventListener("mouseleave", () => row.classList.remove("cm-row-highlight"));
      delZone.appendChild(delBtn);

      if (i === 0) {
        const insZone = makeZone(top - 7, gutterLeft, 14, fullWidth, "cm-row-zone-insert");
        const insBtn = document.createElement("button");
        insBtn.type = "button";
        insBtn.className = "cm-row-insert-btn";
        insBtn.dataset.action = "insert-row-before-first";
        insBtn.title = "Insert row above";
        insBtn.textContent = "+";
        insZone.appendChild(insBtn);
      }

      const insZone = makeZone(top + rect.height - 7, gutterLeft, 14, fullWidth, "cm-row-zone-insert");
      const insBtn = document.createElement("button");
      insBtn.type = "button";
      insBtn.className = "cm-row-insert-btn";
      insBtn.dataset.action = "insert-row-after";
      insBtn.dataset.rowIndex = String(i);
      insBtn.title = "Insert row below";
      insBtn.textContent = "+";
      insZone.appendChild(insBtn);
    });

    const headerCells = Array.from(headerRow.cells) as HTMLTableCellElement[];

    headerCells.forEach((cell, i) => {
      const rect = cell.getBoundingClientRect();
      const left = rect.left - wrapperRect.left;

      const delZone = makeZone(gutterTop, left, GUTTER + 6, rect.width, "cm-col-zone-delete");
      const delBtn = document.createElement("button");
      delBtn.type = "button";
      delBtn.className = "cm-col-delete-btn";
      delBtn.dataset.action = "delete-col-at";
      delBtn.dataset.colIndex = String(i);
      delBtn.title = "Remove column";
      delBtn.textContent = "✕";
      const columnCells = [cell, ...dataRows.map((r) => r.cells[i]).filter((c): c is HTMLTableCellElement => !!c)];
      delBtn.addEventListener("mouseenter", () => columnCells.forEach((c) => c.classList.add("cm-col-highlight")));
      delBtn.addEventListener("mouseleave", () => columnCells.forEach((c) => c.classList.remove("cm-col-highlight")));
      delZone.appendChild(delBtn);

      if (i === 0) {
        const insZone = makeZone(gutterTop, left - 7, fullHeight, 14, "cm-col-zone-insert");
        const insBtn = document.createElement("button");
        insBtn.type = "button";
        insBtn.className = "cm-col-insert-btn";
        insBtn.dataset.action = "insert-col-before-first";
        insBtn.title = "Insert column left";
        insBtn.textContent = "+";
        insZone.appendChild(insBtn);
      }

      const insZone = makeZone(gutterTop, left + rect.width - 7, fullHeight, 14, "cm-col-zone-insert");
      const insBtn = document.createElement("button");
      insBtn.type = "button";
      insBtn.className = "cm-col-insert-btn";
      insBtn.dataset.action = "insert-col-after";
      insBtn.dataset.colIndex = String(i);
      insBtn.title = "Insert column right";
      insBtn.textContent = "+";
      insZone.appendChild(insBtn);
    });
  }

  const resizeObserver = new ResizeObserver(() => layoutOverlays());
  resizeObserver.observe(table);
  requestAnimationFrame(layoutOverlays);
  scroll.addEventListener("scroll", layoutOverlays);

  const popup = markUi(document.createElement("div"));
  popup.className = "cm-table-popup";
  popup.hidden = true;
  wrapper.appendChild(popup);
  let popupCtx: PopupContext | null = null;

  const onDocMouseDown = (e: MouseEvent) => {
    if (popup.contains(e.target as Node)) return;
    closePopup();
  };
  function closePopup() {
    popup.hidden = true;
    popup.innerHTML = "";
    popupCtx = null;
    document.removeEventListener("mousedown", onDocMouseDown, true);
  }
  function openPopup(ctx: PopupContext, anchor: HTMLElement, placement: "below" | "above") {
    popupCtx = ctx;
    fillPopup(popup, ctx);
    popup.hidden = false;
    positionPopup(popup, anchor, wrapper, placement);
    setTimeout(() => document.addEventListener("mousedown", onDocMouseDown, true), 0);
  }

  function deleteEntireTable() {
    if (!window.confirm("Delete this table? Use Ctrl+Z to undo.")) return;
    resizeObserver.disconnect();
    wrapper.remove();
    notifyChange();
  }

  wrapper.addEventListener("mousedown", (e) => {
    if ((e.target as HTMLElement).closest("[data-action]")) e.preventDefault();
  });
  wrapper.addEventListener("input", () => {
    notifyChange();
    layoutOverlays();
  });

  wrapper.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest("[data-action]") as HTMLElement | null;
    if (!btn) return;
    e.preventDefault();
    const action = btn.dataset.action!;

    if (action === "open-col-menu") {
      const th = btn.closest("th") as HTMLTableCellElement;
      const colIndex = Array.from(th.parentElement!.children).indexOf(th);
      openPopup({ type: "col", colIndex, align: th.style.textAlign || "left" }, th, "below");
      return;
    }
    if (action === "open-table-menu") {
      openPopup({ type: "table" }, btn, "below");
      return;
    }
    if (action === "delete-table") {
      closePopup();
      deleteEntireTable();
      return;
    }
    if (action === "insert-row-before-first") {
      const firstRow = table.tBodies[0].rows[0];
      if (firstRow) insertRowBefore(table, firstRow);
      notifyChange();
      layoutOverlays();
      return;
    }
    if (action === "insert-row-after") {
      const row = table.tBodies[0].rows[Number(btn.dataset.rowIndex)];
      if (row) insertRowAfter(table, row);
      notifyChange();
      layoutOverlays();
      return;
    }
    if (action === "delete-row") {
      const row = table.tBodies[0].rows[Number(btn.dataset.rowIndex)];
      if (row) deleteRow(table, row);
      notifyChange();
      layoutOverlays();
      return;
    }
    if (action === "insert-col-before-first") {
      insertColumn(table, 0);
      notifyChange();
      layoutOverlays();
      return;
    }
    if (action === "insert-col-after") {
      insertColumn(table, Number(btn.dataset.colIndex) + 1);
      notifyChange();
      layoutOverlays();
      return;
    }
    if (action === "delete-col-at") {
      deleteColumn(table, Number(btn.dataset.colIndex));
      notifyChange();
      layoutOverlays();
      return;
    }

    if (!popupCtx || popupCtx.type !== "col") return;
    switch (action) {
      case "align-left":
      case "align-center":
      case "align-right":
        setColumnAlign(table, popupCtx.colIndex, action.slice("align-".length));
        break;
    }
    closePopup();
    notifyChange();
    layoutOverlays();
  });
}

// Inserts a brand-new 3x2 table at the current cursor position, wired up
// the same way as any table found in loaded content.
export function insertTable(root: HTMLElement, notifyChange: () => void): HTMLTableElement {
  const table = buildTableElement(
    ["Header 1", "Header 2", "Header 3"],
    [
      ["Cell 1", "Cell 2", "Cell 3"],
      ["Cell 4", "Cell 5", "Cell 6"],
    ],
    ["left", "left", "left"]
  );
  insertBlockNode(root, table);
  wireTable(table, notifyChange);
  return table;
}

export function insertNodeAtCursor(root: HTMLElement, node: Node) {
  const sel = window.getSelection();
  if (sel && sel.rangeCount > 0 && root.contains(sel.anchorNode)) {
    const range = sel.getRangeAt(0);
    range.deleteContents();
    range.insertNode(node);
    range.setStartAfter(node);
    range.collapse(true);
    sel.removeAllRanges();
    sel.addRange(range);
  } else {
    root.appendChild(node);
  }
}

// For block-level content (tables, code blocks): inserting at the raw
// cursor position can land the new node *inside* whatever inline element
// the cursor happens to be in (e.g. a heading), which isn't valid HTML.
// Instead, insert as a sibling right after the top-level block the cursor
// is currently in.
export function insertBlockNode(root: HTMLElement, node: Node) {
  const sel = window.getSelection();
  const anchor = sel && sel.rangeCount > 0 ? sel.anchorNode : null;
  if (anchor && root.contains(anchor)) {
    let topLevel: Node | null = anchor;
    while (topLevel && topLevel.parentNode !== root) topLevel = topLevel.parentNode;
    if (topLevel instanceof HTMLElement) {
      topLevel.after(node);
      return;
    }
  }
  root.appendChild(node);
}
