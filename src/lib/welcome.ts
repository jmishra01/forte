// The note offered to new vaults. It shows each feature by using it.
// One line per paragraph: Forte shows single line breaks as breaks.
export const WELCOME_NAME = "Welcome to Forte";

export const WELCOME_TEXT = `---
tags: [forte, start-here]
---
# Welcome to Forte

Forte keeps your notes as plain Markdown files in a folder you choose, so they're yours: any editor can open them, and any sync tool can copy them.

## Writing

- Click anywhere in a note to edit it; press **Esc** or click away to see it formatted again.
- Type \`[[\` or \`/\` to link another note, like [[Getting started]] — links to notes that don't exist yet create them when you click.
- Tag notes with #ideas or in the front matter at the top of this note.
- Paste or drop an image to add it to the note.

## Finding things

- **⌘K** searches note names and text. **⌘⇧P** lists every command.
- The panel under a note shows which notes link to it.
- **Today** in the sidebar opens a note for the day.

## More than text

Math: $e^{i\\pi} + 1 = 0$

\`\`\`mermaid
graph LR
  Idea --> Note --> Link --> Idea
\`\`\`

\`\`\`js
// Code blocks are coloured by language.
const greet = (name) => \`Hello, \${name}!\`;
\`\`\`

Press **⌘?** for all keyboard shortcuts. Delete this note whenever you like; it goes to the trash first.
`;
