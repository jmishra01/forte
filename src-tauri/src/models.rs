use serde::{Deserialize, Serialize};

fn default_page() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Manual sibling ordering (fractional index). `None` for notes created
    /// before this existed or never manually reordered — the frontend falls
    /// back to sorting those by `created_at`.
    #[serde(default)]
    pub position: Option<f64>,
    #[serde(default)]
    pub linked_pdf_ids: Vec<String>,
    #[serde(default)]
    pub trashed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(flatten)]
    pub meta: NoteMeta,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersion {
    pub timestamp: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionContent {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfMeta {
    pub id: String,
    pub title: String,
    /// "local" or "url"
    pub source: String,
    pub source_url: Option<String>,
    pub added_at: String,
    pub file_name: String,
    #[serde(default = "default_page")]
    pub last_page: u32,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub trashed_at: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfFolder {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub id: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub available: bool,
    pub is_repo: bool,
    pub has_remote: bool,
    pub remote_url: Option<String>,
    pub dirty: bool,
    pub last_commit_at: Option<String>,
}
