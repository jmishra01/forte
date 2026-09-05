import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AttachmentMeta,
  GitStatus,
  Note,
  NoteMeta,
  NoteVersion,
  NoteVersionContent,
  PdfAnnotation,
  PdfMeta,
} from "./types";

export const notesApi = {
  list: () => invoke<NoteMeta[]>("list_notes"),
  listTrashed: () => invoke<NoteMeta[]>("list_trashed_notes"),
  listTags: () => invoke<string[]>("list_tags"),
  get: (id: string) => invoke<Note>("get_note", { id }),
  create: (title: string, parentId?: string | null) =>
    invoke<Note>("create_note", { title, parentId: parentId ?? null }),
  update: (id: string, title: string, content: string) =>
    invoke<NoteMeta>("update_note", { id, title, content }),
  rename: (id: string, title: string) => invoke<NoteMeta>("rename_note", { id, title }),
  setTags: (id: string, tags: string[]) => invoke<NoteMeta>("set_note_tags", { id, tags }),
  setParent: (id: string, parentId: string | null, position?: number | null) =>
    invoke<NoteMeta>("set_note_parent", { id, parentId, position: position ?? null }),
  linkPdf: (noteId: string, pdfId: string) =>
    invoke<NoteMeta>("link_pdf_to_note", { noteId, pdfId }),
  unlinkPdf: (noteId: string, pdfId: string) =>
    invoke<NoteMeta>("unlink_pdf_from_note", { noteId, pdfId }),
  remove: (id: string) => invoke<void>("delete_note", { id }),
  restore: (id: string) => invoke<NoteMeta>("restore_note", { id }),
  permanentlyDelete: (id: string) => invoke<void>("permanently_delete_note", { id }),
  emptyTrash: () => invoke<void>("empty_notes_trash"),
  backlinks: (title: string) => invoke<NoteMeta[]>("get_backlinks", { title }),
  history: (id: string) => invoke<NoteVersion[]>("list_note_history", { id }),
  historyContent: (id: string, timestamp: string) =>
    invoke<NoteVersionContent>("get_note_history_content", { id, timestamp }),
  restoreVersion: (id: string, timestamp: string) =>
    invoke<Note>("restore_note_version", { id, timestamp }),
};

export const pdfsApi = {
  list: () => invoke<PdfMeta[]>("list_pdfs"),
  listTrashed: () => invoke<PdfMeta[]>("list_trashed_pdfs"),
  addFromPath: (path: string, title?: string) =>
    invoke<PdfMeta>("add_pdf_from_path", { path, title: title ?? null }),
  addFromUrl: (url: string, title?: string) =>
    invoke<PdfMeta>("add_pdf_from_url", { url, title: title ?? null }),
  rename: (id: string, title: string) => invoke<PdfMeta>("rename_pdf", { id, title }),
  remove: (id: string) => invoke<void>("delete_pdf", { id }),
  restore: (id: string) => invoke<PdfMeta>("restore_pdf", { id }),
  permanentlyDelete: (id: string) => invoke<void>("permanently_delete_pdf", { id }),
  emptyTrash: () => invoke<void>("empty_pdfs_trash"),
  updateProgress: (id: string, page: number) => invoke<void>("update_pdf_progress", { id, page }),
  listAnnotations: (pdfId: string) => invoke<PdfAnnotation[]>("list_annotations", { pdfId }),
  addAnnotation: (
    pdfId: string,
    page: number,
    x: number,
    y: number,
    w: number,
    h: number,
    color: string,
    note?: string
  ) =>
    invoke<PdfAnnotation>("add_annotation", {
      pdfId,
      page,
      x,
      y,
      w,
      h,
      color,
      note: note ?? null,
    }),
  deleteAnnotation: (pdfId: string, annotationId: string) =>
    invoke<void>("delete_annotation", { pdfId, annotationId }),
  async assetUrl(id: string): Promise<string> {
    const path = await invoke<string>("get_pdf_path", { id });
    return convertFileSrc(path);
  },
  async pickFile(): Promise<string | null> {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!selected) return null;
    return Array.isArray(selected) ? selected[0] : selected;
  },
};

export const attachmentsApi = {
  import: (path: string) => invoke<AttachmentMeta>("import_attachment", { path }),
  async assetUrl(fileName: string): Promise<string> {
    const path = await invoke<string>("get_attachment_path", { fileName });
    return convertFileSrc(path);
  },
};

export const exportApi = {
  writeFileBytes: (path: string, data: Uint8Array) =>
    invoke<void>("write_file_bytes", { path, data: Array.from(data) }),
  exportAllNotes: (path: string) => invoke<number>("export_all_notes", { path }),
  async pickZipSavePath(defaultName: string): Promise<string | null> {
    return save({ defaultPath: defaultName, filters: [{ name: "Zip", extensions: ["zip"] }] });
  },
  async pickPdfSavePath(defaultName: string): Promise<string | null> {
    return save({ defaultPath: defaultName, filters: [{ name: "PDF", extensions: ["pdf"] }] });
  },
};

export const syncApi = {
  getDataDir: () => invoke<string>("get_data_dir"),
  chooseDataDir: (path: string, migrate: boolean) =>
    invoke<string>("choose_data_dir", { path, migrate }),
  async pickDataDir(): Promise<string | null> {
    const selected = await open({ multiple: false, directory: true });
    if (!selected) return null;
    return Array.isArray(selected) ? selected[0] : selected;
  },
  gitStatus: () => invoke<GitStatus>("git_status"),
  gitInit: () => invoke<string>("git_init"),
  gitSetRemote: (url: string) => invoke<string>("git_set_remote", { url }),
  gitSync: () => invoke<string>("git_sync"),
};
