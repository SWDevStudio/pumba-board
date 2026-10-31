use pumba_core::model;
use pumba_install::install;
use pumba_tui::{app, ui};

use install::{Tool, ToolInfo};
use model::Store;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind};
use ratatui::crossterm::execute;
use std::io::{stdout, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "pumba: канбан-доска для задач ИИ-агентов

ИСПОЛЬЗОВАНИЕ:
    pumba [ПУТЬ]                             открыть доску (TUI)
    pumba init [--tools СПИСОК | --no-agents] [ПУТЬ]
                                             создать .pumba и поставить агентов и скиллы
    pumba install [--tools СПИСОК] [--force] [ПУТЬ]
                                             поставить агентов и скиллы для выбранных
                                             ИИ-инструментов
    pumba install-claude [--force] [ПУТЬ]
                                             то же, что install --tools claude

АРГУМЕНТЫ:
    ПУТЬ    каталог проекта (по умолчанию текущий). Доска лежит в ПУТЬ/.pumba/
            (ищется вверх от ПУТИ).

КОМАНДЫ:
    init            создаёт .pumba, если его нет, и ставит пакет шаблонов
                    (существующие файлы не трогает)
    install         ставит пакет в каталоги выбранных инструментов; существующие
                    файлы пропускает
    install-claude  псевдоним install --tools claude

ВЫБОР ИНСТРУМЕНТОВ:
    Без --tools в терминале появится вопрос со списком: найденные в проекте
    инструменты отмечены заранее (если таких нет, то Claude Code). Ответ: номера
    через запятую (например 1,3), пустая строка = отмеченные, 0 = никого.
    Без терминала берётся список из .pumba/board.json (tools), иначе найденные
    в проекте, иначе Claude Code. Выбор записывается в board.json.
    Готовы: claude, codex, cursor, copilot, opencode. Если выбраны инструменты,
    которые читают папки друг друга, будет предупреждение о возможных дублях скиллов.

ОПЦИИ:
        --tools СПИСОК  id через запятую: claude, codex, cursor, copilot, opencode
                        или all (например claude,cursor)
        --no-agents     (init) создать только .pumba/, без агентов и скиллов
        --force         (install) перезаписать существующие файлы
    -h, --help          показать эту справку
    -V, --version       показать версию";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("install-claude") => std::process::exit(cmd_install(&args[1..], true)),
        Some("install") => std::process::exit(cmd_install(&args[1..], false)),
        Some("init") => std::process::exit(cmd_init(&args[1..])),
        _ => {}
    }
    let mut start: Option<PathBuf> = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            "-V" | "--version" => {
                println!("pumba {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            s if s.starts_with('-') => {
                eprintln!("неизвестная опция: {s}
{HINT}");
                std::process::exit(2);
            }
            _ => start = Some(PathBuf::from(a)),
        }
    }
    let start = match start {
        Some(p) => p,
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let start = std::fs::canonicalize(&start).unwrap_or(start);
    let (root, exists) = match Store::find(&start) {
        Some(r) => (r, true),
        None => (start, false),
    };

    let mut app = app::App::new(root, exists);
    let mut terminal = ratatui::init();
    let _ = execute!(stdout(), EnableBracketedPaste);

    let res = run(&mut terminal, &mut app);

    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    if let Err(e) = res {
        eprintln!("pumba: {e}");
        std::process::exit(1);
    }
}

const HINT: &str = "Справка: pumba --help";

// ------------------------------------------------------------ arguments

/// Что сказано про инструменты в командной строке.
#[derive(Debug, PartialEq)]
enum ToolsArg {
    /// Не указано: спросить (терминал) или взять сохранённые/найденные.
    Ask,
    /// `--no-agents`.
    Skip,
    /// `--tools ...`.
    List(Vec<Tool>),
}

#[derive(Debug, PartialEq)]
struct SubOpts {
    force: bool,
    tools: ToolsArg,
    path: Option<PathBuf>,
}

