export interface NoteMeta {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  tags: string[];
  parentId: string | null;
  position: number | null;
  linkedPdfIds: string[];
  trashedAt: string | null;
}

export interface Note extends NoteMeta {
  content: string;
}

export interface NoteVersion {
  timestamp: string;
  title: string;
}

export interface NoteVersionContent {
  title: string;
  content: string;
}

export interface PdfMeta {
  id: string;
  title: string;
  source: "local" | "url";
  sourceUrl: string | null;
  addedAt: string;
  fileName: string;
  lastPage: number;
  completed: boolean;
  trashedAt: string | null;
  folderId: string | null;
  tags: string[];
}

export interface PdfFolder {
  id: string;
  name: string;
  createdAt: string;
}

export interface AttachmentMeta {
  id: string;
  fileName: string;
}

export interface GitStatus {
  available: boolean;
  isRepo: boolean;
  hasRemote: boolean;
  remoteUrl: string | null;
  dirty: boolean;
  lastCommitAt: string | null;
}
