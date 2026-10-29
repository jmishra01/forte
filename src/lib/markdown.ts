import { marked } from "marked";
import DOMPurify from "dompurify";
import katex from "katex";
import "katex/dist/katex.min.css";
import { dirOf, nameOf } from "./api";
import { splitFrontMatter } from "./frontmatter";

// Set by render() so the wikilink renderer can mark links to missing notes,
// and the image renderer can point relative images at files in the vault.
let resolveForRender: (target: string) => string | null = () => null;
let imageSrcForRender: (href: string) => string = (href) => href;

const tex = (src: string, displayMode: boolean) =>
  katex.renderToString(src, { displayMode, throwOnError: false, output: "htmlAndMathml" });

const escapeHtml = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

marked.use({
  gfm: true,
  breaks: true,
  useNewRenderer: true, // renderers get the token object, as in marked 14+
  renderer: {
    image({ href, title, text }) {
      const t = title ? ` title="${escapeHtml(title)}"` : "";
      return `<img src="${escapeHtml(imageSrcForRender(href))}" alt="${escapeHtml(text)}"${t}>`;
    },
  },
  extensions: [
    {
      // $$ ... $$ on its own lines
      name: "mathBlock",
      level: "block",
      start(src: string) {
        const i = src.indexOf("$$");
        return i === -1 ? undefined : i;
      },
      tokenizer(src: string) {
        const m = /^\$\$([\s\S]+?)\$\$[ \t]*(?:\n|$)/.exec(src);
        if (!m) return undefined;
        return { type: "mathBlock", raw: m[0], text: m[1].trim() };
      },
      renderer(token) {
        return `<div class="math">${tex(token.text, true)}</div>`;
      },
    },
    {
      // $x$, but not "$5 and $10": no space just inside the dollars, no digit right after.
      name: "math",
      level: "inline",
      start(src: string) {
        const i = src.indexOf("$");
        return i === -1 ? undefined : i;
      },
      tokenizer(src: string) {
        const m = /^\$(?!\s)((?:\\.|[^\\$\n])+?)\$(?!\d)/.exec(src);
        if (!m || /\s$/.test(m[1])) return undefined;
        return { type: "math", raw: m[0], text: m[1] };
      },
      renderer(token) {
        return tex(token.text, false);
      },
    },
    {
      // #tag after whitespace: a letter or _, then letters, digits, _, - or /.
      name: "tag",
      level: "inline",
      start(src: string) {
        const m = /(^|\s)#[\p{L}_]/u.exec(src);
        return m ? m.index + m[1].length : undefined;
      },
      tokenizer(src: string) {
        const m = /^#([\p{L}_][\p{L}\p{N}_/-]*)/u.exec(src);
        if (!m) return undefined;
        const tag = m[1].replace(/\/+$/, "");
        return { type: "tag", raw: "#" + tag, tag };
      },
      renderer(token) {
        return `<a href="#" class="tag" data-tag="${escapeHtml(token.tag.toLowerCase())}">#${escapeHtml(token.tag)}</a>`;
      },
    },
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

// DOMPurify's default URL check, plus asset: (how Tauri serves vault images) and
// vault: (how exported pages mark images to embed).
const ALLOWED_URI_REGEXP =
  /^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp|matrix|asset|vault):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i;

export function renderMarkdown(
  src: string,
  resolve: (target: string) => string | null,
  imageSrc: (href: string) => string = (href) => href,
): string {
  resolveForRender = resolve;
  imageSrcForRender = imageSrc;
  const { props, body } = splitFrontMatter(src);
  const html = propertiesHtml(props) + (marked.parse(body, { async: false }) as string);
  return DOMPurify.sanitize(html, { ALLOWED_URI_REGEXP });
}

/** Front matter as a small table; tags become clickable like inline #tags. */
function propertiesHtml(props: ReturnType<typeof splitFrontMatter>["props"]) {
  if (!props.length) return "";
  const rows = props.map(({ key, value }) => {
    const items = Array.isArray(value) ? value : [value];
    const cells = /^tags?$/.test(key)
      ? items
          .map((t) => t.replace(/^#/, ""))
          .map((t) => `<a href="#" class="tag" data-tag="${escapeHtml(t.toLowerCase())}">#${escapeHtml(t)}</a>`)
          .join(" ")
      : items.map(escapeHtml).join(", ");
    return `<tr><th>${escapeHtml(key)}</th><td>${cells}</td></tr>`;
  });
  return `<table class="properties">${rows.join("")}</table>`;
}

/**
 * Vault-relative path of a relative image or link `href` written in the note at `from`,
 * or null for absolute URLs and paths outside the vault.
 */
export function resolveRelative(href: string, from: string): string | null {
  if (/^[a-z][a-z+.-]*:/i.test(href) || href.startsWith("/") || href.startsWith("#")) return null;
  let path: string;
  try {
    path = decodeURIComponent(href.split(/[?#]/)[0]);
  } catch {
    return null;
  }
  const here = dirOf(from);
  if ((here ? here.split("/").length : 0) < leadingUps(path)) return null;
  return normalize(here ? `${here}/${path}` : path) || null;
}

const leadingUps = (path: string) => /^(?:\.\.\/)*/.exec(path)![0].length / 3;

/** Relative href from the note at `from` to the vault-relative `path`, URL-encoded. */
export function relativeHref(path: string, from: string): string {
  const fromParts = dirOf(from) ? dirOf(from).split("/") : [];
  const toParts = path.split("/");
  let same = 0;
  while (same < fromParts.length && same < toParts.length - 1 && fromParts[same] === toParts[same]) same++;
  const ups = fromParts.slice(same).map(() => "..");
  return [...ups, ...toParts.slice(same)].map(encodeURIComponent).join("/");
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