#[derive(Debug, PartialEq)]
enum ParseErr {
    Help,
    Bad(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sub {
    Init,
    Install,
}

/// Разбор аргументов `init` / `install` (чистая функция, без ввода-вывода).
fn parse_sub(sub: Sub, args: &[String]) -> Result<SubOpts, ParseErr> {
    let mut o = SubOpts { force: false, tools: ToolsArg::Ask, path: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let a = a.as_str();
        let tools_val = if a == "--tools" {
            Some(it.next().map(String::as_str).ok_or_else(|| ParseErr::Bad("после --tools нужен список, например --tools claude".into()))?)
        } else {
            a.strip_prefix("--tools=")
        };
        if let Some(v) = tools_val {
            if o.tools != ToolsArg::Ask {
                return Err(ParseErr::Bad("--tools и --no-agents можно указать только один раз".into()));
            }
            o.tools = ToolsArg::List(install::parse_tools(v).map_err(ParseErr::Bad)?);
            continue;
        }
        match a {
            "-h" | "--help" => return Err(ParseErr::Help),
            "--force" if sub == Sub::Install => o.force = true,
            "--no-agents" if sub == Sub::Init => {
                if o.tools != ToolsArg::Ask {
                    return Err(ParseErr::Bad("--tools и --no-agents можно указать только один раз".into()));
                }
                o.tools = ToolsArg::Skip;
            }
            s if s.starts_with('-') => return Err(ParseErr::Bad(format!("неизвестная опция: {s}"))),
            _ if o.path.is_some() => return Err(ParseErr::Bad(format!("лишний аргумент: {a}"))),
            _ => o.path = Some(PathBuf::from(a)),
        }
    }
    Ok(o)
}

fn parse_or_exit(sub: Sub, args: &[String]) -> Result<SubOpts, i32> {
    parse_sub(sub, args).map_err(|e| match e {
        ParseErr::Help => {
            println!("{USAGE}");
            0
        }
        ParseErr::Bad(m) => {
            eprintln!("pumba: {m}
{HINT}");
            2
        }
    })
}

/// canonicalize без префикса `\\?\` (Windows), чтобы пути в выводе читались.
fn plain_canonical(p: PathBuf) -> PathBuf {
    match std::fs::canonicalize(&p) {
        Ok(c) => match c.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
            Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
            _ => c,
        },
        Err(_) => p,
    }
}

fn start_dir(path: Option<PathBuf>) -> PathBuf {
    let p = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    plain_canonical(p)
}

// ------------------------------------------------------ tool selection

/// Без терминала: сохранённые в board.json, иначе найденные в проекте, иначе Claude Code.
fn fallback_tools(saved: &[String], root: &Path) -> Vec<Tool> {
    let s = install::from_ids(saved);
    if s.is_empty() { install::preselect(root) } else { s }
}

/// Разбор ответа на вопрос: номера через запятую или пробел, пусто = `pre`, `0` = никого.
fn parse_choice(answer: &str, ready: &[ToolInfo], pre: &[Tool]) -> Result<Vec<Tool>, String> {
    let a = answer.trim();
    if a.is_empty() {
        return Ok(pre.to_vec());
    }
    let mut nums: Vec<usize> = Vec::new();
    for tok in a.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()) {
        nums.push(tok.parse::<usize>().map_err(|_| format!("не понял «{tok}»: нужны номера, например 1,3"))?);
    }
    if nums.contains(&0) {
        return if nums.iter().all(|&n| n == 0) { Ok(Vec::new()) } else { Err("0 нельзя сочетать с другими номерами".into()) };
    }
    if let Some(&bad) = nums.iter().find(|&&n| n > ready.len()) {
        return Err(format!("нет пункта {bad}, есть 1-{}", ready.len()));
    }
    Ok(ready.iter().enumerate().filter(|(i, _)| nums.contains(&(i + 1))).map(|(_, t)| t.tool).collect())
}

/// Вопрос в терминале. На конце ввода берёт отмеченное по умолчанию.
fn prompt_tools(root: &Path, input: &mut impl BufRead, out: &mut impl Write) -> Vec<Tool> {
    let ready = install::ready_tools();
    let pre = install::preselect(root);
    let found = install::detect(root);
    let pre_nums: Vec<String> =
        ready.iter().enumerate().filter(|(_, t)| pre.contains(&t.tool)).map(|(i, _)| (i + 1).to_string()).collect();
    loop {
        let _ = writeln!(out, "Для каких инструментов установить агентов и скиллы?");
        for (i, t) in ready.iter().enumerate() {
            let mark = if pre.contains(&t.tool) { "x" } else { " " };
            let note = if found.contains(&t.tool) { "  (найден в проекте)" } else { "" };
            let _ = writeln!(out, "  {}) [{mark}] {}{note}", i + 1, t.name);
        }
        let _ = write!(out, "Номера через запятую (например 1,3), Enter = [{}], 0 = никого: ", pre_nums.join(","));
        let _ = out.flush();
        let mut line = String::new();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            let _ = writeln!(out);
            return pre;
        }
        match parse_choice(&line, &ready, &pre) {
            Ok(v) => return v,
            Err(e) => {
                let _ = writeln!(out, "{e}");
            }
        }
    }
}

fn choose_tools(arg: &ToolsArg, root: &Path, saved: &[String]) -> Vec<Tool> {
    match arg {
        ToolsArg::Skip => Vec::new(),
        ToolsArg::List(l) => l.clone(),
        ToolsArg::Ask => {
            if std::io::stdin().is_terminal() {
                prompt_tools(root, &mut std::io::stdin().lock(), &mut stdout())
            } else {
                fallback_tools(saved, root)
            }
        }
    }
}

