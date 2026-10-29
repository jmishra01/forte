use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// One matching line of a note.
#[derive(Serialize, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    pub text: String,
}

/// What one note links to and how it is tagged.
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct NoteInfo {
    pub path: String,
    pub links: Vec<Link>,
    /// Lowercase, without `#`, each once, in order of appearance.
    pub tags: Vec<String>,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Link {
    /// For `[[wiki]]` links, what is between `[[` and `|` or `]]`, trimmed. For
    /// markdown links, the `.md` path as written (relative to the note, maybe URL-encoded).
    pub target: String,
    pub line: usize,
    pub text: String,
    /// "wiki" or "md".
    pub kind: &'static str,
}

const HITS_PER_NOTE: usize = 3;
const SNIPPET_CHARS: usize = 160;

/// Every `.md` file under `root`, skipping hidden files and folders such as `.trash`.
pub fn note_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

/// Lines containing `query` (case-insensitive), at most a few per note and
/// `limit` overall.
pub fn search(root: &Path, query: &str, limit: usize) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let mut hits = Vec::new();
    for file in note_files(root) {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        let path = super::vault::to_rel(root, &file);
        for (i, line) in text.lines().enumerate().filter(|(_, l)| l.to_lowercase().contains(&q)).take(HITS_PER_NOTE) {
            hits.push(Hit { path: path.clone(), line: i + 1, text: snippet(line, &q) });
            if hits.len() >= limit {
                return hits;
            }
        }
    }
    hits
}

/// `line` trimmed to a window around the first match of `q` (already lowercase).
fn snippet(line: &str, q: &str) -> String {
    let chars: Vec<char> = line.trim().chars().collect();
    if chars.len() <= SNIPPET_CHARS {
        return chars.into_iter().collect();
    }
    let lower: String = chars.iter().collect::<String>().to_lowercase();
    let at = lower.find(q).map(|b| lower[..b].chars().count()).unwrap_or(0);
    let start = at.saturating_sub(SNIPPET_CHARS / 3).min(chars.len() - SNIPPET_CHARS);
    let end = start + SNIPPET_CHARS;
    let mut s: String = chars[start..end].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    if end < chars.len() {
        s.push('…');
    }
    s
}

/// What a note contains that the app asks about on every change.
#[derive(Clone)]
pub struct Facts {
    pub info: NoteInfo,
    /// Has `<<<<<<<` … `>>>>>>>` blocks from a Git merge.
    pub markers: bool,
}

/// Facts per file, reused while the file's modification time and size are unchanged,
/// so a refresh after each save only re-reads the notes that changed.
static CACHE: OnceLock<Mutex<HashMap<PathBuf, (SystemTime, u64, Facts)>>> = OnceLock::new();

fn cache() -> std::sync::MutexGuard<'static, HashMap<PathBuf, (SystemTime, u64, Facts)>> {
    CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Forgets `file`, for when it was just written (a fast second write can keep the same time and size).
pub fn forget(file: &Path) {
    cache().remove(file);
}

/// Facts for every note under `root`, read from disk only where a note changed.
pub fn facts(root: &Path) -> Vec<Facts> {
    let files = note_files(root);
    let mut out = Vec::with_capacity(files.len());
    let mut cache = cache();
    for file in &files {
        let Ok(meta) = fs::metadata(file) else { continue };
        let (modified, len) = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
        let path = super::vault::to_rel(root, file);
        if let Some((m, l, f)) = cache.get(file) {
            // The same file reached by its path; the vault-relative path is rebuilt in case the vault moved.
            if *m == modified && *l == len && f.info.path == path {
                out.push(f.clone());
                continue;
            }
        }
        let Ok(text) = fs::read_to_string(file) else { continue };
        let f = Facts { info: note_info(path, &text), markers: crate::conflicts::has_markers(&text) };
        cache.insert(file.clone(), (modified, len, f.clone()));
        out.push(f);
    }
    // Drop notes under this vault that are gone.
    let present: std::collections::HashSet<&PathBuf> = files.iter().collect();
    cache.retain(|p, _| !p.starts_with(root) || present.contains(p));
    out
}

