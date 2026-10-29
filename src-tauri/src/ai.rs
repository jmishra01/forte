//! Optional AI help, using the person's own Anthropic API key.
//!
//! The key lives in the OS keychain and is only read here, in Rust: it never
//! goes back to the webview, and requests go out from Rust, so the page's
//! Content Security Policy stays closed to the network. Nothing is sent unless
//! the person runs an action.

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::ipc::Channel;
use tauri::State;

use crate::search::note_files;
use crate::vault::{resolve, root, to_rel, AppState};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-opus-5-5";
const KEYCHAIN_SERVICE: &str = "com.jitendra.forte";
const KEYCHAIN_USER: &str = "anthropic-api-key";
/// How much note text "ask" sends at most, so one question can't send the whole vault.
const ASK_BUDGET_CHARS: usize = 120_000;
const ASK_MAX_NOTES: usize = 8;

/// Bumped to stop the answer being streamed.
static RUN: AtomicU64 = AtomicU64::new(0);

/// Sent to the page while an answer streams in.
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEvent {
    /// Which notes were sent, so the page can show what left the machine.
    Sources { paths: Vec<String> },
    Text { text: String },
    Done,
    Error { message: String },
}

fn keychain() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| e.to_string())
}

fn api_key() -> Result<String, String> {
    keychain()?
        .get_password()
        .map_err(|_| "Add your Anthropic API key in Settings to use AI.".to_string())
}

#[tauri::command]
pub fn ai_has_key() -> bool {
    api_key().is_ok()
}

