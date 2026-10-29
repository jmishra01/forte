import { expect, it } from "vitest";
import { expandTemplate, isoDate } from "./templates";

it("fills title, date and time, leaving unknown placeholders", () => {
  const now = new Date(2026, 9, 29, 9, 5);
  expect(isoDate(now)).toBe("2026-10-29");
  expect(expandTemplate("# {{title}} {{ date }} {{TIME}} {{other}}", "Plan", now)).toBe("# Plan 2026-10-29 09:05 {{other}}");
});