/// Links and tags of every note that has any.
pub fn index(root: &Path) -> Vec<NoteInfo> {
    facts(root)
        .into_iter()
        .map(|f| f.info)
        .filter(|info| !info.links.is_empty() || !info.tags.is_empty())
        .collect()
}

fn note_info(path: String, text: &str) -> NoteInfo {
    let mut links = Vec::new();
    let mut tags = Vec::new();
    let mut add_tag = |t: String| {
        if !tags.contains(&t) {
            tags.push(t);
        }
    };
    let (front, body_start) = front_matter(text);
    front_matter_tags(front).into_iter().for_each(&mut add_tag);
    let mut in_fence = false;
    for (i, line) in text.lines().enumerate() {
        let link = |kind| move |target| Link { target, line: i + 1, text: snippet(line, ""), kind };
        links.extend(wikilinks(line).into_iter().map(link("wiki")));
        links.extend(markdown_links(line).into_iter().map(link("md")));
        if i < body_start {
            continue;
        }
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            inline_tags(line).into_iter().for_each(&mut add_tag);
        }
    }
    NoteInfo { path, links, tags }
}

/// The YAML front matter between leading `---` lines (without them), and the
/// index of the first line after it. Empty and 0 when there is none.
pub fn front_matter(text: &str) -> (&str, usize) {
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return ("", 0);
    }
    let start = text.find('\n').map_or(text.len(), |i| i + 1);
    let mut offset = start;
    for (n, line) in lines.enumerate() {
        if matches!(line.trim_end(), "---" | "...") {
            return (&text[start..offset], n + 2);
        }
        offset += line.len();
    }
    ("", 0)
}

/// `tags:` (or `tag:`) as `[a, b]`, `a, b`, or an indented `- a` list.
fn front_matter_tags(front: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_list = false;
    for line in front.lines() {
        if in_list {
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                out.extend(clean_tag(item));
                continue;
            }
            in_list = false;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        if !matches!(key.trim(), "tags" | "tag") {
            continue;
        }
        let value = value.trim().trim_start_matches('[').trim_end_matches(']');
        if value.is_empty() {
            in_list = true;
        }
        out.extend(value.split(',').filter_map(clean_tag));
    }
    out
}

fn clean_tag(raw: &str) -> Option<String> {
    let t = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim_start_matches('#').trim_end_matches('/');
    let first = t.chars().next()?;
    (first.is_alphabetic() || first == '_')
        .then(|| t.chars().all(|c| c.is_alphanumeric() || "_-/".contains(c)))
        .filter(|ok| *ok)
        .map(|_| t.to_lowercase())
}

/// `#tags` in a line of body text: `#` at the start or after whitespace, then a
/// letter or `_`, then letters, digits, `_`, `-` or `/`. Inline code is skipped.
fn inline_tags(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut prev = ' ';
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            in_code = !in_code;
        } else if c == '#' && !in_code && prev.is_whitespace() {
            let end = (i + 1..chars.len())
                .find(|&j| !(chars[j].is_alphanumeric() || "_-/".contains(chars[j])))
                .unwrap_or(chars.len());
            let tag: String = chars[i + 1..end].iter().collect();
            if let Some(t) = clean_tag(&tag) {
                out.push(t);
            }
            i = end;
            prev = '#';
            continue;
        }
        prev = c;
        i += 1;
    }
    out
}

