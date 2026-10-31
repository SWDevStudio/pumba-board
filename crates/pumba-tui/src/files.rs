//! Project file index for the `@` picker: a bounded std::fs walk plus fuzzy search.
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 20_000;
const MAX_DEPTH: usize = 12;
const SKIP_DIRS: [&str; 8] = ["node_modules", "target", "dist", "build", "out", "venv", "__pycache__", ".git"];

pub struct FileIndex {
    #[allow(dead_code)] // part of the public shape; the walker and tests use it
    pub root: PathBuf,
    /// Relative paths with `/` separators; directories end with `/`.
    /// Sorted by depth, then directories first, then alphabetically.
    pub paths: Vec<String>,
}

/// Names never listed: every dotfile/dotdir, plus well-known build and dependency dirs.
fn skipped(name: &str, is_dir: bool) -> bool {
    name.starts_with('.') || (is_dir && SKIP_DIRS.contains(&name))
}

fn depth_of(p: &str) -> usize {
    p.trim_end_matches('/').matches('/').count()
}

impl FileIndex {
    pub fn build(root: &Path) -> FileIndex {
        let mut paths: Vec<String> = Vec::new();
        // breadth-first, so the entry cap cuts the deepest levels first
        let mut queue: VecDeque<(PathBuf, String, usize)> = VecDeque::new();
        queue.push_back((root.to_path_buf(), String::new(), 0));
        'walk: while let Some((dir, rel, depth)) = queue.pop_front() {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            let mut entries: Vec<(String, bool)> = rd
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let ft = e.file_type().ok()?;
                    let name = e.file_name().into_string().ok()?;
                    // symlinks are skipped: no cycles, no escaping the project
                    if ft.is_symlink() || !(ft.is_dir() || ft.is_file()) || skipped(&name, ft.is_dir()) {
                        return None;
                    }
                    Some((name, ft.is_dir()))
                })
                .collect();
            entries.sort();
            for (name, is_dir) in entries {
                if paths.len() >= MAX_ENTRIES {
                    break 'walk;
                }
                if is_dir {
                    let p = format!("{rel}{name}/");
                    if depth < MAX_DEPTH {
                        queue.push_back((dir.join(&name), p.clone(), depth + 1));
                    }
                    paths.push(p);
                } else {
                    paths.push(format!("{rel}{name}"));
                }
            }
        }
        paths.sort_by_cached_key(|p| (depth_of(p), !p.ends_with('/'), p.to_lowercase(), p.clone()));
        FileIndex { root: root.to_path_buf(), paths }
    }

    /// Up to `limit` best matches for `query`, best first. Empty query: the first entries.
    pub fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let q = lower_chars(&query.replace('\\', "/"));
        if q.is_empty() {
            return self.paths.iter().take(limit).cloned().collect();
        }
        let mut hits: Vec<(i64, &String)> = self.paths.iter().filter_map(|p| score(p, &q).map(|s| (s, p))).collect();
        hits.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())).then_with(|| a.1.cmp(b.1))
        });
        hits.into_iter().take(limit).map(|(_, p)| p.clone()).collect()
    }
}

