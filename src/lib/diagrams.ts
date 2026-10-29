// Renders ```mermaid code blocks inside rendered markdown as diagrams.
// Mermaid is large, so it is only loaded the first time a note has a diagram.

const cache = new Map<string, string>(); // theme + source -> svg
let nextId = 0;

/**
 * Draws each `<pre><code class="language-mermaid">` under `root` in place.
 * The `<pre>` element itself is kept (only its contents change) because
 * Svelte's {@html} block still owns it and removes it on the next render.
 */
export async function renderDiagrams(root: HTMLElement) {
  const blocks = Array.from(root.querySelectorAll<HTMLElement>("pre > code.language-mermaid"));
  if (blocks.length === 0) return;

  const theme = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "default";
  const { default: mermaid } = await import("mermaid");
  mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme });

  for (const code of blocks) {
    const pre = code.parentElement!;
    const source = code.textContent ?? "";
    const key = theme + "\n" + source;
    let svg = cache.get(key);
    if (!svg) {
      const id = `mermaid-${nextId++}`;
      try {
        svg = (await mermaid.render(id, source)).svg;
        cache.set(key, svg);
      } catch (e) {
        // Mermaid leaves its error graphic in <body> when parsing fails.
        document.getElementById("d" + id)?.remove();
        const error = document.createElement("div");
        error.className = "diagram-error";
        error.textContent = `Diagram error: ${e instanceof Error ? e.message : e}`;
        pre.appendChild(error); // inside <pre> so it goes away with it
        continue;
      }
    }
    if (!pre.isConnected) return; // note changed while rendering
    pre.className = "diagram";
    pre.innerHTML = svg;
  }
}