/// Targets of `[[target]]` and `[[target|label]]` in `line`, matching the
/// preview's wikilink syntax.
fn wikilinks(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find("[[") {
        rest = &rest[i + 2..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        if !inner.contains('[') && !inner.contains(']') {
            let target = inner.split('|').next().unwrap_or("").trim();
            if !target.is_empty() {
                out.push(target.to_string());
            }
            rest = &rest[end + 2..];
        }
    }
    out
}

/// Targets of `[text](note.md)` and `[text](<my note.md#part>)` links to other
/// notes: relative paths ending in `.md`, not URLs. A `#part` is dropped.
fn markdown_links(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find("](") {
        rest = &rest[i + 2..];
        let (raw, after) = match rest.strip_prefix('<') {
            Some(inner) => match inner.find('>') {
                Some(end) => (&inner[..end], &inner[end + 1..]),
                None => break,
            },
            None => {
                let end = rest.find(|c: char| c == ')' || c.is_whitespace()).unwrap_or(rest.len());
                (&rest[..end], &rest[end..])
            }
        };
        rest = after;
        let path = raw.split('#').next().unwrap_or("");
        let is_url = path.split('/').next().is_some_and(|first| first.contains(':'));
        if path.to_lowercase().ends_with(".md") && !is_url && !path.starts_with('/') {
            out.push(path.to_string());
        }
    }
    out
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
    fn search_is_case_insensitive_and_skips_hidden_and_non_notes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", "intro\nThe Quick fox\nlast");
        write(root, "sub/b.md", "quick again");
        write(root, ".trash/c.md", "quick but deleted");
        write(root, "d.txt", "quick but not a note");
        let hits = search(root, "  QUICK ", 50);
        assert_eq!(
            hits,
            vec![
                Hit { path: "a.md".into(), line: 2, text: "The Quick fox".into() },
                Hit { path: "sub/b.md".into(), line: 1, text: "quick again".into() },
            ]
        );
        assert!(search(root, "   ", 50).is_empty());
    }

    #[test]
    fn search_respects_limits() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.md", &"x\n".repeat(10));
        write(dir.path(), "b.md", "x");
        assert_eq!(search(dir.path(), "x", 50).len(), HITS_PER_NOTE + 1);
        assert_eq!(search(dir.path(), "x", 2).len(), 2);
    }

    #[test]
    fn snippet_windows_long_lines_around_the_match() {
        let line = format!("{}needle{}", "a".repeat(300), "b".repeat(300));
        let s = snippet(&line, "needle");
        assert!(s.starts_with('…') && s.ends_with('…'));
        assert!(s.contains("needle"));
        assert_eq!(s.chars().count(), SNIPPET_CHARS + 2);
        // Multi-byte characters must not split.
        let s = snippet(&"é".repeat(400), "é");
        assert_eq!(s.chars().count(), SNIPPET_CHARS + 1);
    }

    #[test]
    fn wikilinks_parse_like_the_preview() {
        assert_eq!(wikilinks("see [[A]] and [[ dir/B | label ]]"), ["A", "dir/B"]);
        assert_eq!(wikilinks("[[]] [[x[y]] [[open"), Vec::<String>::new());
    }

    #[test]
    fn index_lists_notes_with_links_or_tags() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.md", "first\nlinks to [[B]]");
        write(dir.path(), "b.md", "nothing here");
        write(dir.path(), "c.md", "just #tagged");
        let all = index(dir.path());
        assert_eq!(all.iter().map(|n| n.path.as_str()).collect::<Vec<_>>(), ["a.md", "c.md"]);
        assert_eq!(all[0].links, vec![Link { target: "B".into(), line: 2, text: "links to [[B]]".into(), kind: "wiki" }]);
        assert_eq!(all[1].tags, ["tagged"]);
    }

    #[test]
    fn index_cache_notices_edits_and_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", "#one");
        write(root, "b.md", "#two");
        assert_eq!(index(root).len(), 2);
        // Rewritten with the same length: forget() makes sure it is read again.
        write(root, "a.md", "#uno");
        forget(&root.join("a.md"));
        assert_eq!(index(root).iter().find(|n| n.path == "a.md").unwrap().tags, ["uno"]);
        fs::remove_file(root.join("b.md")).unwrap();
        assert_eq!(index(root).iter().map(|n| n.path.as_str()).collect::<Vec<_>>(), ["a.md"]);
        assert!(!cache().contains_key(&root.join("b.md")));
    }

    #[test]
    fn markdown_links_to_notes_only() {
        assert_eq!(
            markdown_links("see [a](Plan.md), [b](../work/Ideas.md#top) and [c](<My Note.md>)"),
            ["Plan.md", "../work/Ideas.md", "My Note.md"]
        );
        assert_eq!(markdown_links("[x](https://e.com/a.md) [y](/abs.md) [z](pic.png) [w](#here)"), Vec::<String>::new());
        assert_eq!(markdown_links("[enc](My%20Note.md \"title\")"), ["My%20Note.md"]);
        let info = note_info("n.md".into(), "[[Wiki]] and [md](Other.md)");
        assert_eq!(info.links.iter().map(|l| (l.kind, l.target.as_str())).collect::<Vec<_>>(), [("wiki", "Wiki"), ("md", "Other.md")]);
    }

    #[test]
    fn inline_tags_skip_headings_code_and_fragments() {
        assert_eq!(inline_tags("#Work and #idea/sub-1, not#this"), ["work", "idea/sub-1"]);
        assert_eq!(inline_tags("# Heading ## also #123 #_ok"), ["_ok"]);
        assert_eq!(inline_tags("`#code` then #real"), ["real"]);
        assert_eq!(inline_tags("url http://x.com/#frag #été"), ["été"]);
    }

    #[test]
    fn front_matter_tags_in_all_styles() {
        let info = note_info(
            "n.md".into(),
            "---\ntitle: x\ntags: [Alpha, \"beta\"]\ntag: gamma\n---\n# Body #delta\n```\n#not-in-code\n```\n#alpha again",
        );
        assert_eq!(info.tags, ["alpha", "beta", "gamma", "delta"]);
        let list = note_info("n.md".into(), "---\ntags:\n  - one\n  - two\nother: 1\n---\n");
        assert_eq!(list.tags, ["one", "two"]);
        // Not front matter unless it starts the file and is closed.
        assert_eq!(note_info("n.md".into(), "text\n---\ntags: x\n---\n").tags, Vec::<String>::new());
        assert_eq!(front_matter("---\ntags: x\nnever closed"), ("", 0));
    }
}

