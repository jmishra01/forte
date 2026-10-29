import { expect, it } from "vitest";
import { highlight, parseQuery } from "./query";

it("separates words, phrases, exclusions and filters", () => {
  expect(parseQuery('Fox "brown dog" -cat tag:pets')).toEqual({ words: ["fox", "brown dog"], filtered: true });
  expect(parseQuery("plain words")).toEqual({ words: ["plain", "words"], filtered: false });
  expect(parseQuery('"unfinished phrase')).toEqual({ words: ["unfinished phrase"], filtered: false });
  expect(parseQuery("#tag in:work")).toEqual({ words: [], filtered: true });
});

it("highlights every word, longest first", () => {
  const runs = highlight("The Brown fox and brownies", ["brown", "brownies", "fox"]);
  expect(runs.filter((r) => r.match).map((r) => r.text)).toEqual(["Brown", "fox", "brownies"]);
  expect(runs.map((r) => r.text).join("")).toBe("The Brown fox and brownies");
  expect(highlight("abc", [])).toEqual([{ text: "abc", match: false }]);
});
