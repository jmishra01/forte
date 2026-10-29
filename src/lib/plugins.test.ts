import { expect, it } from "vitest";
import { check, isNotePath } from "./plugins";

it("allows requests only with the matching permission", () => {
  expect(check("notify", ["hi"], [])).toBeNull();
  expect(check("currentNote", [], ["editor"])).toBeNull();
  expect(check("currentNote", [], [])).toMatch(/editor/);
  expect(check("readNote", ["a.md"], ["editor"])).toMatch(/notes:read/);
  expect(check("writeNote", ["a.md", "x"], ["notes:read"])).toMatch(/notes:write/);
  expect(check("writeNote", ["a.md", "x"], ["notes:write"])).toBeNull();
});

it("refuses unknown requests, odd arguments and non-note paths", () => {
  expect(check("deleteEverything", [], ["notes:write"])).toMatch(/unknown/);
  expect(check("readNote", [{ path: "a.md" }], ["notes:read"])).toMatch(/text/);
  expect(check("writeNote", ["a.md"], ["notes:write"])).toMatch(/needs text/);
  expect(check("writeNote", [".forte/plugins/x/plugin.js", "x"], ["notes:write"])).toMatch(/note path/);
  expect(check("readNote", ["../outside.md"], ["notes:read"])).toMatch(/note path/);
});

it("recognises note paths", () => {
  expect(isNotePath("work/Plan.md")).toBe(true);
  expect(isNotePath("Plan.txt")).toBe(false);
  expect(isNotePath(".trash/Plan.md")).toBe(false);
  expect(isNotePath("a//b.md")).toBe(false);
});
