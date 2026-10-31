//! Установка пакета шаблонов (агенты и скиллы) в каталоги ИИ-инструментов проекта.
//! Файлы вшиты в бинарник через `include_str!`.
//!
//! Чтобы добавить инструмент: вариант в `Tool` (и `Tool::id`), строка в `tools()`,
//! таблица файлов и ветка в `table()`. Остальное (CLI, окно выбора в TUI, `board.json`)
//! подхватит реестр. Если пакет читается чужим инструментом (дубли скиллов), добавьте
//! правило в `foreign_dirs()`.

use std::fs;
use std::path::Path;

/// Claude Code: (путь назначения относительно корня проекта, содержимое)
const CLAUDE_FILES: &[(&str, &str)] = &[
    (
        ".claude/agents/pumba-planner.md",
        include_str!("../../../templates/agents/pumba-planner.md"),
    ),
    (
        ".claude/agents/pumba-tester.md",
        include_str!("../../../templates/agents/pumba-tester.md"),
    ),
    (
        ".claude/agents/pumba-worker.md",
        include_str!("../../../templates/agents/pumba-worker.md"),
    ),
    (
        ".claude/agents/pumba-reviewer.md",
        include_str!("../../../templates/agents/pumba-reviewer.md"),
    ),
    (
        ".claude/agents/pumba-committer.md",
        include_str!("../../../templates/agents/pumba-committer.md"),
    ),
    (
        ".claude/skills/pumba-task/SKILL.md",
        include_str!("../../../templates/skills/pumba-task/SKILL.md"),
    ),
    (
        ".claude/skills/pumba-new-card/SKILL.md",
        include_str!("../../../templates/skills/pumba-new-card/SKILL.md"),
    ),
];

/// OpenAI Codex: (путь назначения относительно корня проекта, содержимое). Исходники: `templates/targets/codex/`.
const CODEX_FILES: &[(&str, &str)] = &[
    (
        ".agents/skills/pumba-new-card/SKILL.md",
        include_str!("../../../templates/targets/codex/.agents/skills/pumba-new-card/SKILL.md"),
    ),
    (
        ".agents/skills/pumba-task/SKILL.md",
        include_str!("../../../templates/targets/codex/.agents/skills/pumba-task/SKILL.md"),
    ),
    (
        ".codex/agents/pumba-committer.toml",
        include_str!("../../../templates/targets/codex/.codex/agents/pumba-committer.toml"),
    ),
    (
        ".codex/agents/pumba-planner.toml",
        include_str!("../../../templates/targets/codex/.codex/agents/pumba-planner.toml"),
    ),
    (
        ".codex/agents/pumba-reviewer.toml",
        include_str!("../../../templates/targets/codex/.codex/agents/pumba-reviewer.toml"),
    ),
    (
        ".codex/agents/pumba-tester.toml",
        include_str!("../../../templates/targets/codex/.codex/agents/pumba-tester.toml"),
    ),
    (
        ".codex/agents/pumba-worker.toml",
        include_str!("../../../templates/targets/codex/.codex/agents/pumba-worker.toml"),
    ),
];

/// Cursor: (путь назначения относительно корня проекта, содержимое). Исходники: `templates/targets/cursor/`.
const CURSOR_FILES: &[(&str, &str)] = &[
    (
        ".cursor/agents/pumba-committer.md",
        include_str!("../../../templates/targets/cursor/.cursor/agents/pumba-committer.md"),
    ),
    (
        ".cursor/agents/pumba-planner.md",
        include_str!("../../../templates/targets/cursor/.cursor/agents/pumba-planner.md"),
    ),
    (
        ".cursor/agents/pumba-reviewer.md",
        include_str!("../../../templates/targets/cursor/.cursor/agents/pumba-reviewer.md"),
    ),
    (
        ".cursor/agents/pumba-tester.md",
        include_str!("../../../templates/targets/cursor/.cursor/agents/pumba-tester.md"),
    ),
    (
        ".cursor/agents/pumba-worker.md",
        include_str!("../../../templates/targets/cursor/.cursor/agents/pumba-worker.md"),
    ),
    (
        ".cursor/rules/pumba.mdc",
        include_str!("../../../templates/targets/cursor/.cursor/rules/pumba.mdc"),
    ),
    (
        ".cursor/skills/pumba-new-card/SKILL.md",
        include_str!("../../../templates/targets/cursor/.cursor/skills/pumba-new-card/SKILL.md"),
    ),
    (
        ".cursor/skills/pumba-task/SKILL.md",
        include_str!("../../../templates/targets/cursor/.cursor/skills/pumba-task/SKILL.md"),
    ),
];

/// GitHub Copilot: (путь назначения относительно корня проекта, содержимое). Исходники: `templates/targets/copilot/`.
const COPILOT_FILES: &[(&str, &str)] = &[
    (
        ".github/agents/pumba-committer.agent.md",
        include_str!("../../../templates/targets/copilot/.github/agents/pumba-committer.agent.md"),
    ),
    (
        ".github/agents/pumba-planner.agent.md",
        include_str!("../../../templates/targets/copilot/.github/agents/pumba-planner.agent.md"),
    ),
    (
        ".github/agents/pumba-reviewer.agent.md",
        include_str!("../../../templates/targets/copilot/.github/agents/pumba-reviewer.agent.md"),
    ),
    (
        ".github/agents/pumba-tester.agent.md",
        include_str!("../../../templates/targets/copilot/.github/agents/pumba-tester.agent.md"),
    ),
    (
        ".github/agents/pumba-worker.agent.md",
        include_str!("../../../templates/targets/copilot/.github/agents/pumba-worker.agent.md"),
    ),
    (
        ".github/instructions/pumba.instructions.md",
        include_str!("../../../templates/targets/copilot/.github/instructions/pumba.instructions.md"),
    ),
    (
        ".github/skills/pumba-new-card/SKILL.md",
        include_str!("../../../templates/targets/copilot/.github/skills/pumba-new-card/SKILL.md"),
    ),
    (
        ".github/skills/pumba-task/SKILL.md",
        include_str!("../../../templates/targets/copilot/.github/skills/pumba-task/SKILL.md"),
    ),
];

