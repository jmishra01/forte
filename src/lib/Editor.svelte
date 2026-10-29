<script lang="ts" context="module">
  /** A suggestion for the note picker. `link` is what goes between `[[` and `]]`. */
  export type LinkOption = { label: string; detail: string; link: string };
</script>

<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from "svelte";
  import { Annotation, EditorSelection, EditorState } from "@codemirror/state";
  import { EditorView, keymap, placeholder as placeholderExt, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { HighlightStyle, indentUnit, syntaxHighlighting } from "@codemirror/language";
  import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
  import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
  import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
  import { tags } from "@lezer/highlight";

  export let value: string;
  export let placeholder = "";
  /** Notes to offer after `trigger` ("[[" or "/") and the text typed since. */
  export let linkOptions: (trigger: string, query: string) => LinkOption[] = () => [];

  const dispatch = createEventDispatcher<{ change: string; blur: void; images: { files: File[] } }>();
  let host: HTMLDivElement;
  let view: EditorView | undefined;

  /** Marks changes that came from `value`, so they aren't echoed back as edits. */
  const external = Annotation.define<boolean>();

  // Keep the editor in step when `value` changes from outside (reload from disk, link rewrite).
  $: if (view && value !== view.state.doc.toString()) {
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
      selection: { anchor: Math.min(view.state.selection.main.head, value.length) },
      annotations: external.of(true),
    });
  }

  /**
   * The note picker: "[[" anywhere, or "/" at the start of a word, followed by
   * up to 60 characters on the same line.
   */
  function linkCompletions(ctx: CompletionContext) {
    const wiki = ctx.matchBefore(/\[\[[^[\]\n]{0,60}$/);
    const slash = wiki ? null : ctx.matchBefore(/(?:^|\s)\/(?!\s)[^[\]\n]{0,60}$/);
    const m = wiki ?? slash;
    if (!m) return null;
    const from = slash ? m.from + (m.text.length - m.text.trimStart().length) : m.from;
    const trigger = wiki ? "[[" : "/";
    const query = ctx.state.sliceDoc(from + trigger.length, ctx.pos);
    if (/^\s/.test(query)) return null;
    const options = linkOptions(trigger, query);
    if (!options.length) return null;
    return {
      from,
      filter: false, // linkOptions already filtered and ordered them
      options: options.map((o) => ({
        label: o.label,
        detail: o.detail,
        apply: (view: EditorView, _c: unknown, start: number, end: number) => {
          // Swallow a "]]" the user already typed after the cursor.
          const to = view.state.sliceDoc(end, end + 2) === "]]" ? end + 2 : end;
          const text = `[[${o.link}]]`;
          view.dispatch({
            changes: { from: start, to, insert: text },
            selection: { anchor: start + text.length },
            userEvent: "input.complete",
          });
        },
      })),
    };
  }

  const highlight = HighlightStyle.define([
    { tag: tags.heading1, fontSize: "1.3em", fontWeight: "700" },
    { tag: tags.heading2, fontSize: "1.15em", fontWeight: "700" },
    { tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontWeight: "700" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.strikethrough, textDecoration: "line-through" },
    { tag: [tags.link, tags.url], color: "var(--accent)" },
    { tag: tags.monospace, color: "var(--code)" },
    { tag: [tags.quote, tags.processingInstruction, tags.contentSeparator, tags.labelName], color: "var(--muted)" },
    { tag: tags.list, color: "var(--muted)" },
    { tag: tags.meta, color: "var(--muted)" },
  ]);

  const theme = EditorView.theme({
    "&": { flex: "1", minHeight: "0", backgroundColor: "var(--bg)", color: "var(--text)", fontSize: "14px" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": {
      fontFamily: "var(--editor-font)",
      lineHeight: "1.6",
      overflow: "auto",
    },
    ".cm-content": { padding: "16px 28px 40px", caretColor: "var(--text)", maxWidth: "calc(var(--measure) + 56px)" },
    ".cm-line": { padding: "0" },
    ".cm-cursor": { borderLeftColor: "var(--text)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "var(--active) !important",
    },
    ".cm-selectionMatch": { backgroundColor: "var(--hover)" },
    ".cm-placeholder": { color: "var(--muted)" },
    ".cm-tooltip": {
      backgroundColor: "var(--bg)",
      border: "1px solid var(--border)",
      borderRadius: "6px",
      boxShadow: "0 6px 20px rgba(0, 0, 0, 0.18)",
      overflow: "hidden",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul": {
      fontFamily: "inherit",
      fontSize: "13px",
      maxHeight: "16em",
      minWidth: "260px",
      padding: "4px",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li": { padding: "5px 8px", borderRadius: "4px" },
    ".cm-tooltip-autocomplete ul li[aria-selected]": { background: "var(--active)", color: "var(--text)" },
    ".cm-completionDetail": { marginLeft: "6px", fontSize: "11px", fontStyle: "normal", color: "var(--muted)" },
    ".cm-completionIcon": { display: "none" },
    ".cm-panels": { backgroundColor: "var(--side)", color: "var(--text)", borderColor: "var(--border)" },
    ".cm-panel.cm-search": { padding: "6px 10px", fontSize: "13px", fontFamily: "inherit" },
    ".cm-panel.cm-search input, .cm-panel.cm-search button": { fontSize: "12px" },
    ".cm-searchMatch": { backgroundColor: "rgba(255, 200, 0, 0.3)" },
    ".cm-searchMatch-selected": { backgroundColor: "rgba(255, 160, 0, 0.55)" },
  });

  function imageFiles(data: DataTransfer | null) {
    return Array.from(data?.files ?? []).filter((f) => f.type.startsWith("image/"));
  }

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          drawSelection(),
          EditorView.lineWrapping,
          indentUnit.of("  "),
          markdown({ base: markdownLanguage }), // GFM; Enter continues lists and quotes
          syntaxHighlighting(highlight),
          autocompletion({ override: [linkCompletions], icons: false }),
          search({ top: true }),
          highlightSelectionMatches(),
          placeholderExt(placeholder),
          keymap.of([
            ...searchKeymap, // its Escape closes the find bar first
            { key: "Escape", run: (v) => (v.contentDOM.blur(), true) },
            ...historyKeymap,
            indentWithTab,
            // ⌘[ / ⌘] are the app's back/forward; Tab and Shift-Tab indent.
            ...defaultKeymap.filter((b) => b.key !== "Mod-[" && b.key !== "Mod-]"),
          ]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged && !u.transactions.some((t) => t.annotation(external))) {
              value = u.state.doc.toString();
              dispatch("change", value);
            }
          }),
          EditorView.domEventHandlers({
            paste(e) {
              const files = imageFiles(e.clipboardData);
              if (!files.length) return false;
              e.preventDefault();
              dispatch("images", { files });
              return true;
            },
            drop(e, v) {
              const files = imageFiles(e.dataTransfer);
              if (!files.length) return false;
              e.preventDefault();
              const at = v.posAtCoords({ x: e.clientX, y: e.clientY });
              if (at !== null) v.dispatch({ selection: { anchor: at } });
              dispatch("images", { files });
              return true;
            },
          }),
          theme,
        ],
      }),
    });
  });

  onDestroy(() => view?.destroy());

  export function focus() {
    view?.focus();
  }

  /** Puts the caret at the start of `line` (1-based) and scrolls it near the top. */
  export function goToLine(line: number) {
    if (!view) return;
    const l = view.state.doc.line(Math.max(1, Math.min(line, view.state.doc.lines)));
    view.dispatch({
      selection: { anchor: l.from },
      effects: EditorView.scrollIntoView(l.from, { y: "start", yMargin: 60 }),
    });
    view.focus();
  }

  /** Caret position, to insert at later. */
  export function cursor() {
    return view?.state.selection.main.head ?? 0;
  }

  /** Inserts `text` at `at` (default: the caret) and puts the caret after it. */
  export function insert(text: string, at = cursor()) {
    if (!view) return;
    const pos = Math.min(at, view.state.doc.length);
    view.dispatch({
      changes: { from: pos, insert: text },
      selection: EditorSelection.cursor(pos + text.length),
      userEvent: "input.paste",
    });
  }
</script>

<!-- Leaving means focus left the whole editor, not just moved into its find bar. -->
<div
  class="editor"
  bind:this={host}
  on:focusout={(e) => {
    if (!(e.relatedTarget instanceof Node && host.contains(e.relatedTarget))) dispatch("blur");
  }}
/>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .editor :global(.cm-editor) {
    width: 100%;
  }
</style>
