import { nameOf, type Hit, type NoteInfo } from "./api";
import { resolveLink } from "./markdown";

/** Lines in other notes whose `[[links]]` point at `path`. */
export function backlinks(path: string, index: NoteInfo[], notes: string[]): Hit[] {
  const out: Hit[] = [];
  for (const note of index) {
    if (note.path === path) continue;
    const seen = new Set<number>();
    for (const l of note.links) {
      if (seen.has(l.line) || resolveLink(l.target, note.path, notes) !== path) continue;
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
    if (changed) out.set(newSource, updated);
  }
  return out;
}

function splitHeading(target: string): [string, string] {
  const i = target.indexOf("#");
  return i === -1 ? [target, ""] : [target.slice(0, i), target.slice(i)];
}