/// OpenCode: (путь назначения относительно корня проекта, содержимое). Исходники: `templates/targets/opencode/`.
const OPENCODE_FILES: &[(&str, &str)] = &[
    (
        ".opencode/agents/pumba-committer.md",
        include_str!("../../../templates/targets/opencode/.opencode/agents/pumba-committer.md"),
    ),
    (
        ".opencode/agents/pumba-planner.md",
        include_str!("../../../templates/targets/opencode/.opencode/agents/pumba-planner.md"),
    ),
    (
        ".opencode/agents/pumba-reviewer.md",
        include_str!("../../../templates/targets/opencode/.opencode/agents/pumba-reviewer.md"),
    ),
    (
        ".opencode/agents/pumba-tester.md",
        include_str!("../../../templates/targets/opencode/.opencode/agents/pumba-tester.md"),
    ),
    (
        ".opencode/agents/pumba-worker.md",
        include_str!("../../../templates/targets/opencode/.opencode/agents/pumba-worker.md"),
    ),
    (
        ".opencode/commands/pumba-new-card.md",
        include_str!("../../../templates/targets/opencode/.opencode/commands/pumba-new-card.md"),
    ),
    (
        ".opencode/commands/pumba-task.md",
        include_str!("../../../templates/targets/opencode/.opencode/commands/pumba-task.md"),
    ),
    (
        ".opencode/skills/pumba-new-card/SKILL.md",
        include_str!("../../../templates/targets/opencode/.opencode/skills/pumba-new-card/SKILL.md"),
    ),
    (
        ".opencode/skills/pumba-task/SKILL.md",
        include_str!("../../../templates/targets/opencode/.opencode/skills/pumba-task/SKILL.md"),
    ),
];

/// Инструмент, для которого есть (или будет) адаптер пакета.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Claude,
    Codex,
    Cursor,
    Copilot,
    OpenCode,
}

impl Tool {
    /// Стабильный id: пишется в `board.json` (`tools`) и принимается в `--tools`.
    pub fn id(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Codex => "codex",
            Tool::Cursor => "cursor",
            Tool::Copilot => "copilot",
            Tool::OpenCode => "opencode",
        }
    }

    pub fn info(self) -> ToolInfo {
        tools().into_iter().find(|t| t.tool == self).expect("инструмент есть в реестре")
    }
}

/// Запись реестра.
#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub tool: Tool,
    pub id: &'static str,
    /// Название для людей.
    pub name: &'static str,
    /// Пути относительно корня проекта: если хоть один есть, инструмент в проекте используется.
    /// Общий `AGENTS.md` сюда не входит: его читают почти все инструменты.
    pub detect: &'static [&'static str],
    /// Адаптер готов: файлы можно ставить.
    pub ready: bool,
}

/// Реестр инструментов, порядок = порядок в окнах выбора. Будущие id: gemini, windsurf.
pub fn tools() -> Vec<ToolInfo> {
    vec![
        ToolInfo { tool: Tool::Claude, id: "claude", name: "Claude Code", detect: &[".claude", "CLAUDE.md"], ready: true },
        ToolInfo { tool: Tool::Codex, id: "codex", name: "OpenAI Codex", detect: &[".codex"], ready: true },
        ToolInfo { tool: Tool::Cursor, id: "cursor", name: "Cursor", detect: &[".cursor", ".cursorrules"], ready: true },
        ToolInfo {
            tool: Tool::Copilot,
            id: "copilot",
            name: "GitHub Copilot",
            detect: &[".github/copilot-instructions.md", ".github/agents", ".github/instructions"],
            ready: true,
        },
        ToolInfo { tool: Tool::OpenCode, id: "opencode", name: "OpenCode", detect: &[".opencode", "opencode.json"], ready: true },
    ]
}

/// Готовые к установке инструменты.
pub fn ready_tools() -> Vec<ToolInfo> {
    tools().into_iter().filter(|t| t.ready).collect()
}

/// Таблица файлов пакета инструмента.
fn table(tool: Tool) -> &'static [(&'static str, &'static str)] {
    match tool {
        Tool::Claude => CLAUDE_FILES,
        Tool::Codex => CODEX_FILES,
        Tool::Cursor => CURSOR_FILES,
        Tool::Copilot => COPILOT_FILES,
        Tool::OpenCode => OPENCODE_FILES,
    }
}

/// Адаптер: файлы пакета инструмента: (путь назначения от корня проекта, содержимое).
fn files(tool: Tool) -> Vec<(String, String)> {
    table(tool).iter().map(|(p, c)| ((*p).to_string(), (*c).to_string())).collect()
}

/// Инструменты, чьи пути из `detect` есть в `root` (порядок реестра).
pub fn detect(root: &Path) -> Vec<Tool> {
    tools().into_iter().filter(|t| t.detect.iter().any(|d| root.join(d).exists())).map(|t| t.tool).collect()
}

