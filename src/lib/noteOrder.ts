import type { NoteMeta } from "./types";

/** Sibling ordering key: explicit manual position if set, else creation time. */
export function posOf(n: NoteMeta): number {
  return n.position ?? new Date(n.createdAt).getTime();
}
