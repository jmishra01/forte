import { marked } from "marked";
import DOMPurify from "dompurify";
import { attachmentsApi } from "./api";

let configured = false;

function configureMarked() {
  if (configured) return;
  configured = true;
  marked.use({
    extensions: [
      {
        name: "wikilink",
        level: "inline",
        start(src: string) {
          const idx = src.indexOf("[[");
          return idx === -1 ? undefined : idx;
        },
        tokenizer(src: string) {
          const match = /^\[\[([^[\]]+)\]\]/.exec(src);
          if (!match) return undefined;
          return {
            type: "wikilink",
            raw: match[0],
            title: match[1].trim(),
          };
        },
        renderer(token: { title: string }) {
          const escaped = token.title.replace(/"/g, "&quot;");
          return `<a href="#" class="wikilink" data-wikilink="${escaped}">${token.title}</a>`;
        },
      },
    ],
  });
}

configureMarked();

export function renderMarkdown(content: string): string {
  const html = marked.parse(content, { breaks: true, async: false }) as string;
  return DOMPurify.sanitize(html, { ADD_ATTR: ["data-wikilink"] });
}

const ATTACHMENT_SRC = /src="attachment:\/\/([^"]+)"/g;

export async function resolveAttachmentImages(html: string): Promise<string> {
  const names = new Set<string>();
  for (const match of html.matchAll(ATTACHMENT_SRC)) {
    names.add(match[1]);
  }
  if (names.size === 0) return html;

  const resolved = await Promise.all(
    Array.from(names).map(async (name) => {
      try {
        return [name, await attachmentsApi.assetUrl(name)] as const;
      } catch {
        return [name, ""] as const;
      }
    })
  );
  const map = new Map(resolved);
  return html.replace(ATTACHMENT_SRC, (_full, name) => `src="${map.get(name) ?? ""}"`);
}
