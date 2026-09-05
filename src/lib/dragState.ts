import { writable } from "svelte/store";

export type DropZone = "before" | "after" | "inside";

export interface DragState {
  draggingId: string | null;
  overId: string | null;
  overZone: DropZone | null;
}

export interface DropResult {
  draggedId: string;
  overId: string;
  overZone: DropZone;
}

/**
 * Shared across every NoteTreeLevel instance (regardless of recursion depth)
 * so a row anywhere in the tree can show hover feedback while a drag
 * initiated in a completely different instance is in progress.
 */
export const dragState = writable<DragState>({ draggingId: null, overId: null, overZone: null });

/** Set once, on pointerup, when a drag ended over a valid target; the owning
 * NoteTree consumes it (it has the full-tree data needed to compute a new
 * position) and clears it back to null. */
export const dropResult = writable<DropResult | null>(null);

/** Sentinel overId meaning "the root drop zone", not an actual note. */
export const ROOT_ZONE_ID = "__root__";
