use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// One matching line of a note.
#[derive(Serialize, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    pub text: String,
}

/// The `[[links]]` written in one note.
#[derive(Serialize, Debug, PartialEq)]
pub struct NoteLinks {
    pub path: String,
    pub links: Vec<Link>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Link {
    /// What is between `[[` and `|` or `]]`, trimmed.
    pub target: String,
    pub line: usize,
    pub text: String,
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

/// The `[[links]]` of every note that has any.
pub fn links(root: &Path) -> Vec<NoteLinks> {
    note_files(root)
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(&file).ok()?;
            let links: Vec<Link> = text
                .lines()
                .enumerate()
                .flat_map(|(i, line)| {
                    wikilinks(line).into_iter().map(move |target| Link {
                        target,
                        line: i + 1,
                        text: snippet(line, ""),
                    })
                })
                .collect();
            (!links.is_empty()).then(|| NoteLinks { path: super::vault::to_rel(root, &file), links })
        })
        .collect()
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
    fn links_lists_notes_with_links() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.md", "first\nlinks to [[B]]");
        write(dir.path(), "b.md", "no links");
        let all = links(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].path, "a.md");
        assert_eq!(all[0].links, vec![Link { target: "B".into(), line: 2, text: "links to [[B]]".into() }]);
    }
}
