// The search syntax, as far as the palette needs it (the backend does the
// searching): words and "quoted phrases" to find, -words to leave out, and
// tag:/#tag and in:folder filters.

export interface ParsedQuery {
  /** Lowercase words and phrases to find (and highlight). */
  words: string[];
  /** Whether tag: or in: filters are used, which only content search understands. */
  filtered: boolean;
}

export function parseQuery(query: string): ParsedQuery {
  const words: string[] = [];
  let filtered = false;
  for (const m of query.toLowerCase().matchAll(/"([^"]*)"?|(\S+)/g)) {
    const token = (m[1] ?? m[2] ?? "").trim();
    if (!token) continue;
    if (m[1] !== undefined) words.push(token);
    else if (/^(tag:|in:|#.)/.test(token)) filtered = true;
    else if (!/^-./.test(token)) words.push(token);
  }
  return { words, filtered };
}

/** `text` split into runs, marking those that match any of `words` (case-insensitive). */
export function highlight(text: string, words: string[]) {
  const out: { text: string; match: boolean }[] = [];
  const lower = text.toLowerCase();
  const terms = words.filter(Boolean).sort((a, b) => b.length - a.length);
  let i = 0;
  let plain = 0;
  while (i < text.length && terms.length) {
    const hit = terms.find((t) => lower.startsWith(t, i));
    if (!hit) {
      i++;
      continue;
    }
    if (i > plain) out.push({ text: text.slice(plain, i), match: false });
    out.push({ text: text.slice(i, i + hit.length), match: true });
    i += hit.length;
    plain = i;
  }
  if (plain < text.length) out.push({ text: text.slice(plain), match: false });
  return out;
}
