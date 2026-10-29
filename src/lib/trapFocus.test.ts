// @vitest-environment jsdom
import { expect, it } from "vitest";
import { trapFocus } from "./trapFocus";

it("wraps Tab inside the dialog and restores focus after", () => {
  document.body.innerHTML = `<button id="outside">x</button><div id="d"><input id="a"><button id="b">b</button></div>`;
  const outside = document.getElementById("outside")!;
  outside.focus();
  const dialog = document.getElementById("d")!;
  const trap = trapFocus(dialog);
  const tab = (shiftKey = false) => dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey, bubbles: true, cancelable: true }));

  document.getElementById("b")!.focus();
  tab();
  expect(document.activeElement?.id).toBe("a");
  tab(true);
  expect(document.activeElement?.id).toBe("b");

  trap.destroy();
  expect(document.activeElement).toBe(outside);
});
