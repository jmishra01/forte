// A standalone, light-themed HTML page for a rendered note. Math is shown with
// the browser's MathML (KaTeX's HTML needs its fonts), diagrams are inline SVG,
// and images marked `vault:` are embedded by the backend.

const CSS = `
body { margin: 0; background: #fff; color: #1b1b1f;
  font: 16px/1.65 -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, Roboto, sans-serif; }
main { max-width: 760px; margin: 0 auto; padding: 48px 24px 80px; }
h1, h2, h3, h4 { line-height: 1.25; }
a { color: #2d5aa8; }
a.tag { text-decoration: none; }
code { font: 0.9em ui-monospace, "SF Mono", Menlo, monospace; background: #f3f3f5; padding: 1px 4px; border-radius: 4px; }
pre { background: #f6f6f7; padding: 12px 14px; border-radius: 6px; overflow-x: auto; }
pre code { background: none; padding: 0; }
pre.diagram { background: none; text-align: center; }
pre.diagram svg { max-width: 100%; height: auto; }
blockquote { margin: 0; padding-left: 14px; border-left: 3px solid #e3e3e6; color: #6b6b73; }
table { border-collapse: collapse; }
th, td { border: 1px solid #e3e3e6; padding: 4px 10px; }
table.properties { font-size: 0.9em; margin-bottom: 1.5em; }
table.properties th { text-align: left; font-weight: 500; color: #6b6b73; }
img { max-width: 100%; }
hr { border: none; border-top: 1px solid #e3e3e6; }
.katex-html { display: none; }
.math { text-align: center; margin: 12px 0; }
`;

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export function exportPage(title: string, bodyHtml: string) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<style>${CSS}</style>
</head>
<body>
<main>
${bodyHtml}
</main>
</body>
</html>
`;
}