/// Выбор по умолчанию: найденные в проекте готовые инструменты, иначе Claude Code.
pub fn preselect(root: &Path) -> Vec<Tool> {
    let found: Vec<Tool> = detect(root).into_iter().filter(|t| t.info().ready).collect();
    if found.is_empty() { vec![Tool::Claude] } else { found }
}

/// Инструменты по сохранённым id (неизвестные и неготовые пропускаются), порядок реестра.
pub fn from_ids(ids: &[String]) -> Vec<Tool> {
    ready_tools().into_iter().filter(|t| ids.iter().any(|i| i == t.id)).map(|t| t.tool).collect()
}

/// Список id для `board.json`.
pub fn to_ids(tools: &[Tool]) -> Vec<String> {
    tools.iter().map(|t| t.id().to_string()).collect()
}

/// Разбор `--tools`: id через запятую или `all` (все готовые). Порядок реестра, без повторов.
pub fn parse_tools(s: &str) -> Result<Vec<Tool>, String> {
    let reg = tools();
    let ids = |only_ready: bool| reg.iter().filter(|t| t.ready || !only_ready).map(|t| t.id).collect::<Vec<_>>().join(", ");
    let mut out: Vec<Tool> = Vec::new();
    for part in s.split(',') {
        let id = part.trim().to_lowercase();
        if id.is_empty() {
            continue;
        }
        if id == "all" {
            out.extend(reg.iter().filter(|t| t.ready).map(|t| t.tool));
            continue;
        }
        match reg.iter().find(|t| t.id == id) {
            Some(t) if t.ready => out.push(t.tool),
            Some(t) => return Err(format!("инструмент «{}» пока не поддерживается. Готовы: {}", t.id, ids(true))),
            None => return Err(format!("неизвестный инструмент «{}». Допустимо: {}, all", part.trim(), ids(false))),
        }
    }
    if out.is_empty() {
        return Err(format!("пустой список инструментов. Допустимо: {}, all", ids(false)));
    }
    let mut seen = Vec::new();
    out.retain(|t| {
        let fresh = !seen.contains(t);
        seen.push(*t);
        fresh
    });
    out.sort_by_key(|t| reg.iter().position(|r| r.tool == *t));
    Ok(out)
}

/// Какие папки пакета `source` читает инструмент `reader` помимо своих (`None`: не читает).
/// Источник: `templates/targets/README.md`, «Кто какие папки читает».
fn foreign_dirs(reader: Tool, source: Tool) -> Option<&'static str> {
    match (reader, source) {
        (Tool::Cursor, Tool::Claude) | (Tool::Copilot, Tool::Claude) => Some(".claude/agents, .claude/skills"),
        (Tool::OpenCode, Tool::Claude) => Some(".claude/skills"),
        (Tool::Cursor, Tool::Codex) => Some(".agents/skills, .codex/agents"),
        (Tool::Copilot, Tool::Codex) | (Tool::OpenCode, Tool::Codex) => Some(".agents/skills"),
        _ => None,
    }
}

/// Предупреждения о сочетаниях: инструмент увидит несколько копий скилла `pumba-task`.
/// По одному на инструмент-читатель. Установку не блокируют.
pub fn warnings(tools: &[Tool]) -> Vec<String> {
    let mut out = Vec::new();
    for reader in tools_in_order(tools) {
        let src: Vec<String> = tools_in_order(tools)
            .into_iter()
            .filter_map(|s| foreign_dirs(reader, s).map(|d| format!("{} ({d})", s.info().name)))
            .collect();
        if src.is_empty() {
            continue;
        }
        let name = reader.info().name;
        out.push(format!(
            "{name} читает и чужие копии пакета: {}. Скилл `pumba-task` может оказаться в нескольких экземплярах; если {name} выберет не свою копию, удалите лишнюю.",
            src.join("; ")
        ));
    }
    out
}

/// Выбранные инструменты без повторов, порядок реестра.
fn tools_in_order(tools: &[Tool]) -> Vec<Tool> {
    tools_registry_order().into_iter().filter(|t| tools.contains(t)).collect()
}

fn tools_registry_order() -> Vec<Tool> {
    self::tools().into_iter().map(|t| t.tool).collect()
}

/// Результат установки для одного инструмента.
#[derive(Debug, Clone)]
pub struct ToolReport {
    pub tool: Tool,
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, Default)]
pub struct InstallReport {
    /// Записанные файлы всех инструментов (относительные пути, `/`).
    pub installed: Vec<String>,
    /// Пропущенные: уже существуют (или совпадают по содержимому).
    pub skipped: Vec<String>,
    /// То же, сгруппировано по инструментам.
    pub per_tool: Vec<ToolReport>,
    /// Предупреждения о сочетаниях инструментов (по-русски). Установку не блокируют.
    pub warnings: Vec<String>,
}

