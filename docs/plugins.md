# Writing a Forte plugin

A plugin adds commands to Forte's command palette (⌘⇧P). It lives in your
notes folder, so it travels with your notes, but it only runs on a computer
where you've turned it on in **Settings → Plugins**.

## Files

```
<notes folder>/.forte/plugins/word-count/
  manifest.json
  plugin.js
```

`manifest.json`:

```json
{
  "name": "Word count",
  "description": "Counts the words in the open note.",
  "version": "1.0.0",
  "permissions": ["editor"]
}
```

`plugin.js`:

```js
forte.registerCommand("Count words", async () => {
  const note = await forte.currentNote();
  if (!note) return forte.notify("Open a note first.");
  const words = note.text.split(/\s+/).filter(Boolean).length;
  forte.notify(`${words} words`);
});

forte.registerCommand("Insert today's date", async () => {
  await forte.insertText(new Date().toISOString().slice(0, 10));
});
```

After adding or changing a plugin, choose **Look for plugins again** in
Settings or the command palette.

## Permissions

A plugin can only do what its manifest asks for, and you see the list before
turning it on.

| Permission    | Allows                                   |
| ------------- | ---------------------------------------- |
| *(none)*      | `registerCommand`, `notify`              |
| `editor`      | `currentNote()`, `insertText(text)`      |
| `notes:read`  | `listNotes()`, `readNote(path)`          |
| `notes:write` | `writeNote(path, text)`                  |

Paths are relative to the notes folder and must end in `.md`; hidden folders
such as `.forte` and `.trash` are off limits.

## The `forte` object

All calls except `registerCommand` return promises.

- `registerCommand(label, run)` adds "Plugin name: label" to the palette.
- `notify(message)` shows a short message.
- `currentNote()` gives `{ path, text }` for the open note, or `null`.
- `insertText(text)` inserts at the cursor (switching to editing if needed).
- `listNotes()` gives every note's path.
- `readNote(path)` gives a note's text.
- `writeNote(path, text)` replaces a note's text, creating it if needed.

## What plugins can't do

Each plugin runs in its own Web Worker, separate from the app:

- no access to the page, your settings, or your API key;
- no internet access (the app's security policy applies);
- no access to files outside the `forte` calls above.

Turning a plugin on records a fingerprint of its code. If the code changes
(for example, a synced copy is edited on another computer), the plugin turns
itself off until you approve the new version.