/// Timing on a 5,000-note vault: `cargo test --release bench_ -- --ignored --nocapture`.
#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn bench_index_and_search_5000_notes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..5000 {
            let sub = root.join(format!("folder{}", i % 50));
            fs::create_dir_all(&sub).unwrap();
            let body = format!(
                "---\ntags: [t{}, project]\n---\n# Note {i}\n\nSee [[Note {}]] and [x](../folder{}/Note {}.md). #topic{}\n{}\n",
                i % 20,
                (i + 1) % 5000,
                (i + 7) % 50,
                (i + 7) % 5000,
                i % 30,
                "Some ordinary sentence about notes and ideas. ".repeat(40)
            );
            fs::write(sub.join(format!("Note {i}.md")), body).unwrap();
        }
        let time = |label: &str, f: &dyn Fn() -> usize| {
            let start = Instant::now();
            let n = f();
            println!("{label}: {:>6.1} ms ({n} results)", start.elapsed().as_secs_f64() * 1000.0);
        };
        for round in ["first", "again"] {
            time(&format!("index  ({round})"), &|| index(root).len());
            time(&format!("search ({round})"), &|| search(root, "ordinary sentence", 200).len());
        }
        fs::write(root.join("folder3/Note 3.md"), "changed [[Note 9]]").unwrap();
        time("index  (one note changed)", &|| index(root).len());
        time("conflicts", &|| crate::conflicts::find(root).len());
    }
}