// ------------------------------------------------------------- commands

fn print_report(root: &Path, tools: &[Tool], force: bool) -> i32 {
    if tools.is_empty() {
        println!("Агенты и скиллы не устанавливались (корень: {})", root.display());
        return 0;
    }
    match install::install(root, tools, force) {
        Ok(r) => {
            for t in &r.per_tool {
                println!("{}:", t.tool.info().name);
                for f in &t.installed {
                    println!("  установлен: {f}");
                }
                for f in &t.skipped {
                    println!("  пропущен:   {f}");
                }
            }
            println!(
                "Готово: установлено {}, пропущено {} (корень: {})",
                r.installed.len(),
                r.skipped.len(),
                root.display()
            );
            for w in &r.warnings {
                println!("Внимание: {w}");
            }
            0
        }
        Err(e) => {
            eprintln!("pumba: {e}");
            1
        }
    }
}

/// Запоминает выбор в board.json; сбой только предупреждение (файлы уже поставлены).
fn record_tools(root: &Path, tools: &[Tool]) {
    if let Err(e) = Store::new(root).set_tools(&install::to_ids(tools)) {
        eprintln!("pumba: не удалось записать tools в board.json: {e}");
    }
}

fn cmd_install(args: &[String], alias_claude: bool) -> i32 {
    let o = match parse_or_exit(Sub::Install, args) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let start = start_dir(o.path);
    let root = Store::find(&start).unwrap_or(start);
    let store = Store::new(&root);
    let arg = if alias_claude && o.tools == ToolsArg::Ask { ToolsArg::List(vec![Tool::Claude]) } else { o.tools };
    let tools = choose_tools(&arg, &root, &store.saved_tools());
    let code = print_report(&root, &tools, o.force);
    if code == 0 && Store::find(&root).is_some() {
        record_tools(&root, &tools);
    }
    code
}

