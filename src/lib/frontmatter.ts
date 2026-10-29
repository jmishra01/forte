import { parse } from "yaml";

/** A front matter entry, flattened for display: lists of plain values stay lists. */
export type Property = { key: string; value: string | string[] };

/**
 * Splits leading YAML front matter (between `---` lines) from the body and
 * reads it with a full YAML parser. Front matter that isn't valid YAML is read
 * line by line instead (`key: value`, `key: [a, b]`, `- item`), so a typo
 * doesn't hide everything.
 */
export function splitFrontMatter(text: string): { props: Property[]; body: string; lines: number } {
  const m = /^---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/.exec(text);
  if (!m) return { props: [], body: text, lines: 0 };
  return { props: yamlProps(m[1]) ?? lineProps(m[1]), body: text.slice(m[0].length), lines: m[0].split("\n").length - 1 };
}

function yamlProps(src: string): Property[] | null {
  let data: unknown;
  try {
    data = parse(src, { prettyErrors: false });
  } catch {
    return null;
  }
  if (data === null || data === undefined) return [];
  if (typeof data !== "object" || Array.isArray(data)) return null;
  return Object.entries(data as Record<string, unknown>).map(([key, value]) => ({ key, value: display(value) }));
}

/** Scalars as text, lists of scalars as lists, anything nested as compact text. */
function display(v: unknown): string | string[] {
  if (Array.isArray(v) && v.every((x) => x === null || typeof x !== "object")) return v.map((x) => (x === null ? "" : String(x)));
  if (v === null || v === undefined) return "";
  if (v instanceof Date) return v.toISOString().slice(0, 10);
  if (typeof v === "object") return compact(v);
  return String(v);
}

function compact(v: unknown): string {
  if (Array.isArray(v)) return v.map(compact).join(", ");
  if (v && typeof v === "object") {
    return Object.entries(v as Record<string, unknown>)
      .map(([k, x]) => `${k}: ${x && typeof x === "object" ? `(${compact(x)})` : String(x)}`)
      .join(", ");
  }
  return String(v);
}

function lineProps(src: string): Property[] {
  const props: Property[] = [];
  for (const line of src.split(/\r?\n/)) {
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
  return props;
}

const unquote = (s: string) => s.trim().replace(/^(["'])(.*)\1$/, "$2");
const splitList = (s: string) =>
  s
    .split(",")
    .map(unquote)
    .filter((x) => x);

/** Tags from a `tags:` value, which may be a list or "a, b" / "a b" text. */
export function tagList(value: string | string[]): string[] {
  const items = Array.isArray(value) ? value : value.split(/[,\s]+/);
  return items.map((t) => t.trim().replace(/^#/, "")).filter(Boolean);
}
