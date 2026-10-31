//! Subagent names for the column form's `agent` field: a bounded scan of the agent folders of
//! every supported tool (project and user level). The value suggested is the file stem, which is
//! what `board.json` stores.
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Pack agents (`pumba-*`) are listed first.
const PACK_PREFIX: &str = "pumba-";
/// Longest description kept for the popup row, in chars (the rest becomes «…»).
const DESC_MAX: usize = 60;
/// Frontmatter is near the top: this much of a file is enough.
const READ_MAX: u64 = 16 * 1024;
const MAX_PER_DIR: usize = 500;

/// Agent folders under the project root: (folder, tool tag).
const PROJECT_DIRS: [(&str, &str); 5] = [
    (".claude/agents", "claude"),
    (".codex/agents", "codex"),
    (".cursor/agents", "cursor"),
    (".github/agents", "copilot"),
    (".opencode/agents", "opencode"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRef {
    /// File stem: the value stored in `board.json`.
    pub name: String,
    /// First ~60 chars of the frontmatter `description` (may be empty).
    pub desc: String,
    /// Tools whose folders hold this agent, in scan order.
    pub tools: Vec<String>,
    name_l: String,
    desc_l: String,
}

impl AgentRef {
    fn new(name: &str, desc: &str, tool: &str) -> Self {
        AgentRef {
            name: name.to_string(),
            desc: desc.to_string(),
            tools: vec![tool.to_string()],
            name_l: name.to_lowercase(),
            desc_l: desc.to_lowercase(),
        }
    }

    /// Tool tags for display: `claude, codex`.
    pub fn tag(&self) -> String {
        self.tools.join(", ")
    }
}

/// File stem without `.md` / `.toml` and the Copilot `.agent` infix; None for other files.
fn stem(file: &str, ext: &str) -> Option<String> {
    let s = file.strip_suffix(ext)?;
    let s = s.strip_suffix(".agent").unwrap_or(s);
    (!s.is_empty() && !s.starts_with('.')).then(|| s.to_string())
}

/// Collapse whitespace and cut to `DESC_MAX` chars.
fn shorten(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= DESC_MAX {
        one
    } else {
        let mut t: String = one.chars().take(DESC_MAX - 1).collect();
        t.truncate(t.trim_end().len());
        t.push('…');
        t
    }
}

/// Value of a YAML double-quoted scalar's body (after the opening quote) up to the closing quote.
fn unquote_double(rest: &str) -> String {
    let mut out = String::new();
    let mut it = rest.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => break,
            '\\' => match it.next() {
                Some('n') | Some('t') => out.push(' '),
                Some(o) => out.push(o),
                None => {}
            },
            c => out.push(c),
        }
    }
    out
}

