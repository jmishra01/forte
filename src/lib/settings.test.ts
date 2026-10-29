import { expect, it } from "vitest";
import { defaults, withDefaults } from "./settings";

it("keeps known settings of the right type and drops the rest", () => {
  const s = withDefaults({ theme: "dark", textSize: 3, unknown: true, showOutline: false });
  expect(s.theme).toBe("dark");
  expect(s.textSize).toBe(defaults.textSize);
  expect(s.showOutline).toBe(false);
  expect("unknown" in s).toBe(false);
  expect(withDefaults(null)).toEqual(defaults);
  expect(withDefaults({ plugins: null }).plugins).toEqual({});
});
