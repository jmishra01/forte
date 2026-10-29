import { splitFrontMatter } from "./frontmatter";

export type Heading = { level: number; text: string; line: number };

/** ATX headings (`# ...`) outside code blocks and front matter, with 1-based lines. */
export function headings(text: string): Heading[] {
  const out: Heading[] = [];
  const skip = splitFrontMatter(text).lines;
  let fence: string | null = null;
  text.split("\n").forEach((line, i) => {
    if (i < skip) return;
    const f = /^\s{0,3}(`{3,}|~{3,})/.exec(line);
    if (f) {
      if (!fence) fence = f[1][0];
      else if (f[1][0] === fence) fence = null;
      return;
    }
    if (fence) return;
    const h = /^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (h && h[2]) out.push({ level: h[1].length, text: h[2].replace(/[*_`]/g, ""), line: i + 1 });
  });
  return out;
}