/// Body of a single-quoted scalar (`''` is an escaped quote).
fn unquote_single(rest: &str) -> String {
    let mut out = String::new();
    let mut it = rest.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\'' {
            if it.peek() == Some(&'\'') {
                out.push('\'');
                it.next();
            } else {
                break;
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Scalar after `key:` / `key =`: quoted or plain.
fn scalar(v: &str) -> String {
    let v = v.trim();
    if let Some(r) = v.strip_prefix('"') {
        unquote_double(r)
    } else if let Some(r) = v.strip_prefix('\'') {
        unquote_single(r)
    } else {
        v.to_string()
    }
}

/// `description:` from YAML frontmatter (quoted, plain or a `>`/`|` block).
fn md_description(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return String::new();
    }
    let fm: Vec<&str> = lines.take_while(|l| l.trim() != "---").collect();
    for (i, l) in fm.iter().enumerate() {
        let Some(v) = l.strip_prefix("description:") else { continue };
        let v = v.trim();
        if v.starts_with(['>', '|']) {
            let block: Vec<&str> = fm[i + 1..].iter().take_while(|l| l.starts_with(' ') || l.starts_with('\t')).map(|l| l.trim()).collect();
            return block.join(" ");
        }
        return scalar(v);
    }
    String::new()
}

/// Top-level `description = "..."` of a Codex agent `.toml` (before any multi-line string).
fn toml_description(text: &str) -> String {
    for l in text.lines() {
        let t = l.trim_start();
        if let Some(v) = t.strip_prefix("description").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
            return scalar(v);
        }
        if t.contains("'''") || t.contains("\"\"\"") || t.starts_with('[') {
            break;
        }
    }
    String::new()
}

fn read_head(path: &Path) -> String {
    let mut buf = Vec::new();
    if let Ok(f) = fs::File::open(path) {
        let _ = f.take(READ_MAX).read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn scan_dir(dir: &Path, tool: &str, out: &mut Vec<AgentRef>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut files: Vec<(String, PathBuf)> =
        rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).filter_map(|e| Some((e.file_name().into_string().ok()?, e.path()))).collect();
    files.sort();
    for (file, path) in files.into_iter().take(MAX_PER_DIR) {
        let ext = if tool == "codex" { ".toml" } else { ".md" };
        let Some(name) = stem(&file, ext) else { continue };
        let head = read_head(&path);
        let desc = shorten(&if ext == ".toml" { toml_description(&head) } else { md_description(&head) });
        match out.iter_mut().find(|a| a.name == name) {
            Some(a) => {
                if !a.tools.iter().any(|t| t == tool) {
                    a.tools.push(tool.to_string());
                }
                if a.desc.is_empty() && !desc.is_empty() {
                    a.desc_l = desc.to_lowercase();
                    a.desc = desc;
                }
            }
            None => out.push(AgentRef::new(&name, &desc, tool)),
        }
    }
}

/// Subagents found under `root` (project folders of every tool) and in `home/.claude/agents`
/// (user level, read-only). Deduplicated by name; `pumba-*` first, then alphabetical.
pub fn scan(root: &Path, home: Option<&Path>) -> Vec<AgentRef> {
    let mut out: Vec<AgentRef> = Vec::new();
    for (dir, tool) in PROJECT_DIRS {
        scan_dir(&root.join(dir), tool, &mut out);
    }
    if let Some(h) = home {
        scan_dir(&h.join(".claude/agents"), "claude", &mut out);
    }
    out.sort_by(|a, b| (!a.name_l.starts_with(PACK_PREFIX)).cmp(&!b.name_l.starts_with(PACK_PREFIX)).then_with(|| a.name_l.cmp(&b.name_l)).then_with(|| a.name.cmp(&b.name)));
    out
}

/// User home from the environment (`USERPROFILE`, then `HOME`).
pub fn home_dir() -> Option<PathBuf> {
    ["USERPROFILE", "HOME"].iter().filter_map(|k| std::env::var_os(k)).map(PathBuf::from).find(|p| !p.as_os_str().is_empty())
}

/// Agents matching `query` (case-insensitive): name prefix, name substring, then description.
/// An empty query lists everything. Order inside a tier is the order of `agents`.
pub fn search<'a>(agents: &'a [AgentRef], query: &str, limit: usize) -> Vec<&'a AgentRef> {
    let q = query.trim().to_lowercase();
    let mut hits: Vec<(u8, &AgentRef)> = agents
        .iter()
        .filter_map(|a| {
            let tier = if q.is_empty() || a.name_l.starts_with(&q) {
                0
            } else if a.name_l.contains(&q) {
                1
            } else if a.desc_l.contains(&q) {
                2
            } else {
                return None;
            };
            Some((tier, a))
        })
        .collect();
    hits.sort_by_key(|h| h.0); // stable
    hits.into_iter().take(limit).map(|h| h.1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pumba-agents-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn frontmatter_descriptions() {
        assert_eq!(md_description("---\nname: a\ndescription: Plain text here\n---\nbody"), "Plain text here");
        assert_eq!(md_description("---\ndescription: \"Quoted: with \\\"inner\\\" colon\"\nname: a\n---\n"), "Quoted: with \"inner\" colon");
        assert_eq!(md_description("---\ndescription: 'It''s single'\n---\n"), "It's single");
        assert_eq!(md_description("---\ndescription: >\n  folded one\n  folded two\nname: x\n---\n"), "folded one folded two");
        assert_eq!(md_description("---\nname: a\n---\ndescription: not frontmatter\n"), "");
        assert_eq!(md_description("no frontmatter\ndescription: x"), "");
        assert_eq!(toml_description("# c\nname = \"pumba_worker\"\ndescription = \"Исполнитель: делает\"\ndeveloper_instructions = '''\ndescription = \"no\"\n'''\n"), "Исполнитель: делает");
        assert_eq!(toml_description("name = \"x\"\ndescription = 'lit'\n"), "lit");
        assert_eq!(toml_description("developer_instructions = '''\ndescription = \"no\"\n'''\n"), "");
        let long = "слово ".repeat(30);
        let s = shorten(&long);
        assert_eq!(s.chars().count(), DESC_MAX);
        assert!(s.ends_with('…') && !s.contains("  "));
        assert_eq!(shorten("  a \n b "), "a b");
    }

    #[test]
    fn stems() {
        assert_eq!(stem("pumba-worker.md", ".md").as_deref(), Some("pumba-worker"));
        assert_eq!(stem("pumba-worker.agent.md", ".md").as_deref(), Some("pumba-worker"));
        assert_eq!(stem("pumba-worker.toml", ".toml").as_deref(), Some("pumba-worker"));
        assert_eq!(stem("README.txt", ".md"), None);
        assert_eq!(stem(".md", ".md"), None);
        assert_eq!(stem("x.toml", ".md"), None);
    }

    #[test]
    fn scan_all_tools_dedupe_and_rank() {
        let root = tmp("scan");
        let home = tmp("home");
        let desc = "---\nname: n\ndescription: \"Исполнитель карточки доски pumba. Вызывать как исполнителя колонки «В работе»\"\n---\nbody";
        put(&root, ".claude/agents/pumba-worker.md", desc);
        put(&root, ".claude/agents/pumba-planner.md", "---\ndescription: Plans\n---\n");
        put(&root, ".claude/agents/zeta.md", "---\ndescription: Last one\n---\n");
        put(&root, ".claude/agents/notes.txt", "x");
        put(&root, ".codex/agents/pumba-worker.toml", "name = \"pumba_worker\"\ndescription = \"codex worker\"\n");
        put(&root, ".codex/agents/ignored.md", "---\ndescription: wrong ext\n---\n");
        put(&root, ".cursor/agents/pumba-worker.md", "---\ndescription: cursor worker\n---\n");
        put(&root, ".github/agents/pumba-worker.agent.md", "---\ndescription: \"copilot worker\"\n---\n");
        put(&root, ".opencode/agents/alpha.md", "---\ndescription: opencode alpha\n---\n");
        put(&home, ".claude/agents/alpha.md", "---\ndescription: user alpha\n---\n");
        put(&home, ".claude/agents/mine.md", "---\ndescription: user only\n---\n");
        let a = scan(&root, Some(&home));
        let names: Vec<&str> = a.iter().map(|x| x.name.as_str()).collect();
        // pack first (alphabetical), then the rest alphabetical; deduped across tools and home
        assert_eq!(names, ["pumba-planner", "pumba-worker", "alpha", "mine", "zeta"]);
        let w = &a[1];
        assert_eq!(w.tools, ["claude", "codex", "cursor", "copilot"]);
        assert_eq!(w.tag(), "claude, codex, cursor, copilot");
        // first non-empty description wins (claude), cut to 60 chars
        assert!(w.desc.starts_with("Исполнитель карточки доски pumba.") && w.desc.chars().count() <= DESC_MAX);
        assert!(w.desc.ends_with('…'));
        assert_eq!(a[2].tools, ["opencode", "claude"]);
        assert_eq!(a[2].desc, "opencode alpha");
        assert_eq!(a[3].desc, "user only");
        // no home: the user-level agent disappears
        assert!(!scan(&root, None).iter().any(|x| x.name == "mine"));
        // a codex-only agent keeps the stem as its name
        put(&root, ".codex/agents/only_codex.toml", "name = \"only_codex\"\ndescription = \"c\"\n");
        assert!(scan(&root, None).iter().any(|x| x.name == "only_codex" && x.tools == ["codex"]));
        // missing project: empty
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&home);
        assert!(scan(&root, Some(&home)).is_empty());
    }

    #[test]
    fn search_tiers() {
        let root = tmp("search");
        put(&root, ".claude/agents/pumba-worker.md", "---\ndescription: does the work\n---\n");
        put(&root, ".claude/agents/pumba-tester.md", "---\ndescription: runs tests\n---\n");
        put(&root, ".claude/agents/my-worker.md", "---\ndescription: mine\n---\n");
        put(&root, ".claude/agents/helper.md", "---\ndescription: a worker for chores\n---\n");
        let a = scan(&root, None);
        let names = |q: &str| search(&a, q, 10).iter().map(|x| x.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(""), ["pumba-tester", "pumba-worker", "helper", "my-worker"]);
        assert_eq!(names("pum"), ["pumba-tester", "pumba-worker"]);
        // prefix, then substring, then description
        assert_eq!(names("WORKER"), ["pumba-worker", "my-worker", "helper"]);
        assert_eq!(names("tests"), ["pumba-tester"]);
        assert!(names("zzz").is_empty());
        assert_eq!(search(&a, "", 2).len(), 2);
        let _ = fs::remove_dir_all(&root);
    }
}
