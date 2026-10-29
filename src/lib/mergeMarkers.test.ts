import { describe, expect, it } from "vitest";
import { hasMarkers, resolveMarkers } from "./mergeMarkers";

const text =
  "top\n<<<<<<< HEAD\nmine 1\nmine 2\n=======\ntheirs\n>>>>>>> origin/main\nmid\n" +
  "<<<<<<< HEAD\na\n||||||| base\nold\n=======\nb\n>>>>>>> x\nend\n";

describe("merge markers", () => {
  it("detects blocks, including diff3 style", () => {
    expect(hasMarkers(text)).toBe(true);
    expect(hasMarkers("a\n=======\nb")).toBe(false);
  });

  it("keeps the chosen side of every block", () => {
    expect(resolveMarkers(text, "mine")).toBe("top\nmine 1\nmine 2\nmid\na\nend\n");
    expect(resolveMarkers(text, "theirs")).toBe("top\ntheirs\nmid\nb\nend\n");
    expect(resolveMarkers(text, "both")).toBe("top\nmine 1\nmine 2\ntheirs\nmid\na\nb\nend\n");
    expect(hasMarkers(resolveMarkers(text, "mine"))).toBe(false);
  });
});
