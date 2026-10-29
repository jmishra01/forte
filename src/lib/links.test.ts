import { describe, expect, it } from "vitest";
import { backlinks, linkFor, rewriteLinks } from "./links";

const notes = ["Home.md", "work/Plan.md", "work/Ideas.md", "My Note.md"];

describe("rewriteLinks", () => {
  it("fixes wiki and markdown links to a renamed note, keeping labels and headings", () => {
    const moved = new Map([["work/Plan.md", "work/Roadmap.md"]]);
    const after = ["Home.md", "work/Roadmap.md", "work/Ideas.md", "My Note.md"];
    const out = rewriteLinks(
      moved,
      notes,
      after,
      new Map([
        ["Home.md", "See [plan](work/Plan.md#goals), [[work/Plan|the plan]] and [web](https://x.com/Plan.md)."],
        ["work/Ideas.md", 'Back to [plan](Plan.md "title") and [home](../Home.md).'],
      ]),
    );
    expect(out.get("Home.md")).toBe(
      "See [plan](work/Roadmap.md#goals), [[Roadmap|the plan]] and [web](https://x.com/Plan.md).",
    );
    expect(out.get("work/Ideas.md")).toBe('Back to [plan](Roadmap.md "title") and [home](../Home.md).');
  });

  it("redoes a moved note's own relative links", () => {
    const out = rewriteLinks(
      new Map([["work/Ideas.md", "Ideas.md"]]),
      notes,
      ["Home.md", "work/Plan.md", "Ideas.md", "My Note.md"],
      new Map([["work/Ideas.md", "[p](Plan.md) [h](../Home.md) [n](<../My Note.md>) [[Plan]]"]]),
    );
    expect(out.get("Ideas.md")).toBe("[p](work/Plan.md) [h](Home.md) [n](<My Note.md>) [[Plan]]");
  });

  it("handles encoded and angle-bracket paths with spaces", () => {
    const out = rewriteLinks(
      new Map([["My Note.md", "Your Note.md"]]),
      notes,
      ["Home.md", "work/Plan.md", "work/Ideas.md", "Your Note.md"],
      new Map([["Home.md", "[a](My%20Note.md) [b](<My Note.md#x>)"]]),
    );
    expect(out.get("Home.md")).toBe("[a](Your%20Note.md) [b](<Your Note.md#x>)");
  });

  it("leaves notes whose links still work alone", () => {
    const out = rewriteLinks(
      new Map([["Home.md", "Start.md"]]),
      notes,
      ["Start.md", "work/Plan.md", "work/Ideas.md", "My Note.md"],
      new Map([["work/Ideas.md", "[p](Plan.md) [[Plan]]"]]),
    );
    expect(out.size).toBe(0);
  });
});

describe("backlinks and linkFor", () => {
  it("counts wiki and markdown links, once per line", () => {
    const index = [
      {
        path: "Home.md",
        tags: [],
        links: [
          { target: "work/Plan.md", line: 3, text: "see", kind: "md" as const },
          { target: "Plan", line: 3, text: "see", kind: "wiki" as const },
          { target: "Plan", line: 5, text: "again", kind: "wiki" as const },
        ],
      },
    ];
    expect(backlinks("work/Plan.md", index, notes).map((h) => h.line)).toEqual([3, 5]);
  });

  it("uses the bare name unless another note shares it", () => {
    expect(linkFor("work/Plan.md", notes)).toBe("Plan");
    expect(linkFor("work/Plan.md", [...notes, "Plan.md"])).toBe("work/Plan");
  });
});