fn cmd_init(args: &[String]) -> i32 {
    let o = match parse_or_exit(Sub::Init, args) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let start = start_dir(o.path);
    let (root, saved) = match Store::find(&start) {
        Some(r) => {
            println!("Уже инициализировано: {}", r.join(".pumba").display());
            let saved = Store::new(&r).saved_tools();
            (r, saved)
        }
        None => {
            if let Err(e) = Store::new(&start).init() {
                eprintln!("pumba: не удалось создать {}: {e}", start.join(".pumba").display());
                return 1;
            }
            println!("Создано: {}", start.join(".pumba").display());
            (start, Vec::new())
        }
    };
    let tools = choose_tools(&o.tools, &root, &saved);
    let code = print_report(&root, &tools, false);
    if code == 0 {
        record_tools(&root, &tools);
    }
    code
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App) -> std::io::Result<()> {
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(400))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.on_key(k),
                Event::Paste(s) => app.on_paste(s),
                _ => {}
            }
        }
        app.tick();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_every_ready_tool_id() {
        let ready_line = USAGE.lines().find(|l| l.contains("Готовы:")).unwrap();
        assert!(install::ready_tools().iter().all(|t| ready_line.contains(t.id)), "{ready_line}");
        let opt_line = USAGE.lines().find(|l| l.trim_start().starts_with("--tools СПИСОК")).unwrap();
        assert!(install::ready_tools().iter().all(|t| opt_line.contains(t.id)), "{opt_line}");
    }

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_sub_init_variants() {
        let o = parse_sub(Sub::Init, &a(&[])).unwrap();
        assert_eq!(o, SubOpts { force: false, tools: ToolsArg::Ask, path: None });
        let o = parse_sub(Sub::Init, &a(&["--tools", "claude", "proj"])).unwrap();
        assert_eq!(o.tools, ToolsArg::List(vec![Tool::Claude]));
        assert_eq!(o.path, Some(PathBuf::from("proj")));
        let o = parse_sub(Sub::Init, &a(&["proj", "--tools=all"])).unwrap();
        assert_eq!(o.tools, ToolsArg::List(vec![Tool::Claude, Tool::Codex, Tool::Cursor, Tool::Copilot, Tool::OpenCode]));
        let o = parse_sub(Sub::Init, &a(&["--tools", "opencode,codex"])).unwrap();
        assert_eq!(o.tools, ToolsArg::List(vec![Tool::Codex, Tool::OpenCode]));
        assert_eq!(parse_sub(Sub::Init, &a(&["--no-agents"])).unwrap().tools, ToolsArg::Skip);
    }

    #[test]
    fn parse_sub_install_variants() {
        let o = parse_sub(Sub::Install, &a(&["--force", "--tools", "claude"])).unwrap();
        assert!(o.force);
        assert_eq!(o.tools, ToolsArg::List(vec![Tool::Claude]));
        assert_eq!(parse_sub(Sub::Install, &a(&["-h"])), Err(ParseErr::Help));
    }

    #[test]
    fn parse_sub_errors() {
        let bad = |sub, v: &[&str]| match parse_sub(sub, &a(v)) {
            Err(ParseErr::Bad(m)) => m,
            other => panic!("{other:?}"),
        };
        assert!(bad(Sub::Install, &["--tools", "bogus"]).contains("неизвестный инструмент «bogus»"));
        assert!(bad(Sub::Install, &["--tools"]).contains("нужен список"));
        assert!(bad(Sub::Install, &["--no-agents"]).contains("неизвестная опция"));
        assert!(bad(Sub::Init, &["--force"]).contains("неизвестная опция"));
        assert!(bad(Sub::Init, &["--no-agents", "--tools", "claude"]).contains("только один раз"));
        assert!(bad(Sub::Init, &["--tools", "claude", "--no-agents"]).contains("только один раз"));
        assert!(bad(Sub::Init, &["a", "b"]).contains("лишний аргумент: b"));
    }

    #[test]
    fn fallback_prefers_saved_then_detected_then_claude() {
        let root = std::env::temp_dir().join(format!("pumba-main-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(fallback_tools(&[], &root), vec![Tool::Claude]);
        assert_eq!(fallback_tools(&a(&["claude"]), &root), vec![Tool::Claude]);
        assert_eq!(fallback_tools(&a(&["zzz"]), &root), vec![Tool::Claude]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_choice_answers() {
        let ready = install::ready_tools();
        let pre = vec![Tool::Claude];
        assert_eq!(parse_choice("", &ready, &pre).unwrap(), pre);
        assert_eq!(parse_choice("  \n", &ready, &pre).unwrap(), pre);
        assert_eq!(parse_choice("1", &ready, &pre).unwrap(), vec![Tool::Claude]);
        assert_eq!(parse_choice("1,1", &ready, &pre).unwrap(), vec![Tool::Claude]);
        assert_eq!(parse_choice("0", &ready, &pre).unwrap(), Vec::<Tool>::new());
        assert_eq!(parse_choice("1,3", &ready, &pre).unwrap(), vec![Tool::Claude, Tool::Cursor]);
        assert_eq!(parse_choice("5 2", &ready, &pre).unwrap(), vec![Tool::Codex, Tool::OpenCode]);
        assert_eq!(parse_choice("1,2,3,4,5", &ready, &pre).unwrap().len(), 5);
        assert!(parse_choice("6", &ready, &pre).unwrap_err().contains("нет пункта 6, есть 1-5"));
        assert!(parse_choice("x", &ready, &pre).unwrap_err().contains("не понял"));
        assert!(parse_choice("0,1", &ready, &pre).unwrap_err().contains("0 нельзя"));
    }

    #[test]
    fn prompt_reads_answers_and_retries() {
        let root = std::env::temp_dir().join(format!("pumba-main-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        let mut out = Vec::new();
        let got = prompt_tools(&root, &mut "9\n\n".as_bytes(), &mut out);
        assert_eq!(got, vec![Tool::Claude]);
        let text = String::from_utf8(out).unwrap();
        println!("{text}");
        assert!(text.contains("1) [x] Claude Code  (найден в проекте)") && text.contains("нет пункта 9, есть 1-5"));
        for line in ["2) [ ] OpenAI Codex", "3) [ ] Cursor", "4) [ ] GitHub Copilot", "5) [ ] OpenCode"] {
            assert!(text.contains(line), "{line}");
        }
        assert!(text.contains("Enter = [1]") && !text.contains("6)"));
        // other tools found: they are marked and preselected together
        std::fs::create_dir_all(root.join(".cursor")).unwrap();
        std::fs::write(root.join("opencode.json"), "{}").unwrap();
        let mut out2 = Vec::new();
        assert_eq!(prompt_tools(&root, &mut "
".as_bytes(), &mut out2), vec![Tool::Claude, Tool::Cursor, Tool::OpenCode]);
        let text2 = String::from_utf8(out2).unwrap();
        assert!(text2.contains("3) [x] Cursor  (найден в проекте)") && text2.contains("5) [x] OpenCode  (найден в проекте)"));
        assert!(text2.contains("Enter = [1,3,5]"), "{text2}");
        assert_eq!(prompt_tools(&root, &mut "2,4
".as_bytes(), &mut Vec::new()), vec![Tool::Codex, Tool::Copilot]);
        let _ = std::fs::remove_file(root.join("opencode.json"));
        let _ = std::fs::remove_dir_all(root.join(".cursor"));
        assert_eq!(prompt_tools(&root, &mut "0\n".as_bytes(), &mut Vec::new()), Vec::<Tool>::new());
        // end of input = preselected
        assert_eq!(prompt_tools(&root, &mut "".as_bytes(), &mut Vec::new()), vec![Tool::Claude]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