/// Ставит пакет в `root` для выбранных инструментов. Существующие файлы не трогает,
/// кроме случая `force` (тогда перезаписывает, если содержимое отличается).
pub fn install(root: &Path, tools: &[Tool], force: bool) -> Result<InstallReport, String> {
    let mut report = InstallReport::default();
    let mut done: Vec<Tool> = Vec::new();
    for &tool in tools {
        if done.contains(&tool) {
            continue;
        }
        done.push(tool);
        let info = tool.info();
        if !info.ready {
            return Err(format!("инструмент «{}» пока не поддерживается", info.id));
        }
        let mut tr = ToolReport { tool, installed: Vec::new(), skipped: Vec::new() };
        for (rel, content) in files(tool) {
            let dest = root.join(&rel);
            if dest.is_file() {
                // Нечитаемый файл считаем отличающимся.
                let same = fs::read(&dest).map(|b| b == content.as_bytes()).unwrap_or(false);
                if same || !force {
                    tr.skipped.push(rel);
                    continue;
                }
            } else if dest.exists() {
                return Err(format!("{}: путь занят, это не файл", dest.display()));
            }
            if let Some(dir) = dest.parent() {
                fs::create_dir_all(dir)
                    .map_err(|e| format!("не удалось создать каталог {}: {e}", dir.display()))?;
            }
            fs::write(&dest, content.as_bytes())
                .map_err(|e| format!("не удалось записать {}: {e}", dest.display()))?;
            tr.installed.push(rel);
        }
        report.installed.extend(tr.installed.iter().cloned());
        report.skipped.extend(tr.skipped.iter().cloned());
        report.per_tool.push(tr);
    }
    report.warnings = warnings(&done);
    Ok(report)
}

/// Ставит пакет Claude Code в `root/.claude/` (см. `install`).
#[allow(dead_code)]
pub fn install_claude(root: &Path, force: bool) -> Result<InstallReport, String> {
    install(root, &[Tool::Claude], force)
}