/// Per-char lowercase, so indices stay aligned with the original string.
fn lower_chars(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// Greedy subsequence match of `q` in `hay[from..]`; char indices into `hay`.
fn subseq(hay: &[char], q: &[char], from: usize) -> Option<Vec<usize>> {
    let mut out = Vec::with_capacity(q.len());
    let mut qi = 0;
    for (i, &c) in hay.iter().enumerate().skip(from) {
        if qi < q.len() && c == q[qi] {
            out.push(i);
            qi += 1;
        }
    }
    (qi == q.len()).then_some(out)
}

/// Start of the last path segment (a trailing `/` of a directory does not count).
fn name_start(hay: &[char]) -> usize {
    let end = if hay.last() == Some(&'/') { hay.len() - 1 } else { hay.len() };
    hay[..end].iter().rposition(|&c| c == '/').map_or(0, |i| i + 1)
}

/// Matched char positions (for highlighting): the filename is preferred, then the whole path.
fn find(path: &str, q: &[char]) -> Option<(Vec<usize>, bool)> {
    let hay = lower_chars(path);
    let ns = name_start(&hay);
    if ns > 0 {
        if let Some(p) = subseq(&hay, q, ns) {
            return Some((p, true));
        }
    }
    subseq(&hay, q, 0).map(|p| (p, ns == 0))
}

pub fn match_positions(path: &str, query: &str) -> Vec<usize> {
    find(path, &lower_chars(&query.replace('\\', "/"))).map(|m| m.0).unwrap_or_default()
}

fn score(path: &str, q: &[char]) -> Option<i64> {
    let (pos, in_name) = find(path, q)?;
    let hay = lower_chars(path);
    let ns = name_start(&hay);
    let mut s: i64 = if in_name { 1000 } else { 0 };
    if hay[ns..].starts_with(q) {
        s += 500;
    } else if hay.starts_with(q) {
        s += 300;
    }
    s += pos.windows(2).filter(|w| w[1] == w[0] + 1).count() as i64 * 10;
    Some(s - hay.len() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(paths: &[&str]) -> FileIndex {
        FileIndex { root: PathBuf::new(), paths: paths.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn fuzzy_ranking() {
        let i = idx(&[
            "docs/",
            "src/",
            "docs/user_interface_notes.md",
            "src/ui.rs",
            "src/model.rs",
            "src/utils/index.rs",
            "README.md",
            "tests/ui_test.rs",
        ]);
        // filename prefix beats loose subsequence; shorter first
        assert_eq!(i.search("ui", 8)[..3], ["src/ui.rs", "tests/ui_test.rs", "docs/user_interface_notes.md"]);
        // case-insensitive, filename match
        assert_eq!(i.search("READ", 3)[0], "README.md");
        assert_eq!(i.search("readme", 3), ["README.md"]);
        // subsequence across the name
        assert_eq!(i.search("mdl", 3), ["src/model.rs"]);
        // path-level subsequence
        assert!(i.search("srcmod", 3).contains(&"src/model.rs".to_string()));
        // no match
        assert!(i.search("zzz", 3).is_empty());
        // empty query: first entries in index order, honouring the limit
        assert_eq!(i.search("", 2), ["docs/", "src/"]);
        // query with a slash matches the path
        assert_eq!(i.search("src/u", 5)[0], "src/ui.rs");
        // ties are alphabetical
        let t = idx(&["b/x.rs", "a/x.rs"]);
        assert_eq!(t.search("x.rs", 5), ["a/x.rs", "b/x.rs"]);
        // limit
        assert_eq!(i.search("s", 2).len(), 2);
        // highlight positions
        assert_eq!(match_positions("src/ui.rs", "ui"), vec![4, 5]);
        assert_eq!(match_positions("src/ui.rs", "SRC"), vec![0, 1, 2]);
    }

    #[test]
    fn walker_skips_and_sorts() {
        let root = std::env::temp_dir().join(format!("pumba-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["src/deep", ".git/x", ".pumba/tasks", "node_modules/m", "target/debug", ".hidden", "docs"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in [
            "README.md", ".env.example", "src/main.rs", "src/deep/a.rs", ".git/x/y", ".pumba/board.json",
            "node_modules/m/i.js", "target/debug/app", ".hidden/f", "docs/Guide.md",
        ] {
            fs::write(root.join(f), "x").unwrap();
        }
        let ix = FileIndex::build(&root);
        assert_eq!(ix.root, root);
        assert_eq!(ix.paths, ["docs/", "src/", "README.md", "src/deep/", "docs/Guide.md", "src/main.rs", "src/deep/a.rs"]);
        let _ = fs::remove_dir_all(&root);
        // a missing root is just empty
        assert!(FileIndex::build(&root).paths.is_empty());
    }

    #[test]
    fn walker_caps_depth() {
        let root = std::env::temp_dir().join(format!("pumba-files-deep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut p = root.clone();
        for i in 0..15 {
            p = p.join(format!("d{i}"));
        }
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("f.txt"), "x").unwrap();
        let ix = FileIndex::build(&root);
        assert!(ix.paths.iter().all(|s| depth_of(s) <= MAX_DEPTH));
        assert!(!ix.paths.iter().any(|s| s.ends_with("f.txt")));
        assert!(ix.paths.iter().any(|s| s.starts_with("d0/d1/d2/")));
        let _ = fs::remove_dir_all(&root);
    }
}
