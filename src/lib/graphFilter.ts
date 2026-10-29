// Which notes the graph shows, given its filters. Pure, so it can be tested.

export interface GraphFilters {
  /** Only notes with this tag (or one nested under it). */
  tag: string;
  /** Only notes in this folder (or below it). */
  folder: string;
  hideUnlinked: boolean;
  /** Only notes within this many links of `center` (0 = no limit). */
  depth: number;
  center: string | null;
}

export const noFilters: GraphFilters = { tag: "", folder: "", hideUnlinked: false, depth: 0, center: null };

export function visibleNotes(
  notes: string[],
  edges: [string, string][],
  tagsOf: (path: string) => string[],
  f: GraphFilters,
): Set<string> {
  let keep = notes.filter(
    (p) =>
      (!f.tag || tagsOf(p).some((t) => t === f.tag || t.startsWith(f.tag + "/"))) &&
      (!f.folder || p.startsWith(f.folder + "/")),
  );
  if (f.depth > 0 && f.center) {
    const near = new Set([f.center]);
    let frontier = [f.center];
    for (let d = 0; d < f.depth; d++) {
      const next: string[] = [];
      for (const [a, b] of edges) {
        for (const [from, to] of [[a, b], [b, a]]) {
          if (frontier.includes(from) && !near.has(to)) {
            near.add(to);
            next.push(to);
          }
        }
      }
      frontier = next;
    }
    keep = keep.filter((p) => near.has(p));
  }
  const kept = new Set(keep);
  if (f.hideUnlinked) {
    const linked = new Set(edges.filter(([a, b]) => kept.has(a) && kept.has(b)).flat());
    for (const p of kept) if (!linked.has(p)) kept.delete(p);
  }
  return kept;
}