/// Одной строкой для строки состояния TUI: первое предупреждение и «и ещё N».
pub fn warnings_line(w: &[String]) -> Option<String> {
    let first = w.first()?;
    Some(if w.len() > 1 { format!("{first} (и ещё {})", w.len() - 1) } else { first.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);
    const FILES: &[(&str, &str)] = CLAUDE_FILES;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Tmp {
            let p = std::env::temp_dir().join(format!(
                "pumba-install-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const ALL: [Tool; 5] = [Tool::Claude, Tool::Codex, Tool::Cursor, Tool::Copilot, Tool::OpenCode];

    /// Все файлы под `dir` (пути относительно `dir`, `/`), отсортированы.
    fn walk(dir: &Path) -> Vec<String> {
        fn rec(base: &Path, dir: &Path, out: &mut Vec<String>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    rec(base, &p, out);
                } else {
                    out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut out = Vec::new();
        rec(dir, dir, &mut out);
        out.sort();
        out
    }

    // ------------------------------------------------------------ claude pack

    #[test]
    fn installs_full_set_into_empty_dir() {
        let t = Tmp::new();
        let r = install_claude(&t.0, false).unwrap();
        assert_eq!(r.installed.len(), FILES.len());
        assert!(r.skipped.is_empty() && r.warnings.is_empty());
        for (rel, content) in FILES {
            assert_eq!(&fs::read_to_string(t.0.join(rel)).unwrap(), content);
        }
        assert_eq!(FILES.len(), 7);
        assert!(r.installed.iter().all(|p| !p.contains('\\')));
    }

    #[test]
    fn second_run_skips_everything() {
        let t = Tmp::new();
        install_claude(&t.0, false).unwrap();
        let r = install_claude(&t.0, false).unwrap();
        assert!(r.installed.is_empty());
        assert_eq!(r.skipped.len(), FILES.len());
    }

    #[test]
    fn modified_file_skipped_then_overwritten_with_force() {
        let t = Tmp::new();
        install_claude(&t.0, false).unwrap();
        let rel = FILES[0].0;
        let p = t.0.join(rel);
        fs::write(&p, "my edits\n").unwrap();

        let r = install_claude(&t.0, false).unwrap();
        assert!(r.installed.is_empty());
        assert!(r.skipped.contains(&rel.to_string()));
        assert_eq!(fs::read_to_string(&p).unwrap(), "my edits\n");

        let r = install_claude(&t.0, true).unwrap();
        assert_eq!(r.installed, vec![rel.to_string()]);
        assert_eq!(r.skipped.len(), FILES.len() - 1);
        assert_eq!(fs::read_to_string(&p).unwrap(), FILES[0].1);
    }

    #[test]
    fn identical_existing_file_counts_as_skipped_even_with_force() {
        let t = Tmp::new();
        let (rel, content) = FILES[1];
        let p = t.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        let r = install_claude(&t.0, true).unwrap();
        assert!(r.skipped.contains(&rel.to_string()));
        assert!(!r.installed.contains(&rel.to_string()));
        assert_eq!(r.installed.len(), FILES.len() - 1);
    }

    #[test]
    fn does_not_touch_other_files() {
        let t = Tmp::new();
        let other = t.0.join(".claude/agents/mine.md");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(&other, "keep").unwrap();
        install_claude(&t.0, true).unwrap();
        assert_eq!(fs::read_to_string(&other).unwrap(), "keep");
    }

    // ------------------------------------------------- content checks (all tools)

    /// Строки frontmatter между первыми двумя `---` (None: файл не начинается с `---` или не закрыт).
    fn frontmatter(content: &str) -> Option<Vec<&str>> {
        let mut lines = content.lines();
        if lines.next()? != "---" {
            return None;
        }
        let mut out = Vec::new();
        for l in lines {
            if l == "---" {
                return Some(out);
            }
            out.push(l);
        }
        None
    }

    /// Значение ключа верхнего уровня.
    fn fm_value<'a>(fm: &[&'a str], key: &str) -> Option<&'a str> {
        let prefix = format!("{key}:");
        fm.iter().find_map(|l| l.strip_prefix(prefix.as_str())).map(str::trim)
    }

    /// Строка в двойных кавычках: закрыта, внутри нет неэкранированной `"`, после закрытия пусто.
    fn check_double_quoted(v: &str) -> Result<(), String> {
        let rest = v.strip_prefix('"').ok_or("нет открывающей кавычки")?;
        let mut esc = false;
        for (i, c) in rest.char_indices() {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                return if rest[i + 1..].trim().is_empty() { Ok(()) } else { Err(format!("текст после закрывающей кавычки: {v}")) };
            }
        }
        Err(format!("кавычка не закрыта: {v}"))
    }

    /// Строгая проверка скалярного значения верхнего уровня. Допустимы: строка в двойных или
    /// одинарных кавычках, блочный скаляр (`|`, `>`), flow-список/словарь (`[`, `{`), простой
    /// скаляр без `: ` и ` #`.
    fn check_yaml_scalar(v: &str) -> Result<(), String> {
        if v.is_empty() || v.starts_with(['|', '>', '[', '{']) {
            return Ok(());
        }
        if v.starts_with('"') {
            return check_double_quoted(v);
        }
        if let Some(rest) = v.strip_prefix('\'') {
            let inner = rest.strip_suffix('\'').ok_or_else(|| format!("кавычка не закрыта: {v}"))?;
            return if inner.replace("''", "").contains('\'') { Err(format!("одинарная кавычка не удвоена: {v}")) } else { Ok(()) };
        }
        if v.contains(": ") || v.contains(" #") || v.ends_with(':') {
            return Err(format!("простой скаляр с «: » или « #» ломает YAML, нужны кавычки: {v}"));
        }
        if v.starts_with(['*', '&', '!', '%', '@', '`', '?']) {
            return Err(format!("простой скаляр начинается со служебного символа: {v}"));
        }
        Ok(())
    }

    fn check_frontmatter(rel: &str, content: &str) -> Result<Vec<String>, String> {
        let fm = frontmatter(content).ok_or("нет закрытого frontmatter")?;
        let mut keys = Vec::new();
        for l in &fm {
            if l.trim().is_empty() || l.starts_with([' ', '\t', '#']) || l.starts_with("- ") {
                continue;
            }
            let (k, v) = l.split_once(':').ok_or_else(|| format!("{rel}: строка без ключа: {l}"))?;
            if k.is_empty() || k.contains(' ') {
                return Err(format!("{rel}: плохой ключ: {l}"));
            }
            check_yaml_scalar(v.trim()).map_err(|e| format!("{rel}, {k}: {e}"))?;
            keys.push(k.to_string());
        }
        let d = fm_value(&fm, "description").ok_or_else(|| format!("{rel}: нет description"))?;
        if d.is_empty() || d == "\"\"" {
            return Err(format!("{rel}: пустой description"));
        }
        Ok(keys)
    }

    /// Ожидаемое `name` во frontmatter по пути (None: `name` быть не должно или не проверяется).
    fn expected_name(rel: &str) -> Option<String> {
        if let Some(dir) = rel.strip_suffix("/SKILL.md") {
            return dir.rsplit('/').next().map(str::to_string);
        }
        let file = rel.rsplit('/').next().unwrap();
        if rel.starts_with(".claude/agents/") || rel.starts_with(".cursor/agents/") {
            return file.strip_suffix(".md").map(str::to_string);
        }
        if rel.starts_with(".github/agents/") {
            return file.strip_suffix(".agent.md").map(str::to_string);
        }
        None
    }

    /// Простая проверка TOML-агента Codex по строкам.
    fn check_codex_toml(rel: &str, content: &str) -> Result<(), String> {
        let stem = rel.rsplit('/').next().unwrap().strip_suffix(".toml").ok_or("не .toml")?;
        let line = |key: &str| content.lines().find_map(|l| l.strip_prefix(&format!("{key} = ")));
        let name = line("name").ok_or("нет name")?;
        check_double_quoted(name)?;
        let want = format!("\"{}\"", stem.replace('-', "_"));
        if name != want {
            return Err(format!("name {name}, ожидалось {want}"));
        }
        check_double_quoted(line("description").ok_or("нет description")?)?;
        if line("developer_instructions") != Some("'''") {
            return Err("developer_instructions должен быть многострочной литеральной строкой '''".into());
        }
        if content.matches("'''").count() != 2 {
            return Err("'''-блок не закрыт или закрыт раньше времени".into());
        }
        if !content.trim_end().ends_with("'''") {
            return Err("после закрывающего ''' есть лишний текст".into());
        }
        Ok(())
    }

    #[test]
    fn every_ready_tool_has_clean_well_formed_files() {
        let expected = [("claude", 7), ("codex", 7), ("cursor", 8), ("copilot", 8), ("opencode", 9)];
        let ready = ready_tools();
        assert_eq!(ready.len(), 5);
        let mut total = 0;
        for (id, count) in expected {
            let info = ready.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("нет {id}"));
            let f = table(info.tool);
            assert_eq!(f.len(), count, "{id}");
            total += f.len();
            for (i, (rel, content)) in f.iter().enumerate() {
                let who = format!("{id}: {rel}");
                // путь: относительный, только `/`, без `..`, без дублей внутри инструмента
                assert!(!rel.is_empty() && !rel.starts_with('/') && !rel.contains('\\') && !rel.contains(':'), "{who}");
                assert!(rel.split('/').all(|s| !s.is_empty() && s != ".." && s != "."), "{who}");
                assert!(rel.contains("pumba"), "{who}: путь без pumba");
                assert!(f.iter().skip(i + 1).all(|(o, _)| o != rel), "дубль {who}");
                // содержимое
                assert!(!content.trim().is_empty(), "{who}: пусто");
                assert!(!content.starts_with('\u{feff}'), "BOM: {who}");
                assert!(!content.contains('\r'), "CR: {who}");
                assert!(content.ends_with('\n'), "нет \\n в конце: {who}");

                if rel.ends_with(".toml") {
                    assert_eq!(id, "codex", "{who}");
                    check_codex_toml(rel, content).unwrap_or_else(|e| panic!("{who}: {e}"));
                } else if rel.ends_with(".md") || rel.ends_with(".mdc") {
                    if !content.starts_with("---") {
                        panic!("{who}: нет frontmatter");
                    }
                    let keys = check_frontmatter(rel, content).unwrap_or_else(|e| panic!("{who}: {e}"));
                    let fm = frontmatter(content).unwrap();
                    let name = fm_value(&fm, "name").map(|v| v.trim_matches('"').to_string());
                    match expected_name(rel) {
                        Some(want) => assert_eq!(name.as_deref(), Some(want.as_str()), "{who}"),
                        None => assert!(!(id == "opencode" && rel.contains("/agents/")) || name.is_none(), "{who}: у агента OpenCode не должно быть name"),
                    }
                    if id == "opencode" && rel.contains("/agents/") {
                        assert!(!keys.iter().any(|k| k == "name"), "{who}");
                    }
                } else {
                    panic!("{who}: неожиданное расширение");
                }
            }
        }
        assert_eq!(total, 39);
        // 32 файла чужих пакетов
        assert_eq!(total - CLAUDE_FILES.len(), 32);
    }

    #[test]
    fn yaml_helper_catches_unquoted_colon_and_bad_quotes() {
        assert!(check_yaml_scalar("Роль: исполнитель").is_err());
        assert!(check_yaml_scalar("текст #комментарий").is_err());
        assert!(check_yaml_scalar("\"Роль: исполнитель\"").is_ok());
        assert!(check_yaml_scalar("\"внутри \"кавычки\" плохо\"").is_err());
        assert!(check_yaml_scalar("\"внутри \\\"кавычки\\\" хорошо\"").is_ok());
        assert!(check_yaml_scalar("\"не закрыта").is_err());
        assert!(check_yaml_scalar("'it''s'").is_ok() && check_yaml_scalar("'it's'").is_err());
        assert!(check_yaml_scalar(">").is_ok() && check_yaml_scalar("['read', 'search']").is_ok());
        assert!(check_yaml_scalar("haiku").is_ok() && check_yaml_scalar("Bash, Read").is_ok());
        assert!(check_frontmatter("x.md", "---\nname: x\ndescription: Роль: плохо\n---\n").is_err());
        assert!(check_frontmatter("x.md", "---\nname: x\ndescription: \"Роль: ок\"\n---\n").is_ok());
        assert!(check_frontmatter("x.md", "---\nname: x\n---\n").is_err());
        assert!(check_frontmatter("x.md", "---\nname: x\ndescription: d\n").is_err());
        assert!(check_codex_toml(".codex/agents/pumba-a.toml", "name = \"pumba_a\"\ndescription = \"d\"\ndeveloper_instructions = '''\nx\n'''\n").is_ok());
        assert!(check_codex_toml(".codex/agents/pumba-a.toml", "name = \"pumba-a\"\ndescription = \"d\"\ndeveloper_instructions = '''\nx\n'''\n").is_err());
    }

    // ---------------------------------------------------------- install per tool

    #[test]
    fn install_each_tool_creates_exactly_its_files_and_second_run_skips_all() {
        for tool in ALL {
            let t = Tmp::new();
            let want: Vec<String> = {
                let mut v: Vec<String> = table(tool).iter().map(|(p, _)| p.to_string()).collect();
                v.sort();
                v
            };
            let r = install(&t.0, &[tool], false).unwrap();
            assert_eq!(walk(&t.0), want, "{}", tool.id());
            assert_eq!(r.installed.len(), want.len(), "{}", tool.id());
            assert!(r.skipped.is_empty() && r.warnings.is_empty(), "{}: {:?}", tool.id(), r.warnings);
            for (rel, content) in table(tool) {
                assert_eq!(&fs::read_to_string(t.0.join(rel)).unwrap(), content, "{rel}");
            }
            let r = install(&t.0, &[tool], false).unwrap();
            assert!(r.installed.is_empty(), "{}", tool.id());
            assert_eq!(r.skipped.len(), want.len(), "{}", tool.id());
            assert!(r.warnings.is_empty(), "один инструмент: предупреждений нет ({})", tool.id());
            assert_eq!(walk(&t.0), want, "{}", tool.id());
        }
    }

    #[test]
    fn install_all_writes_every_file_without_collisions() {
        let t = Tmp::new();
        let r = install(&t.0, &ALL, false).unwrap();
        assert_eq!(r.installed.len(), 39);
        assert!(r.skipped.is_empty(), "пути разных инструментов не пересекаются: {:?}", r.skipped);
        assert_eq!(walk(&t.0).len(), 39);
        assert_eq!(r.per_tool.len(), 5);
        assert_eq!(r.warnings.len(), 3, "{:?}", r.warnings);
    }

    #[test]
    fn install_never_writes_outside_tool_folders_or_touches_user_files() {
        let t = Tmp::new();
        for (name, body) in [("AGENTS.md", "mine"), ("CLAUDE.md", "mine"), ("opencode.json", "{}")] {
            fs::write(t.0.join(name), body).unwrap();
        }
        fs::create_dir_all(t.0.join(".github")).unwrap();
        fs::write(t.0.join(".github/copilot-instructions.md"), "mine").unwrap();
        fs::create_dir_all(t.0.join(".codex")).unwrap();
        fs::write(t.0.join(".codex/config.toml"), "mine").unwrap();
        install(&t.0, &ALL, true).unwrap();
        for p in ["AGENTS.md", "CLAUDE.md", "opencode.json", ".github/copilot-instructions.md", ".codex/config.toml"] {
            assert_eq!(fs::read_to_string(t.0.join(p)).unwrap(), if p == "opencode.json" { "{}" } else { "mine" }, "{p}");
        }
        // для не-Claude целей в .claude/ ничего не пишется
        let t2 = Tmp::new();
        install(&t2.0, &[Tool::Codex, Tool::Cursor, Tool::Copilot, Tool::OpenCode], false).unwrap();
        assert!(!t2.0.join(".claude").exists());
        // в .agents/ пишет только codex, и только скиллы pumba-*
        for f in walk(&t2.0).iter().filter(|f| f.starts_with(".agents/")) {
            assert!(f.starts_with(".agents/skills/pumba-"), "{f}");
        }
    }

    // --------------------------------------------------------------- warnings

    #[test]
    fn warnings_for_duplicate_skill_combinations_only() {
        let w = |v: &[Tool]| warnings(v);
        for t in ALL {
            assert!(w(&[t]).is_empty(), "{}", t.id());
        }
        assert!(w(&[]).is_empty());
        // Claude и Codex не видят друг друга; Cursor и Copilot тоже
        assert!(w(&[Tool::Claude, Tool::Codex]).is_empty());
        assert!(w(&[Tool::Cursor, Tool::Copilot]).is_empty());
        assert!(w(&[Tool::Cursor, Tool::Copilot, Tool::OpenCode]).is_empty());

        let cx = w(&[Tool::Codex, Tool::Cursor]);
        assert_eq!(cx.len(), 1);
        assert!(cx[0].starts_with("Cursor читает") && cx[0].contains("OpenAI Codex (.agents/skills, .codex/agents)"), "{}", cx[0]);
        assert!(cx[0].contains("`pumba-task`") && cx[0].contains("удалите лишнюю"));

        let cl = w(&[Tool::Claude, Tool::Copilot]);
        assert_eq!(cl.len(), 1);
        assert!(cl[0].starts_with("GitHub Copilot читает") && cl[0].contains("Claude Code (.claude/agents, .claude/skills)"), "{}", cl[0]);

        let oc = w(&[Tool::Claude, Tool::OpenCode]);
        assert_eq!(oc.len(), 1);
        assert!(oc[0].contains("Claude Code (.claude/skills)") && !oc[0].contains(".claude/agents"), "{}", oc[0]);

        // Claude + Codex + Cursor: у Cursor оба источника в одном предупреждении
        let m = w(&[Tool::Cursor, Tool::Codex, Tool::Claude, Tool::Cursor]);
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("Claude Code") && m[0].contains("OpenAI Codex") && m[0].contains("; "), "{}", m[0]);

        let all = w(&ALL);
        assert_eq!(all.len(), 3);
        assert!(all[0].starts_with("Cursor") && all[1].starts_with("GitHub Copilot") && all[2].starts_with("OpenCode"));
    }

    #[test]
    fn install_reports_warnings_and_still_installs() {
        let t = Tmp::new();
        let r = install(&t.0, &[Tool::Codex, Tool::Cursor], false).unwrap();
        assert_eq!(r.warnings.len(), 1);
        assert_eq!(r.installed.len(), 7 + 8);
        let r = install(&t.0, &[Tool::Claude], false).unwrap();
        assert!(r.warnings.is_empty());
        // повторный запуск с тем же выбором повторяет предупреждение
        let r = install(&t.0, &[Tool::Cursor, Tool::Codex], false).unwrap();
        assert_eq!(r.warnings.len(), 1);
        assert!(r.installed.is_empty());
    }

    #[test]
    fn warnings_line_shows_first_and_count() {
        assert_eq!(warnings_line(&[]), None);
        assert_eq!(warnings_line(&["a".into()]).as_deref(), Some("a"));
        assert_eq!(warnings_line(&["a".into(), "b".into(), "c".into()]).as_deref(), Some("a (и ещё 2)"));
    }

    // ------------------------------------------------------------ registry

    #[test]
    fn registry_ids_are_stable_unique_and_have_adapters() {
        let reg = tools();
        let ids: Vec<&str> = reg.iter().map(|t| t.id).collect();
        assert_eq!(ids, ["claude", "codex", "cursor", "copilot", "opencode"]);
        let names: Vec<&str> = reg.iter().map(|t| t.name).collect();
        assert_eq!(names, ["Claude Code", "OpenAI Codex", "Cursor", "GitHub Copilot", "OpenCode"]);
        for (i, t) in reg.iter().enumerate() {
            assert!(!t.detect.is_empty() && !t.name.is_empty());
            assert!(!t.detect.contains(&"AGENTS.md"), "{}: AGENTS.md общий, не признак", t.id);
            assert_eq!(t.tool.id(), t.id);
            assert_eq!(t.tool, ALL[i]);
            assert!(reg.iter().skip(i + 1).all(|o| o.id != t.id), "dup {}", t.id);
            if t.ready {
                let f = files(t.tool);
                assert!(!f.is_empty(), "{}", t.id);
                assert!(f.iter().all(|(p, c)| !p.starts_with('/') && !p.contains('\\') && !c.is_empty()));
            }
        }
        assert_eq!(files(Tool::Claude).len(), 7);
        assert_eq!(ready_tools().len(), 5);
        assert_eq!(ready_tools().len(), reg.iter().filter(|t| t.ready).count());
    }

    #[test]
    fn parse_tools_valid_all_and_dedupe() {
        assert_eq!(parse_tools("claude").unwrap(), vec![Tool::Claude]);
        assert_eq!(parse_tools(" Claude , claude ").unwrap(), vec![Tool::Claude]);
        assert_eq!(parse_tools("all").unwrap(), ALL.to_vec());
        assert_eq!(parse_tools("claude,all").unwrap(), ALL.to_vec());
        assert_eq!(parse_tools("claude,").unwrap(), vec![Tool::Claude]);
        // порядок реестра, не порядок ввода
        assert_eq!(parse_tools("opencode,CURSOR,codex,copilot,claude").unwrap(), ALL.to_vec());
        assert_eq!(parse_tools("copilot,codex").unwrap(), vec![Tool::Codex, Tool::Copilot]);
    }

    #[test]
    fn parse_tools_unknown_and_empty_give_russian_errors() {
        let e = parse_tools("claude,bogus").unwrap_err();
        assert!(e.contains("неизвестный инструмент «bogus»") && e.contains("Допустимо: claude, codex, cursor, copilot, opencode, all"), "{e}");
        assert!(parse_tools("").unwrap_err().contains("пустой список"));
        assert!(parse_tools(" , ").unwrap_err().contains("пустой список"));
    }

    #[test]
    fn detect_finds_tools_by_marker_paths() {
        let t = Tmp::new();
        assert!(detect(&t.0).is_empty());
        assert_eq!(preselect(&t.0), vec![Tool::Claude]);
        // общие файлы не считаются признаком конкретного инструмента
        fs::write(t.0.join("AGENTS.md"), "x").unwrap();
        fs::create_dir_all(t.0.join(".github/workflows")).unwrap();
        assert!(detect(&t.0).is_empty());

        // каждый маркер по отдельности находит ровно свой инструмент
        let markers: &[(Tool, &str, bool)] = &[
            (Tool::Claude, "CLAUDE.md", false),
            (Tool::Claude, ".claude", true),
            (Tool::Codex, ".codex", true),
            (Tool::Cursor, ".cursor", true),
            (Tool::Cursor, ".cursorrules", false),
            (Tool::Copilot, ".github/copilot-instructions.md", false),
            (Tool::Copilot, ".github/agents", true),
            (Tool::Copilot, ".github/instructions", true),
            (Tool::OpenCode, ".opencode", true),
            (Tool::OpenCode, "opencode.json", false),
        ];
        for (tool, m, is_dir) in markers {
            let d = Tmp::new();
            let p = d.0.join(m);
            if *is_dir {
                fs::create_dir_all(&p).unwrap();
            } else {
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(&p, "x").unwrap();
            }
            assert_eq!(detect(&d.0), vec![*tool], "{m}");
            assert_eq!(preselect(&d.0), vec![*tool], "{m}");
        }
        // несколько сразу, порядок реестра
        let d = Tmp::new();
        fs::create_dir_all(d.0.join(".opencode")).unwrap();
        fs::create_dir_all(d.0.join(".codex")).unwrap();
        assert_eq!(detect(&d.0), vec![Tool::Codex, Tool::OpenCode]);
    }

    #[test]
    fn install_for_selected_tools_only() {
        let t = Tmp::new();
        let r = install(&t.0, &[], false).unwrap();
        assert!(r.installed.is_empty() && r.skipped.is_empty() && r.per_tool.is_empty() && r.warnings.is_empty());
        assert!(!t.0.join(".claude").exists());

        let r = install(&t.0, &[Tool::Claude, Tool::Claude], false).unwrap();
        assert_eq!(r.per_tool.len(), 1);
        assert_eq!(r.per_tool[0].tool, Tool::Claude);
        assert_eq!(r.per_tool[0].installed.len(), 7);
        assert_eq!(r.installed, r.per_tool[0].installed);
        assert!(t.0.join(".claude/agents/pumba-worker.md").is_file());
        let r = install(&t.0, &[Tool::Claude], false).unwrap();
        assert!(r.per_tool[0].installed.is_empty() && r.per_tool[0].skipped.len() == 7);
    }

    #[test]
    fn ids_roundtrip_and_ignore_unknown() {
        assert_eq!(to_ids(&[Tool::Claude]), vec!["claude".to_string()]);
        assert_eq!(from_ids(&["claude".into(), "zzz".into()]), vec![Tool::Claude]);
        assert!(from_ids(&[]).is_empty());
        let all = to_ids(&ALL);
        assert_eq!(all, ["claude", "codex", "cursor", "copilot", "opencode"]);
        assert_eq!(from_ids(&["opencode".into(), "codex".into()]), vec![Tool::Codex, Tool::OpenCode]);
        assert_eq!(from_ids(&all), ALL.to_vec());
    }
}
