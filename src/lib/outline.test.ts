import { expect, it } from "vitest";
import { headings } from "./outline";

it("lists ATX headings outside code and front matter", () => {
  const text = "---\ntitle: x\n---\n# One\n\n```\n# not a heading\n```\n## Two *em* ##\n~~~\n### no\n~~~\n### Three";
  expect(headings(text)).toEqual([
    { level: 1, text: "One", line: 4 },
    { level: 2, text: "Two em", line: 9 },
    { level: 3, text: "Three", line: 13 },
  ]);
});
