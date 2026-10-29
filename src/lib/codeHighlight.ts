// Colours ```lang code blocks in the preview with the same parsers the editor
// uses. Each language's parser is downloaded the first time a note uses it.

import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { classHighlighter, highlightCode } from "@lezer/highlight";

/** Highlights each `<pre><code class="language-x">` under `root` in place (mermaid is left to diagrams.ts). */
export async function highlightCodeBlocks(root: HTMLElement) {
  const blocks = root.querySelectorAll<HTMLElement>("pre > code[class*='language-']:not(.language-mermaid)");
  for (const code of Array.from(blocks)) {
    if (code.dataset.highlighted) continue;
    const name = /language-([\w+#.-]+)/.exec(code.className)?.[1];
    const desc = name ? LanguageDescription.matchLanguageName(languages, name, true) : null;
    if (!desc) continue;
    const support = await desc.load().catch(() => null);
    if (!support || !code.isConnected) continue; // unknown language, or the note changed meanwhile
    const text = code.textContent ?? "";
    // Built from text nodes, never HTML, so code can't inject markup.
    const out = document.createDocumentFragment();
    highlightCode(
      text,
      support.language.parser.parse(text),
      classHighlighter,
      (piece, classes) => {
        if (!classes) return out.append(piece);
        const span = document.createElement("span");
        span.className = classes;
        span.textContent = piece;
        out.append(span);
      },
      () => out.append("\n"),
    );
    code.replaceChildren(out);
    code.dataset.highlighted = "true";
  }
}
