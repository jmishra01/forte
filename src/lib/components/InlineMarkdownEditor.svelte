<script lang="ts">
  import { onDestroy, onMount, createEventDispatcher } from "svelte";
  import { EditorState, RangeSet, Facet, StateField, type Extension, type Range } from "@codemirror/state";
  import {
    EditorView,
    keymap,
    Decoration,
    type DecorationSet,
    ViewPlugin,
    type ViewUpdate,
    WidgetType,
    placeholder as placeholderExtension,
  } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { markdown } from "@codemirror/lang-markdown";
  import { GFM } from "@lezer/markdown";
  import { syntaxTree } from "@codemirror/language";

  export let value = "";
  export let placeholder = "";

  const dispatch = createEventDispatcher<{ change: string; wikilinkClick: string }>();

  let container: HTMLDivElement;
  let view: EditorView;

  const onWikilinkClick = Facet.define<(title: string) => void, (title: string) => void>({
    combine: (values) => values[values.length - 1] ?? (() => {}),
  });

  class WikilinkWidget extends WidgetType {
    constructor(readonly title: string) {
      super();
    }
    eq(other: WikilinkWidget) {
      return other.title === this.title;
    }
    toDOM(v: EditorView) {
      const span = document.createElement("span");
      span.className = "cm-wikilink-widget";
      span.textContent = this.title;
      span.addEventListener("mousedown", (e) => e.preventDefault());
      span.addEventListener("click", () => {
        v.state.facet(onWikilinkClick)(this.title);
      });
      return span;
    }
    ignoreEvent() {
      return false;
    }
  }

  class CheckboxWidget extends WidgetType {
    constructor(
      readonly checked: boolean,
      readonly markFrom: number,
      readonly markTo: number
    ) {
      super();
    }
    eq(other: CheckboxWidget) {
      return other.checked === this.checked && other.markFrom === this.markFrom && other.markTo === this.markTo;
    }
    toDOM(v: EditorView) {
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = this.checked;
      box.className = "cm-task-checkbox";
      box.addEventListener("mousedown", (e) => e.preventDefault());
      box.addEventListener("click", (e) => {
        e.preventDefault();
        const replacement = this.checked ? "[ ]" : "[x]";
        v.dispatch({ changes: { from: this.markFrom, to: this.markTo, insert: replacement } });
      });
      return box;
    }
    ignoreEvent() {
      return false;
    }
  }

  class BulletWidget extends WidgetType {
    eq() {
      return true;
    }
    toDOM() {
      const span = document.createElement("span");
      span.className = "cm-bullet";
      span.textContent = "•";
      return span;
    }
    ignoreEvent() {
      return true;
    }
  }

  const WIKILINK_RE = /\[\[([^[\]]+)\]\]/g;

  const DELIMITER_ROW_RE = /^\s*\|?(\s*:?-+:?\s*\|)*\s*:?-+:?\s*\|?\s*$/;

  function parseTableRow(row: string): string[] {
    let r = row.trim();
    if (r.startsWith("|")) r = r.slice(1);
    if (r.endsWith("|")) r = r.slice(0, -1);
    return r.split("|").map((c) => c.trim());
  }

  class TableWidget extends WidgetType {
    constructor(
      readonly text: string,
      readonly from: number
    ) {
      super();
    }
    eq(other: TableWidget) {
      return other.text === this.text;
    }
    toDOM(v: EditorView) {
      const wrapper = document.createElement("div");
      wrapper.className = "cm-table-widget";

      const rows = this.text.split("\n").filter((r) => r.trim().length > 0);
      if (rows.length === 0) return wrapper;

      const headerCells = parseTableRow(rows[0]);
      let alignments = headerCells.map(() => "left");
      let bodyStart = 1;
      if (rows.length > 1 && DELIMITER_ROW_RE.test(rows[1])) {
        alignments = parseTableRow(rows[1]).map((c) => {
          const left = c.startsWith(":");
          const right = c.endsWith(":");
          if (left && right) return "center";
          if (right) return "right";
          return "left";
        });
        bodyStart = 2;
      }

      const table = document.createElement("table");
      table.className = "cm-table";

      const thead = document.createElement("thead");
      const headTr = document.createElement("tr");
      headerCells.forEach((cell, i) => {
        const th = document.createElement("th");
        th.textContent = cell;
        th.style.textAlign = alignments[i] ?? "left";
        headTr.appendChild(th);
      });
      thead.appendChild(headTr);
      table.appendChild(thead);

      const tbody = document.createElement("tbody");
      for (let i = bodyStart; i < rows.length; i++) {
        if (DELIMITER_ROW_RE.test(rows[i])) continue;
        const tr = document.createElement("tr");
        parseTableRow(rows[i]).forEach((cell, ci) => {
          const td = document.createElement("td");
          td.textContent = cell;
          td.style.textAlign = alignments[ci] ?? "left";
          tr.appendChild(td);
        });
        tbody.appendChild(tr);
      }
      table.appendChild(tbody);
      wrapper.appendChild(table);

      wrapper.addEventListener("mousedown", (e) => e.preventDefault());
      wrapper.addEventListener("click", () => {
        v.dispatch({ selection: { anchor: this.from } });
        v.focus();
      });
      return wrapper;
    }
    ignoreEvent() {
      return false;
    }
  }

  function buildDecorations(v: EditorView): DecorationSet {
    const decos: Range<Decoration>[] = [];
    const cursorLine = v.state.doc.lineAt(v.state.selection.main.head).number;
    const wikilinkRanges: Array<[number, number]> = [];

    for (const { from, to } of v.visibleRanges) {
      const text = v.state.doc.sliceString(from, to);
      WIKILINK_RE.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = WIKILINK_RE.exec(text))) {
        const start = from + match.index;
        const end = start + match[0].length;
        const lineNo = v.state.doc.lineAt(start).number;
        if (lineNo === cursorLine) continue;
        wikilinkRanges.push([start, end]);
        decos.push(Decoration.replace({ widget: new WikilinkWidget(match[1].trim()) }).range(start, end));
      }
    }

    const insideWikilink = (from: number, to: number) =>
      wikilinkRanges.some(([ws, we]) => from >= ws && to <= we);

    for (const { from, to } of v.visibleRanges) {
      syntaxTree(v.state).iterate({
        from,
        to,
        enter: (node) => {
          if (node.to <= node.from) return;
          if (insideWikilink(node.from, node.to)) return;

          // Tables are handled by a separate StateField (block decorations aren't
          // allowed from a ViewPlugin), so just skip their internals here.
          if (node.name === "Table") return false;

          // Checkboxes stay interactive regardless of cursor position — unlike the
          // other hide-on-active-line constructs, replacing text with a widget here
          // IS the intended way to edit a task, not something to reveal while typing.
          if (node.name === "TaskMarker") {
            const text = v.state.doc.sliceString(node.from, node.to);
            const checked = /\[x\]/i.test(text);
            decos.push(
              Decoration.replace({ widget: new CheckboxWidget(checked, node.from, node.to) }).range(
                node.from,
                node.to
              )
            );
            return;
          }

          // Bullet markers (-, *, +) render as a dot, always — same reasoning as
          // checkboxes above. Ordered-list numbers ("1.") are left alone since
          // they carry real information. Task items already show a checkbox as
          // their marker, so skip the dot there to avoid showing both.
          if (node.name === "ListMark") {
            const text = v.state.doc.sliceString(node.from, node.to);
            if (text === "-" || text === "*" || text === "+") {
              const after = v.state.doc.sliceString(node.to, node.to + 4);
              const isTaskItem = /^\s*\[[ xX]\]/.test(after);
              if (isTaskItem) {
                let end = node.to;
                if (v.state.doc.sliceString(end, end + 1) === " ") end += 1;
                decos.push(Decoration.replace({}).range(node.from, end));
              } else {
                decos.push(Decoration.replace({ widget: new BulletWidget() }).range(node.from, node.to));
              }
            }
            return;
          }

          const lineNo = v.state.doc.lineAt(node.from).number;
          if (lineNo === cursorLine) return;

          switch (node.name) {
            case "HeaderMark": {
              let end = node.to;
              if (v.state.doc.sliceString(end, end + 1) === " ") end += 1;
              decos.push(Decoration.replace({}).range(node.from, end));
              break;
            }
            case "EmphasisMark":
            case "CodeMark":
            case "StrikethroughMark":
              decos.push(Decoration.replace({}).range(node.from, node.to));
              break;
            case "ATXHeading1":
            case "ATXHeading2":
            case "ATXHeading3":
            case "ATXHeading4":
            case "ATXHeading5":
            case "ATXHeading6": {
              const level = node.name.slice(-1);
              decos.push(Decoration.mark({ class: `cm-heading cm-heading-${level}` }).range(node.from, node.to));
              break;
            }
            case "StrongEmphasis":
              decos.push(Decoration.mark({ class: "cm-strong" }).range(node.from, node.to));
              break;
            case "Emphasis":
              decos.push(Decoration.mark({ class: "cm-em" }).range(node.from, node.to));
              break;
            case "InlineCode":
              decos.push(Decoration.mark({ class: "cm-inline-code" }).range(node.from, node.to));
              break;
            case "Strikethrough":
              decos.push(Decoration.mark({ class: "cm-strikethrough" }).range(node.from, node.to));
              break;
          }
        },
      });
    }

    return RangeSet.of(decos, true);
  }

  // Block decorations (the rendered <table>) can only come from a StateField,
  // not a ViewPlugin, so tables get their own decoration source.
  function buildTableDecorations(state: EditorState): DecorationSet {
    const decos: Range<Decoration>[] = [];
    const cursorLine = state.doc.lineAt(state.selection.main.head).number;
    syntaxTree(state).iterate({
      enter: (node) => {
        if (node.name !== "Table") return;
        const startLine = state.doc.lineAt(node.from).number;
        const endLine = state.doc.lineAt(node.to).number;
        const isActive = cursorLine >= startLine && cursorLine <= endLine;
        if (!isActive) {
          const text = state.doc.sliceString(node.from, node.to);
          decos.push(
            Decoration.replace({ widget: new TableWidget(text, node.from), block: true }).range(
              node.from,
              node.to
            )
          );
        }
        return false;
      },
    });
    return RangeSet.of(decos, true);
  }

  const tableField = StateField.define<DecorationSet>({
    create(state) {
      return buildTableDecorations(state);
    },
    update(value, tr) {
      if (tr.docChanged || tr.selection) {
        return buildTableDecorations(tr.state);
      }
      return value;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  const livePreviewField = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(v: EditorView) {
        this.decorations = buildDecorations(v);
      }
      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) {
          this.decorations = buildDecorations(update.view);
        }
      }
    },
    { decorations: (p) => p.decorations }
  );

  onMount(() => {
    const extensions: Extension[] = [
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      markdown({ extensions: GFM }),
      EditorView.lineWrapping,
      livePreviewField,
      tableField,
      placeholderExtension(placeholder),
      onWikilinkClick.of((title) => dispatch("wikilinkClick", title)),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          dispatch("change", update.state.doc.toString());
        }
      }),
      EditorView.theme({
        "&": { height: "100%", fontSize: "14px" },
        ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.6", overflow: "auto" },
        ".cm-content": { padding: "16px 24px", caretColor: "var(--text)" },
        "&.cm-focused": { outline: "none" },
        ".cm-line": { padding: "0" },
      }),
    ];

    view = new EditorView({
      state: EditorState.create({ doc: value, extensions }),
      parent: container,
    });
  });

  onDestroy(() => {
    view?.destroy();
  });

  export function insertAtCursor(text: string) {
    if (!view) return;
    const { from, to } = view.state.selection.main;
    view.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + text.length },
    });
    view.focus();
  }

  export function focusEditor() {
    view?.focus();
  }
