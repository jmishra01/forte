<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import NoteEditor from "./lib/components/NoteEditor.svelte";
  import PdfViewer from "./lib/components/PdfViewer.svelte";
  import AddPdfDialog from "./lib/components/AddPdfDialog.svelte";
  import TrashView from "./lib/components/TrashView.svelte";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import { notesApi, pdfsApi, attachmentsApi } from "./lib/api";
  import type { Note, NoteMeta, PdfMeta } from "./lib/types";

  const IMAGE_RE = /\.(png|jpe?g|gif|webp|svg|bmp)$/i;

  let view: "notes" | "pdfs" | "trash" = "notes";
  let notes: NoteMeta[] = [];
  let pdfs: PdfMeta[] = [];
  let tags: string[] = [];
  let selectedNote: Note | null = null;
  let selectedPdf: PdfMeta | null = null;
  let showAddUrlDialog = false;
  let addUrlBusy = false;
  let addUrlError: string | null = null;
  let showPalette = false;
  let showSettings = false;

  let noteEditorRef: NoteEditor | undefined;
  let unlistenDrop: (() => void) | undefined;

  onMount(async () => {
    await refreshAll();
    unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") {
        handleDrop(event.payload.paths);
      }
    });
    window.addEventListener("keydown", handleGlobalKeydown);
  });

  onDestroy(() => {
    unlistenDrop?.();
    window.removeEventListener("keydown", handleGlobalKeydown);
  });

  async function refreshAll() {
    notes = await notesApi.list();
    pdfs = await pdfsApi.list();
    tags = await notesApi.listTags();
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    const key = e.key.toLowerCase();
    if (key === "k") {
      e.preventDefault();
      showPalette = true;
    } else if (key === "n") {
      e.preventDefault();
      createNote();
    } else if (key === "s") {
      e.preventDefault();
      noteEditorRef?.forceSave();
    }
  }

  async function selectNote(id: string) {
    selectedNote = await notesApi.get(id);
    view = "notes";
  }

  async function createNote() {
    const note = await notesApi.create("Untitled");
    notes = await notesApi.list();
    selectedNote = note;
    view = "notes";
  }

  async function createChildNote(parentId: string) {
    const note = await notesApi.create("Untitled", parentId);
    notes = await notesApi.list();
    selectedNote = note;
    view = "notes";
  }

  async function deleteNote(id: string) {
    await notesApi.remove(id);
    if (selectedNote?.id === id) selectedNote = null;
    notes = await notesApi.list();
  }

  async function renameNote(id: string, title: string) {
    try {
      await notesApi.rename(id, title);
      notes = await notesApi.list();
      if (selectedNote?.id === id) selectedNote = { ...selectedNote, title };
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to rename note");
    }
  }

  async function reparentNote(noteId: string, parentId: string | null, position: number | null) {
    try {
      await notesApi.setParent(noteId, parentId, position);
      notes = await notesApi.list();
      if (selectedNote?.id === noteId) {
        selectedNote = { ...selectedNote, parentId };
      }
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to move note");
    }
  }

  async function onNoteSaved() {
    notes = await notesApi.list();
    tags = await notesApi.listTags();
  }

  async function handleOpenWikilink(e: CustomEvent<string>) {
    const title = e.detail.trim();
    if (!title) return;
    const existing = notes.find((n) => n.title.toLowerCase() === title.toLowerCase());
    if (existing) {
      await selectNote(existing.id);
    } else {
      const created = await notesApi.create(title);
      notes = await notesApi.list();
      selectedNote = created;
    }
  }

  function selectPdf(id: string) {
    selectedPdf = pdfs.find((p) => p.id === id) ?? null;
    view = "pdfs";
  }

  async function addPdfFile() {
    const path = await pdfsApi.pickFile();
    if (!path) return;
    try {
      const meta = await pdfsApi.addFromPath(path);
      pdfs = await pdfsApi.list();
      selectedPdf = meta;
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to add PDF");
    }
  }

  async function submitAddUrl(event: CustomEvent<{ url: string; title: string }>) {
    addUrlBusy = true;
    addUrlError = null;
    try {
      const meta = await pdfsApi.addFromUrl(event.detail.url, event.detail.title || undefined);
      pdfs = await pdfsApi.list();
      selectedPdf = meta;
      showAddUrlDialog = false;
    } catch (e) {
      addUrlError = e instanceof Error ? e.message : "Failed to add PDF";
    } finally {
      addUrlBusy = false;
    }
  }

  async function deletePdf(id: string) {
    await pdfsApi.remove(id);
    if (selectedPdf?.id === id) selectedPdf = null;
    pdfs = await pdfsApi.list();
  }

  async function renamePdf(id: string, title: string) {
    try {
      await pdfsApi.rename(id, title);
      pdfs = await pdfsApi.list();
      if (selectedPdf?.id === id) selectedPdf = { ...selectedPdf, title };
    } catch (e) {
      alert(e instanceof Error ? e.message : "Failed to rename PDF");
    }
  }

  async function handleDrop(paths: string[]) {
    const pdfPaths = paths.filter((p) => p.toLowerCase().endsWith(".pdf"));
    const imagePaths = paths.filter((p) => IMAGE_RE.test(p));

    for (const path of pdfPaths) {
      try {
        const meta = await pdfsApi.addFromPath(path);
        if (view === "notes" && selectedNote) {
          const updatedMeta = await notesApi.linkPdf(selectedNote.id, meta.id);
          selectedNote = { ...selectedNote, linkedPdfIds: updatedMeta.linkedPdfIds };
        }
      } catch (e) {
        alert(e instanceof Error ? e.message : "Failed to import PDF");
      }
    }
    if (pdfPaths.length) pdfs = await pdfsApi.list();

    if (imagePaths.length) {
      if (view === "notes" && selectedNote && noteEditorRef) {
        for (const path of imagePaths) {
          try {
            const att = await attachmentsApi.import(path);
            await noteEditorRef.insertAtCursor(`\n![](attachment://${att.fileName})\n`);
          } catch (e) {
            alert(e instanceof Error ? e.message : "Failed to import image");
          }
        }
      } else {
        alert("Open a note first, then drop images to insert them there.");
      }
    }
  }

  function handlePaletteOpenNote(e: CustomEvent<string>) {
    showPalette = false;
    selectNote(e.detail);
  }

  function handlePaletteOpenPdf(e: CustomEvent<string>) {
    showPalette = false;
    selectPdf(e.detail);
  }