#[tauri::command]
pub fn ai_set_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if !key.starts_with("sk-ant-") {
        return Err("That doesn't look like an Anthropic API key (they start with sk-ant-).".into());
    }
    keychain()?.set_password(key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_clear_key() -> Result<(), String> {
    match keychain()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn ai_stop() {
    RUN.fetch_add(1, Ordering::SeqCst);
}

/// Runs `task` ("summarize", "links" or "ask") and streams the answer to `events`.
#[tauri::command]
pub async fn ai_run(
    state: State<'_, AppState>,
    task: String,
    path: Option<String>,
    question: Option<String>,
    events: Channel<AiEvent>,
) -> Result<(), String> {
    let root = root(&state);
    let run = RUN.fetch_add(1, Ordering::SeqCst) + 1;
    let result = async {
        let key = api_key()?;
        let (system, user, sources) = prompt(&root, &task, path.as_deref(), question.as_deref())?;
        let _ = events.send(AiEvent::Sources { paths: sources });
        stream(&key, &system, &user, run, &events).await
    }
    .await;
    let _ = events.send(match result {
        Ok(()) => AiEvent::Done,
        Err(message) => AiEvent::Error { message },
    });
    Ok(())
}

/// The system prompt, the user message, and which notes it includes.
fn prompt(root: &Path, task: &str, path: Option<&str>, question: Option<&str>) -> Result<(String, String, Vec<String>), String> {
    let note = |p: &str| -> Result<String, String> { fs::read_to_string(resolve(root, p)?).map_err(|e| e.to_string()) };
    let title = |p: &str| p.trim_end_matches(".md").to_string();
    match task {
        "summarize" => {
            let p = path.ok_or("no note open")?;
            let system = "You help someone with their personal markdown notes. Summarize the note you are given \
                in a few short bullet points that capture its main points and any open questions or to-dos. \
                Answer in the note's language. Reply with the bullet points only.";
            let user = format!("<note title=\"{}\">\n{}\n</note>", title(p), note(p)?);
            Ok((system.into(), user, vec![p.into()]))
        }
        "links" => {
            let p = path.ok_or("no note open")?;
            let others: Vec<String> = note_files(root)
                .iter()
                .map(|f| to_rel(root, f))
                .filter(|o| o != p)
                .map(|o| title(&o))
                .collect();
            let system = "You help someone connect their personal markdown notes. Given one note and the titles of \
                their other notes, suggest up to five other notes it would be worth linking to, most relevant first. \
                Only suggest titles from the list, exactly as written. For each, write one line: \
                `- [[Title]]: why it is related` (a short reason). If none are clearly related, say so in one sentence.";
            let user = format!(
                "<note title=\"{}\">\n{}\n</note>\n\n<other_note_titles>\n{}\n</other_note_titles>",
                title(p),
                note(p)?,
                others.join("\n")
            );
            Ok((system.into(), user, vec![p.into()]))
        }
        "ask" => {
            let q = question.map(str::trim).filter(|q| !q.is_empty()).ok_or("ask a question first")?;
            let picked = relevant_notes(root, q);
            if picked.is_empty() {
                return Err("No notes mention the words in that question.".into());
            }
            let mut budget = ASK_BUDGET_CHARS;
            let mut docs = String::new();
            let mut sources = Vec::new();
            for p in picked {
                let Ok(text) = note(&p) else { continue };
                if budget == 0 {
                    break;
                }
                let take: String = text.chars().take(budget).collect();
                let cut = take.len() < text.len();
                budget = budget.saturating_sub(take.chars().count());
                docs.push_str(&format!(
                    "<note title=\"{}\"{}>\n{}\n</note>\n",
                    title(&p),
                    if cut { " truncated=\"true\"" } else { "" },
                    take
                ));
                sources.push(p);
            }
            let system = "You answer questions using someone's personal markdown notes. Use only the notes provided. \
                Cite the notes you rely on inline as [[Title]]. If the notes don't contain the answer, say so plainly \
                rather than guessing. Notes marked truncated were cut off for length. Be concise.";
            let user = format!("<notes>\n{docs}</notes>\n\nQuestion: {q}");
            Ok((system.into(), user, sources))
        }
        _ => Err(format!("unknown AI action: {task}")),
    }
}

/// Notes that best match the question's words (title matches count more), best first.
fn relevant_notes(root: &Path, question: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "are", "was", "what", "when", "where", "which", "who", "why", "how", "did", "does",
        "with", "about", "from", "that", "this", "have", "has", "had", "you", "your", "my", "our", "can", "could",
        "should", "would", "into", "there", "their", "them", "any", "all", "not", "but", "its",
    ];
    let words: Vec<String> = question
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(&w.as_str()))
        .collect();
    if words.is_empty() {
        return vec![];
    }
    let mut scored: Vec<(usize, String)> = note_files(root)
        .into_iter()
        .filter_map(|f| {
            let text = fs::read_to_string(&f).ok()?.to_lowercase();
            let rel = to_rel(root, &f);
            let name = rel.to_lowercase();
            let score: usize = words.iter().map(|w| text.matches(w.as_str()).count() + 5 * name.matches(w.as_str()).count()).sum();
            (score > 0).then_some((score, rel))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().take(ASK_MAX_NOTES).map(|(_, p)| p).collect()
}

fn request_body(system: &str, user: &str) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": 64000,
        "stream": true,
        // Thinking is adaptive by default on this model; effort sets how hard it works.
        "output_config": { "effort": "medium" },
        // If a safety check declines, let the API retry on its recommended fallback model.
        "fallbacks": "default",
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    })
}

async fn stream(key: &str, system: &str, user: &str, run: u64, events: &Channel<AiEvent>) -> Result<(), String> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let res = reqwest::Client::new()
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", "2026-10-21")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .header("content-type", "application/json")
        .json(&request_body(system, user))
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Anthropic: {e}"))?;
    let status = res.status();
    if !status.is_success() {
        let body: Value = res.json().await.unwrap_or_default();
        let detail = body["error"]["message"].as_str().unwrap_or("").to_string();
        return Err(match status.as_u16() {
            401 => "Anthropic rejected the API key. Check it in Settings.".into(),
            429 => "Anthropic's rate limit was reached. Try again in a minute.".into(),
            529 | 503 => "Anthropic is overloaded right now. Try again shortly.".into(),
            _ => format!("Anthropic returned an error ({status}): {detail}"),
        });
    }

    let mut body = res.bytes_stream();
    // Raw bytes: a chunk can end mid-character, so only whole events are decoded.
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = body.next().await {
        if RUN.load(Ordering::SeqCst) != run {
            return Ok(()); // stopped, or replaced by a newer run
        }
        buf.extend_from_slice(&chunk.map_err(|e| e.to_string())?);
        // Server-sent events are separated by a blank line; keep any partial event for the next chunk.
        while let Some(end) = buf.windows(2).position(|w| w == b"\n\n") {
            let event: Vec<u8> = buf.drain(..end + 2).collect();
            for line in String::from_utf8_lossy(&event).lines() {
                let Some(data) = line.strip_prefix("data: ") else { continue };
                let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                if let Some(done) = handle(&v, events)? {
                    return done;
                }
            }
        }
    }
    Ok(())
}

