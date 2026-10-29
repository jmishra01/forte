/** Which side of each `<<<<<<<` … `>>>>>>>` block to keep. */
export type Side = "mine" | "theirs" | "both";

const BLOCK = /^<<<<<<<[^\n]*\n([\s\S]*?)(?:^\|\|\|\|\|\|\|[^\n]*\n[\s\S]*?)?^=======[ \t]*\n([\s\S]*?)^>>>>>>>[^\n]*(?:\n|$)/gm;

export function hasMarkers(text: string) {
  BLOCK.lastIndex = 0;
  return BLOCK.test(text);
}

/**
 * Resolves every Git conflict block in `text`. "mine" is the version from this
 * computer (before `=======`), "theirs" the one that came in from the remote.
 */
export function resolveMarkers(text: string, side: Side) {
  return text.replace(BLOCK, (_whole, mine: string, theirs: string) =>
    side === "mine" ? mine : side === "theirs" ? theirs : mine + theirs,
  );
}
