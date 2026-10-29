import { nameOf, type Hit, type NoteInfo } from "./api";
import { relativeHref, resolveLink, resolveRelative } from "./markdown";

/** The note a link from `from` points at, if it exists. */
export function linkTarget(l: NoteInfo["links"][number], from: string, notes: string[]): string | null {
  if (l.kind === "wiki") return resolveLink(l.target, from, notes);
  const rel = resolveRelative(l.target, from);
  return rel && notes.includes(rel) ? rel : null;
}

/** Lines in other notes whose links point at `path`. */
export function backlinks(path: string, index: NoteInfo[], notes: string[]): Hit[] {
  const out: Hit[] = [];
  for (const note of index) {
    if (note.path === path) continue;
    const seen = new Set<number>();
    for (const l of note.links) {
      if (seen.has(l.line) || linkTarget(l, note.path, notes) !== path) continue;
      seen.add(l.line);
      out.push({ path: note.path, line: l.line, text: l.text });
    }
  }
  return out;
}

/** Shortest link text that still points at `path` among `notes`: the bare name unless another note shares it. */
export function linkFor(path: string, notes: string[]) {
  const name = nameOf(path);
  return notes.filter((n) => nameOf(n) === name).length === 1 ? name : path.replace(/\.md$/, "");
}

/** Same pattern as the preview's wikilink tokenizer. */
const WIKILINK = /\[\[([^[\]|]+)(\|[^[\]]+)?\]\]/g;

/** `](path.md#part)` or `](<path.md#part>)`, as the Rust index reads them. */
const MDLINK = /\]\((?:<([^>]+\.md)(#[^>]*)?>|([^()\s<>]+?\.md)(#[^()\s<>]*)?)(?=[\s)])/gi;

/**
 * After notes were renamed or moved (`moved` maps old path -> new path),
 * returns the new text of each note whose links would otherwise break,
 * keyed by its new path. Links that still resolve are left as written.
 */
export function rewriteLinks(
  moved: Map<string, string>,
  oldNotes: string[],
  newNotes: string[],
  texts: Map<string, string>, // old path -> content, for the notes that may need changes
): Map<string, string> {
  const out = new Map<string, string>();
  for (const [oldSource, text] of texts) {
    const newSource = moved.get(oldSource) ?? oldSource;
    let changed = false;
    const updated = text.replace(WIKILINK, (whole, rawTarget: string, label = "") => {
      const [target, heading = ""] = splitHeading(rawTarget.trim());
      const oldTarget = resolveLink(target, oldSource, oldNotes);
      if (!oldTarget) return whole;
      const newTarget = moved.get(oldTarget) ?? oldTarget;
      if (resolveLink(target, newSource, newNotes) === newTarget) return whole;
      changed = true;
      return `[[${linkFor(newTarget, newNotes)}${heading}${label}]]`;
    });
    // Markdown links are relative to the note, so they can break when either end moves.
    const final = updated.replace(MDLINK, (whole, angled, angledPart = "", plain, plainPart = "") => {
      const written: string = angled ?? plain;
      const oldTarget = resolveRelative(written, oldSource);
      if (!oldTarget || !oldNotes.includes(oldTarget)) return whole;
      const newTarget = moved.get(oldTarget) ?? oldTarget;
      if (resolveRelative(written, newSource) === newTarget) return whole;
      changed = true;
      const href = relativeHref(newTarget, newSource);
      return angled ? `](<${decodeURIComponent(href)}${angledPart}>` : `](${href}${plainPart}`;
    });
    if (changed) out.set(newSource, final);
  }
  return out;
}

function splitHeading(target: string): [string, string] {
  const i = target.indexOf("#");
  return i === -1 ? [target, ""] : [target.slice(0, i), target.slice(i)];
}
