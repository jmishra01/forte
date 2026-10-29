// Draws $…$ and $$…$$ placeholders left by the markdown renderer. KaTeX (code,
// styles and fonts) is only downloaded the first time a note has math.

import DOMPurify from "dompurify";

let katex: Promise<typeof import("katex").default> | null = null;

function load() {
  katex ??= Promise.all([import("katex"), import("katex/dist/katex.min.css")]).then(([m]) => m.default);
  return katex;
}

/** Renders every `[data-tex]` under `root` in place. */
export async function renderMath(root: HTMLElement) {
  const nodes = Array.from(root.querySelectorAll<HTMLElement>("[data-tex]"));
  if (!nodes.length) return;
  const k = await load();
  for (const el of nodes) {
    if (!el.isConnected && root.isConnected) continue; // the note changed meanwhile
    const tex = el.dataset.tex ?? "";
    const html = k.renderToString(tex, { displayMode: el.dataset.display === "true", throwOnError: false, output: "htmlAndMathml" });
    el.innerHTML = DOMPurify.sanitize(html);
    delete el.dataset.tex; // drawn
  }
}
