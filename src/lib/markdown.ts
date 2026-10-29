import { marked } from "marked";
import DOMPurify from "dompurify";
import { dirOf, nameOf } from "./api";

// Set by render() so the wikilink renderer can mark links to missing notes.
let resolveForRender: (target: string) => string | null = () => null;

const escapeHtml = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

marked.use({
  gfm: true,
  breaks: true,
  extensions: [
    {
      // [[Note]], [[folder/Note]], [[Note|label]]
      name: "wikilink",
      level: "inline",
      start(src: string) {
        const i = src.indexOf("[[");
        return i === -1 ? undefined : i;
      },
      tokenizer(src: string) {
        const m = /^\[\[([^[\]|]+)(?:\|([^[\]]+))?\]\]/.exec(src);
        if (!m) return undefined;
        return { type: "wikilink", raw: m[0], target: m[1].trim(), label: (m[2] ?? m[1]).trim() };
      },
      renderer(token) {
        const missing = resolveForRender(token.target) ? "" : " missing";
        return `<a href="#" class="wikilink${missing}" data-wikilink="${escapeHtml(token.target)}">${escapeHtml(token.label)}</a>`;
      },
    },
  ],
});

export function renderMarkdown(src: string, resolve: (target: string) => string | null): string {
  resolveForRender = resolve;
  const html = marked.parse(src, { async: false }) as string;
  return DOMPurify.sanitize(html);
}

/** Collapse "." and ".." segments of a "/"-separated relative path. */
function normalize(path: string): string {
  const out: string[] = [];
  for (const part of path.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") out.pop();
    else out.push(part);
  }
  return out.join("/");
}

/**
 * Find the note a link points to, from the note at `from`. Tries, in order:
 * a path relative to the current folder, a path from the vault root, and
 * (for bare names) any note with that name, preferring the current folder.
 */
export function resolveLink(target: string, from: string, notes: string[]): string | null {
  let t = target.split("#")[0].trim();
  if (!t) return null;
  t = t.replace(/\.md$/i, "");
  const lower = new Set(notes.map((n) => n.toLowerCase()));
  const find = (p: string) =>
    lower.has(p.toLowerCase()) ? notes.find((n) => n.toLowerCase() === p.toLowerCase())! : null;

  const here = dirOf(from);
  const relative = normalize(here ? `${here}/${t}` : t) + ".md";
  const absolute = normalize(t) + ".md";
  const direct = find(relative) ?? find(absolute);
  if (direct || t.includes("/")) return direct;

  const sameName = notes.filter((n) => nameOf(n).toLowerCase() === t.toLowerCase());
  return sameName.find((n) => dirOf(n) === here) ?? sameName[0] ?? null;
}
