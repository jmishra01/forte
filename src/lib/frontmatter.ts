/** A `key: value` from front matter; lists become arrays. */
export type Property = { key: string; value: string | string[] };

/**
 * Splits leading YAML front matter (between `---` lines) from the body.
 * Understands the simple subset notes use: `key: value`, `key: [a, b]`, and
 * `key:` followed by indented `- item` lines. Anything else is kept as text.
 */
export function splitFrontMatter(text: string): { props: Property[]; body: string; lines: number } {
  const m = /^---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(text);
  if (!m) return { props: [], body: text, lines: 0 };
  const props: Property[] = [];
  for (const line of m[1].split(/\r?\n/)) {
    const item = /^\s+-\s+(.*)$/.exec(line);
    const last = props[props.length - 1];
    if (item && last && Array.isArray(last.value)) {
      last.value.push(unquote(item[1]));
      continue;
    }
    const kv = /^([\w-]+)\s*:\s*(.*)$/.exec(line);
    if (!kv) continue;
    const raw = kv[2].trim();
    if (!raw) props.push({ key: kv[1], value: [] });
    else if (/^\[.*\]$/.test(raw)) props.push({ key: kv[1], value: splitList(raw.slice(1, -1)) });
    else props.push({ key: kv[1], value: unquote(raw) });
  }
  return { props, body: text.slice(m[0].length), lines: m[0].split("\n").length - 1 };
}

const unquote = (s: string) => s.trim().replace(/^(["'])(.*)\1$/, "$2");
const splitList = (s: string) =>
  s
    .split(",")
    .map(unquote)
    .filter((x) => x);