/// Forwards one stream event. Returns Some when the answer is over.
fn handle(v: &Value, events: &Channel<AiEvent>) -> Result<Option<Result<(), String>>, String> {
    match v["type"].as_str() {
        Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
            let _ = events.send(AiEvent::Text { text: v["delta"]["text"].as_str().unwrap_or("").to_string() });
            Ok(None)
        }
        Some("message_delta") => match v["delta"]["stop_reason"].as_str() {
            Some("refusal") => Ok(Some(Err("The model declined this request.".into()))),
            Some("max_tokens") => {
                let _ = events.send(AiEvent::Text { text: "\n\n*(The answer was cut off for length.)*".into() });
                Ok(None)
            }
            _ => Ok(None),
        },
        Some("message_stop") => Ok(Some(Ok(()))),
        Some("error") => Ok(Some(Err(format!(
            "Anthropic error: {}",
            v["error"]["message"].as_str().unwrap_or("unknown error")
        )))),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    #[test]
    fn request_uses_current_model_and_fallbacks() {
        let body = request_body("sys", "hi");
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["stream"], true);
        assert!(body.get("thinking").is_none() && body.get("temperature").is_none());
        assert_eq!(body["messages"][0]["content"], "hi");
    }

    #[test]
    fn ask_picks_matching_notes_best_first() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "Garden.md", "tomatoes and basil");
        write(root, "Tomatoes.md", "notes on tomatoes: water tomatoes daily");
        write(root, "Taxes.md", "file by april");
        write(root, ".trash/Tomato old.md", "tomatoes");
        assert_eq!(relevant_notes(root, "How often should I water the tomatoes?"), ["Tomatoes.md", "Garden.md"]);
        assert!(relevant_notes(root, "what is it?").is_empty(), "only stop words");
    }

    #[test]
    fn prompts_include_the_right_notes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a/Plan.md", "ship forte");
        write(root, "Ideas.md", "more ideas");
        let (_, user, sources) = prompt(root, "summarize", Some("a/Plan.md"), None).unwrap();
        assert!(user.contains("ship forte") && sources == ["a/Plan.md"]);
        let (_, user, _) = prompt(root, "links", Some("a/Plan.md"), None).unwrap();
        assert!(user.contains("Ideas") && !user.contains("<other_note_titles>\na/Plan"));
        let (_, user, sources) = prompt(root, "ask", None, Some("what ideas")).unwrap();
        assert!(user.contains("more ideas") && sources == ["Ideas.md"]);
        assert!(prompt(root, "ask", None, Some("  ")).is_err());
        assert!(prompt(root, "summarize", Some("../etc/passwd"), None).is_err());
        assert!(prompt(root, "other", None, None).is_err());
    }

    #[test]
    fn ask_stays_within_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "Big.md", &"word ".repeat(40_000)); // 200k chars
        write(root, "Small.md", "word");
        let (_, user, sources) = prompt(root, "ask", None, Some("word")).unwrap();
        assert!(user.len() < ASK_BUDGET_CHARS + 1_000);
        assert!(user.contains("truncated=\"true\""));
        assert_eq!(sources, ["Big.md"], "nothing left for a second note");
    }
}