</script>

<div class="cm-host" bind:this={container}></div>

<style>
  .cm-host {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

  .cm-host :global(.cm-editor) {
    height: 100%;
    background: var(--bg);
    color: var(--text);
  }

  .cm-host :global(.cm-heading) {
    font-weight: 700;
    line-height: 1.3;
  }
  .cm-host :global(.cm-heading-1) {
    font-size: 1.6em;
  }
  .cm-host :global(.cm-heading-2) {
    font-size: 1.4em;
  }
  .cm-host :global(.cm-heading-3) {
    font-size: 1.2em;
  }
  .cm-host :global(.cm-heading-4),
  .cm-host :global(.cm-heading-5),
  .cm-host :global(.cm-heading-6) {
    font-size: 1.05em;
  }
  .cm-host :global(.cm-strong) {
    font-weight: 700;
  }
  .cm-host :global(.cm-em) {
    font-style: italic;
  }
  .cm-host :global(.cm-inline-code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    background: var(--bg-alt);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.92em;
  }
  .cm-host :global(.cm-strikethrough) {
    text-decoration: line-through;
    opacity: 0.7;
  }
  .cm-host :global(.cm-task-checkbox) {
    cursor: pointer;
    margin: 0 4px 0 0;
    vertical-align: middle;
  }
  .cm-host :global(.cm-bullet) {
    color: var(--accent);
    font-weight: 700;
    display: inline-block;
    width: 1em;
  }
  .cm-host :global(.cm-wikilink-widget) {
    color: var(--accent);
    text-decoration: underline dotted;
    cursor: pointer;
    border-radius: 3px;
    padding: 0 2px;
  }
  .cm-host :global(.cm-wikilink-widget:hover) {
    background: var(--bg-alt);
  }
  .cm-host :global(.cm-table-widget) {
    cursor: text;
    margin: 4px 0;
  }
  .cm-host :global(.cm-table) {
    border-collapse: collapse;
    font-size: 0.95em;
  }
  .cm-host :global(.cm-table th),
  .cm-host :global(.cm-table td) {
    border: 1px solid var(--border);
    padding: 6px 12px;
  }
  .cm-host :global(.cm-table th) {
    background: var(--bg-alt);
    font-weight: 600;
  }
</style>
