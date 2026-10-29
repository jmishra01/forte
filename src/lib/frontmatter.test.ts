import { expect, it } from "vitest";
import { splitFrontMatter, tagList } from "./frontmatter";

it("reads full YAML: lists, nesting, numbers, booleans, block text", () => {
  const { props, body, lines } = splitFrontMatter(
    "---\ntitle: 'Plan: Q4'\ntags:\n  - work\n  - forte\ndone: false\npriority: 2\nauthor:\n  name: Ann\n  team: core\nsummary: |\n  two\n  lines\n---\nBody\n",
  );
  expect(props).toEqual([
    { key: "title", value: "Plan: Q4" },
    { key: "tags", value: ["work", "forte"] },
    { key: "done", value: "false" },
    { key: "priority", value: "2" },
    { key: "author", value: "name: Ann, team: core" },
    { key: "summary", value: "two\nlines\n" },
  ]);
  expect(body).toBe("Body\n");
  expect(lines).toBe(14);
});

it("falls back to reading lines when the YAML is invalid", () => {
  const { props } = splitFrontMatter("---\ntitle: a: b: [\ntags: [x, y]\n---\n");
  expect(props.find((p) => p.key === "tags")?.value).toEqual(["x", "y"]);
});

it("needs front matter at the very top", () => {
  expect(splitFrontMatter("text\n---\na: 1\n---\n").props).toEqual([]);
  expect(splitFrontMatter("---\n---\nbody").props).toEqual([]);
});

it("reads tags written as a list or as text", () => {
  expect(tagList(["#a", "b"])).toEqual(["a", "b"]);
  expect(tagList("a, b c")).toEqual(["a", "b", "c"]);
});
