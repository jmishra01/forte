// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { relativeHref, renderMarkdown, resolveLink, resolveRelative } from "./markdown";

const notes = ["Home.md", "work/Plan.md", "work/Ideas.md", "Plan.md"];
const render = (src: string) => renderMarkdown(src, (t) => resolveLink(t, "work/Ideas.md", notes));

describe("renderMarkdown", () => {
  it("tags blocks with their source line, after front matter", () => {
    const html = render("---\na: 1\n---\n# H\n\npara\n\n- x\n");
    expect([...html.matchAll(/<(\w+) data-line="(\d+)"/g)].map((m) => `${m[1]}@${m[2]}`)).toEqual(["h1@4", "p@6", "ul@8"]);
  });

  it("renders tags, wikilinks, missing links and math", () => {
    const html = render("#idea [[Plan]] [[Nope]] $x^2$ costs $5 and $10");
    expect(html).toContain('data-tag="idea"');
    expect(html).toMatch(/class="wikilink" data-wikilink="Plan"/);
    expect(html).toMatch(/class="wikilink missing" data-wikilink="Nope"/);
    expect(html).toContain('<span class="math-inline" data-tex="x^2">');
    expect(html).toContain("$5 and $10");
  });

  it("removes scripts and event handlers", () => {
    const html = render('<script>alert(1)</script><img src=x onerror="alert(2)">\n\n[x](javascript:alert(3))');
    expect(html).not.toMatch(/<script|onerror|href="javascript:/);
    expect(html).toContain("<a>x</a>"); // the link text stays, the javascript: target goes
  });

  it("points relative images at the vault and keeps vault: for export", () => {
    const html = renderMarkdown("![a](../pic.png)", () => null, (href) => `notefile://localhost/${href}`);
    expect(html).toContain('src="notefile://localhost/../pic.png"');
    expect(renderMarkdown("![a](other:pic.png)", () => null)).not.toContain("other:");
    expect(renderMarkdown("![a](vault:pic.png)", () => null)).toContain('src="vault:pic.png"');
  });
});

describe("paths", () => {
  it("resolves links the way notes are written", () => {
    expect(resolveLink("Plan", "work/Ideas.md", notes)).toBe("work/Plan.md"); // same folder first
    expect(resolveLink("Plan", "Home.md", notes)).toBe("Plan.md");
    expect(resolveLink("../Home", "work/Ideas.md", notes)).toBe("Home.md");
    expect(resolveLink("plan#part", "Home.md", notes)).toBe("Plan.md");
    expect(resolveLink("work/Missing", "Home.md", notes)).toBeNull();
  });

  it("converts between relative hrefs and vault paths", () => {
    expect(resolveRelative("../My%20Note.md#x", "work/Ideas.md")).toBe("My Note.md");
    expect(resolveRelative("https://e.com/a.md", "Home.md")).toBeNull();
    expect(resolveRelative("../../out.md", "work/Ideas.md")).toBeNull();
    expect(relativeHref("attachments/a b.png", "work/Ideas.md")).toBe("../attachments/a%20b.png");
    expect(relativeHref("work/Plan.md", "work/Ideas.md")).toBe("Plan.md");
  });
});

describe("math", () => {
  it("draws placeholders with KaTeX, loaded on first use", async () => {
    const { renderMath } = await import("./math");
    const root = document.createElement("div");
    root.innerHTML = renderMarkdown("Inline $a^2$ and\n\n$$\n\\frac{1}{2}\n$$", () => null);
    document.body.append(root);
    await renderMath(root);
    expect(root.querySelectorAll(".katex").length).toBe(2);
    expect(root.querySelector(".katex-display")).not.toBeNull();
    expect(root.querySelector("[data-tex]")).toBeNull();
    root.remove();
  });
});