</script>

<main>
  <Sidebar
    {view}
    {notes}
    {pdfs}
    {tags}
    selectedNoteId={selectedNote?.id ?? null}
    selectedPdfId={selectedPdf?.id ?? null}
    on:switch-view={(e) => (view = e.detail)}
    on:select-note={(e) => selectNote(e.detail)}
    on:create-note={createNote}
    on:create-child-note={(e) => createChildNote(e.detail)}
    on:delete-note={(e) => deleteNote(e.detail)}
    on:rename-note={(e) => renameNote(e.detail.id, e.detail.title)}
    on:reparent-note={(e) => reparentNote(e.detail.noteId, e.detail.parentId, e.detail.position)}
    on:select-pdf={(e) => selectPdf(e.detail)}
    on:add-pdf-file={addPdfFile}
    on:add-pdf-url={() => {
      addUrlError = null;
      showAddUrlDialog = true;
    }}
    on:delete-pdf={(e) => deletePdf(e.detail)}
    on:rename-pdf={(e) => renamePdf(e.detail.id, e.detail.title)}
    on:open-settings={() => (showSettings = true)}
    on:open-palette={() => (showPalette = true)}
  />

  <div class="content">
    {#if view === "notes"}
      {#if selectedNote}
        {#key selectedNote.id}
          <NoteEditor
            bind:this={noteEditorRef}
            note={selectedNote}
            {pdfs}
            on:saved={onNoteSaved}
            on:open-wikilink={handleOpenWikilink}
            on:open-pdf={(e) => selectPdf(e.detail)}
            on:select-note={(e) => selectNote(e.detail)}
          />
        {/key}
      {:else}
        <div class="placeholder">Select a note, or create a new one.</div>
      {/if}
    {:else if view === "pdfs"}
      {#if selectedPdf}
        {#key selectedPdf.id}
          <PdfViewer
            pdf={selectedPdf}
            on:rename={(e) => {
              if (selectedPdf) selectedPdf = { ...selectedPdf, title: e.detail.title };
              pdfsApi.list().then((list) => (pdfs = list));
            }}
          />
        {/key}
      {:else}
        <div class="placeholder">Select a PDF, or add one from a file or URL.</div>
      {/if}
    {:else}
      <TrashView on:changed={refreshAll} />
    {/if}
  </div>
</main>

{#if showAddUrlDialog}
  <AddPdfDialog
    busy={addUrlBusy}
    errorMessage={addUrlError}
    on:submit={submitAddUrl}
    on:cancel={() => (showAddUrlDialog = false)}
  />
{/if}

{#if showPalette}
  <CommandPalette
    {notes}
    {pdfs}
    on:open-note={handlePaletteOpenNote}
    on:open-pdf={handlePaletteOpenPdf}
    on:close={() => (showPalette = false)}
  />
{/if}

{#if showSettings}
  <SettingsPanel on:close={() => (showSettings = false)} on:data-dir-changed={refreshAll} />
{/if}

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
  }
  .content {
    flex: 1;
    display: flex;
    min-width: 0;
  }
  .placeholder {
    margin: auto;
    color: var(--text-muted);
    font-size: 14px;
  }
</style>
