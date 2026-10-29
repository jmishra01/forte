import { expect, it } from "vitest";
import { noFilters, visibleNotes } from "./graphFilter";

const notes = ["a.md", "b.md", "c.md", "work/d.md", "work/e.md", "lonely.md"];
const edges: [string, string][] = [["a.md", "b.md"], ["b.md", "c.md"], ["work/d.md", "work/e.md"]];
const tags: Record<string, string[]> = { "a.md": ["project/x"], "work/d.md": ["project"], "c.md": ["other"] };
const show = (f: Partial<typeof noFilters>) => [...visibleNotes(notes, edges, (p) => tags[p] ?? [], { ...noFilters, ...f })].sort();

it("shows everything without filters", () => expect(show({})).toEqual([...notes].sort()));
it("filters by tag, including nested tags", () => expect(show({ tag: "project" })).toEqual(["a.md", "work/d.md"]));
it("filters by folder", () => expect(show({ folder: "work" })).toEqual(["work/d.md", "work/e.md"]));
it("hides unlinked notes", () => expect(show({ hideUnlinked: true })).not.toContain("lonely.md"));
it("keeps only notes near the open one", () => {
  expect(show({ center: "a.md", depth: 1 })).toEqual(["a.md", "b.md"]);
  expect(show({ center: "a.md", depth: 2 })).toEqual(["a.md", "b.md", "c.md"]);
});
it("combines filters: links to hidden notes don't count", () =>
  expect(show({ tag: "project", hideUnlinked: true })).toEqual([]));
