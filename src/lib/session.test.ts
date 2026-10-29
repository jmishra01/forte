// @vitest-environment jsdom
import { beforeEach, expect, it } from "vitest";
import { loadSession, saveSession } from "./session";

beforeEach(() => localStorage.clear());

it("restores tabs per vault, dropping notes that are gone", () => {
  saveSession("/v1", { tabs: ["a.md", "gone.md", "b.md"], current: "gone.md" });
  saveSession("/v2", { tabs: ["x.md"], current: "x.md" });
  expect(loadSession("/v1", ["a.md", "b.md"])).toEqual({ tabs: ["a.md", "b.md"], current: "a.md" });
  expect(loadSession("/v2", ["x.md"])).toEqual({ tabs: ["x.md"], current: "x.md" });
  expect(loadSession("/v3", ["a.md"])).toBeNull();
});

it("survives junk in storage", () => {
  localStorage.setItem("forte.session:/v", "{not json");
  expect(loadSession("/v", ["a.md"])).toBeNull();
  localStorage.setItem("forte.session:/v", JSON.stringify({ tabs: "nope" }));
  expect(loadSession("/v", ["a.md"])).toBeNull();
});
