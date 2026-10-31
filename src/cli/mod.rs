//! `pumba <команда>` for agents and scripts: not interactive, no TUI. Arguments are parsed here by
//! hand (like in `main.rs`, no clap); the work is done by `pumba_core::api`, `read.rs` and
//! `write.rs` only print and map errors to exit codes. Contract: `templates/API-DESIGN.md`.
use pumba_core::api::{self, ApiErr, BodyMode};
use pumba_core::board::Board;
use std::path::PathBuf;

mod read;
mod write;

/// Board commands. They win over a folder name: a project folder with such a
/// name is opened as `pumba ./brief`.
pub const SUBCOMMANDS: &[&str] = &["brief", "move", "note", "rules", "unblocks", "column", "rules-hash"];

/// Text for `pumba --help`.
pub const USAGE: &str = "КОМАНДЫ ДЛЯ АГЕНТОВ (не интерактивные, TUI не запускают):
    pumba brief ID [--known ЧАСТИ] [--body head|full|none] [--json]
                                             всё для шага агента-драйвера по карточке
    pumba move ID КОЛОНКА|next [--expect-from КОЛОНКА] [--note \"ТЕКСТ\"] [--user-approved]
                                             перенести карточку в конец колонки
    pumba note ID \"ТЕКСТ\" [--user-approved]    дописать заметку под `## Ход работы`
    pumba rules [--part ID]... [--json]      правила проекта и свои правила
    pumba unblocks ID [--json]               какие карточки ждут эту
    pumba column КОЛОНКА [--json]            правило колонки: три части, agent, next
    pumba rules-hash [--parts] [--json]      отпечаток правил (12 символов)
    Общие: --root ПУТЬ (откуда искать .pumba/ вверх; по умолчанию текущая папка),
    --json (один JSON-объект), --at \"ТЕКСТ\" (метка времени заметки вместо текущего UTC).
    Имена команд главнее пути: папку с таким именем открывай как `pumba ./brief`.
    Коды выхода: 0 успех, 1 ввод-вывод, 2 аргументы, 3 нет карточки/колонки/next,
    4 карточка в корзине, 5 заблокирована зависимостями, 6 конфликт записи или
    --expect-from, 7 запрещено ручным режимом доски (нужен --user-approved),
    9 доски нет или board.json не читается.";

const HELP_BRIEF: &str = "pumba brief ID [--known ЧАСТИ] [--body head|full|none] [--json] [--root ПУТЬ]
    --known   части, которые агент уже видел: col:<id>=<хеш>, rules=<хеш> (через запятую);
              такие части не повторяются
    --body    head (по умолчанию, до 40 строк и последние заметки), full, none";
const HELP_MOVE: &str = "pumba move ID КОЛОНКА|next [--expect-from КОЛОНКА] [--note \"ТЕКСТ\"] [--user-approved] [--at \"ТЕКСТ\"] [--json] [--root ПУТЬ]
    next              колонка next текущей колонки карточки
    --expect-from     перенести, только если карточка сейчас в этой колонке (код 6 иначе)
    --note            дописать заметку «Откуда» → «Куда»: ТЕКСТ под `## Ход работы`
    --user-approved   только после явного «да» пользователя: нужен при ручном режиме доски";
const HELP_NOTE: &str = "pumba note ID \"ТЕКСТ\" [--user-approved] [--at \"ТЕКСТ\"] [--json] [--root ПУТЬ]
    Дописывает строку под `## Ход работы` (раздел создаётся в конце). Тот же текст, что у
    последней заметки, второй раз не пишется. Текст, начинающийся с «--», пиши после `--`.";
const HELP_RULES: &str = "pumba rules [--part ID]... [--json] [--root ПУТЬ]
    --part   id правила проекта (commits, tests, ...) или custom; можно несколько раз
             или через запятую; без --part выводятся все";
const HELP_UNBLOCKS: &str = "pumba unblocks ID [--json] [--root ПУТЬ]";
const HELP_COLUMN: &str = "pumba column КОЛОНКА [--json] [--root ПУТЬ]";
const HELP_HASH: &str = "pumba rules-hash [--parts] [--json] [--root ПУТЬ]
    --parts  ещё и по строке на часть: proto, cols, col:<id>, rule:<id>, custom";

/// What went wrong while reading the arguments.
#[derive(Debug, PartialEq)]
enum Usage {
    /// `-h`/`--help`: the text to print.
    Help(&'static str),
    Bad(String),
}

#[derive(Debug, Default, PartialEq)]
struct Common {
    root: Option<PathBuf>,
    json: bool,
}

#[derive(Debug, PartialEq)]
struct BriefArgs {
    c: Common,
    id: String,
    known: Vec<String>,
    body: BodyMode,
}

#[derive(Debug, PartialEq)]
struct MoveArgs {
    c: Common,
    id: String,
    to: String,
    expect_from: Option<String>,
    note: Option<String>,
    user_approved: bool,
    at: Option<String>,
}

#[derive(Debug, PartialEq)]
struct NoteArgs {
    c: Common,
    id: String,
    text: String,
    user_approved: bool,
    at: Option<String>,
}

#[derive(Debug, PartialEq)]
struct RulesArgs {
    c: Common,
    parts: Vec<String>,
}

#[derive(Debug, PartialEq)]
struct IdArgs {
    c: Common,
    id: String,
}

#[derive(Debug, PartialEq)]
struct HashArgs {
    c: Common,
    parts: bool,
}

// -------------------------------------------------------------- parsing

#[derive(Debug, Default)]
struct Parsed {
    pos: Vec<String>,
    vals: Vec<(String, String)>,
    flags: Vec<String>,
}

impl Parsed {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
    /// Last value of an option.
    fn val(&self, name: &str) -> Option<String> {
        self.vals.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }
    fn all(&self, name: &str) -> Vec<String> {
        self.vals.iter().filter(|(k, _)| k == name).map(|(_, v)| v.clone()).collect()
    }
}

/// `--name`, `--name value`, `--name=value`, `--` (the rest is positional). A word that starts
/// with one `-` and is not `-h` is text, not an option (`-5`, `- пункт`).
fn parse(args: &[String], help: &'static str, vals: &[&str], flags: &[&str]) -> Result<Parsed, Usage> {
    let mut p = Parsed::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            p.pos.extend(it.by_ref().cloned());
            break;
        }
        if a == "-h" || a == "--help" {
            return Err(Usage::Help(help));
        }
        let Some(name) = a.strip_prefix("--").filter(|n| !n.is_empty()) else {
            p.pos.push(a.clone());
            continue;
        };
        let (name, inline) = match name.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (name, None),
        };
        if vals.contains(&name) {
            let v = match inline {
                Some(v) => v,
                None => it.next().cloned().ok_or_else(|| Usage::Bad(format!("после --{name} нужно значение")))?,
            };
            p.vals.push((name.to_string(), v));
        } else if flags.contains(&name) && inline.is_none() {
            p.flags.push(name.to_string());
        } else {
            return Err(Usage::Bad(format!("неизвестная опция: --{name}")));
        }
    }
    Ok(p)
}

fn common(p: &Parsed) -> Common {
    Common { root: p.val("root").map(PathBuf::from), json: p.flag("json") }
}

/// Exactly the positional arguments `names`.
fn positional(p: &Parsed, names: &[&str]) -> Result<Vec<String>, Usage> {
    if p.pos.len() < names.len() {
        return Err(Usage::Bad(format!("не хватает аргумента: {}", names[p.pos.len()])));
    }
    if p.pos.len() > names.len() {
        return Err(Usage::Bad(format!("лишний аргумент: {}", p.pos[names.len()])));
    }
    Ok(p.pos.clone())
}

/// Words split by commas and spaces; empty ones dropped.
fn words(vals: Vec<String>) -> Vec<String> {
    vals.iter().flat_map(|v| v.split(|c: char| c == ',' || c.is_whitespace())).filter(|w| !w.is_empty()).map(String::from).collect()
}

fn parse_brief(args: &[String]) -> Result<BriefArgs, Usage> {
    let p = parse(args, HELP_BRIEF, &["root", "known", "body"], &["json"])?;
    let pos = positional(&p, &["ID"])?;
    let body = match p.val("body").as_deref() {
        None | Some("head") => BodyMode::Head,
        Some("full") => BodyMode::Full,
        Some("none") => BodyMode::None,
        Some(o) => return Err(Usage::Bad(format!("--body: head, full или none, а не «{o}»"))),
    };
    Ok(BriefArgs { c: common(&p), id: pos[0].clone(), known: words(p.all("known")), body })
}

fn parse_move(args: &[String]) -> Result<MoveArgs, Usage> {
    let p = parse(args, HELP_MOVE, &["root", "expect-from", "note", "at"], &["json", "user-approved"])?;
    let pos = positional(&p, &["ID", "КОЛОНКА или next"])?;
    Ok(MoveArgs {
        c: common(&p),
        id: pos[0].clone(),
        to: pos[1].clone(),
        expect_from: p.val("expect-from"),
        note: p.val("note"),
        user_approved: p.flag("user-approved"),
        at: p.val("at"),
    })
}

fn parse_note(args: &[String]) -> Result<NoteArgs, Usage> {
    let p = parse(args, HELP_NOTE, &["root", "at"], &["json", "user-approved"])?;
    let pos = match positional(&p, &["ID", "ТЕКСТ"]) {
        Err(Usage::Bad(m)) if m.starts_with("лишний аргумент") => {
            return Err(Usage::Bad(format!("{m} (текст заметки одним аргументом, в кавычках)")));
        }
        r => r?,
    };
    Ok(NoteArgs { c: common(&p), id: pos[0].clone(), text: pos[1].clone(), user_approved: p.flag("user-approved"), at: p.val("at") })
}

fn parse_rules(args: &[String]) -> Result<RulesArgs, Usage> {
    let p = parse(args, HELP_RULES, &["root", "part"], &["json"])?;
    positional(&p, &[])?;
    Ok(RulesArgs { c: common(&p), parts: words(p.all("part")) })
}

fn parse_id(args: &[String], help: &'static str, what: &str) -> Result<IdArgs, Usage> {
    let p = parse(args, help, &["root"], &["json"])?;
    let pos = positional(&p, &[what])?;
    Ok(IdArgs { c: common(&p), id: pos[0].clone() })
}

fn parse_hash(args: &[String]) -> Result<HashArgs, Usage> {
    let p = parse(args, HELP_HASH, &["root"], &["json", "parts"])?;
    positional(&p, &[])?;
    Ok(HashArgs { c: common(&p), parts: p.flag("parts") })
}

// ------------------------------------------------------------- dispatch

/// Run the command if `args[0]` is one of `SUBCOMMANDS` (None: not ours, `main` goes on).
pub fn run(args: &[String]) -> Option<i32> {
    let (cmd, rest) = args.split_first()?;
    if !SUBCOMMANDS.contains(&cmd.as_str()) {
        return None;
    }
    let r = match cmd.as_str() {
        "brief" => parse_brief(rest).map(read::brief),
        "move" => parse_move(rest).map(write::move_cmd),
        "note" => parse_note(rest).map(write::note_cmd),
        "rules" => parse_rules(rest).map(read::rules),
        "unblocks" => parse_id(rest, HELP_UNBLOCKS, "ID").map(read::unblocks),
        "column" => parse_id(rest, HELP_COLUMN, "КОЛОНКА").map(read::column),
        _ => parse_hash(rest).map(read::rules_hash),
    };
    Some(match r {
        Ok(code) => code,
        Err(Usage::Help(text)) => {
            println!("{text}");
            0
        }
        Err(Usage::Bad(m)) => {
            eprintln!("pumba: {m} ({})\nСправка: pumba {cmd} --help", api::code::ARGS);
            api::code::ARGS
        }
    })
}

// -------------------------------------------------------------- helpers

/// The board of the current folder (or of `--root`); on failure the error is already printed.
fn open(c: &Common) -> Result<Board, i32> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = c.root.as_ref().map(|r| std::path::absolute(r).unwrap_or_else(|_| r.clone()));
    api::open_board(root.as_deref(), &cwd).map_err(|e| fail(c, &e))
}

/// Print an error the same way for every command and return its exit code. Extra lines go to
/// stdout (similar ids, columns, blockers, the phrase for the user), the message to stderr; with
/// `--json` stdout holds one object instead.
fn fail(c: &Common, e: &ApiErr) -> i32 {
    if c.json {
        println!("{}", api::to_json(&ErrOut { ok: false, code: e.code, error: &e.msg, details: &e.out }));
    } else {
        for l in &e.out {
            println!("{l}");
        }
    }
    eprintln!("pumba: {} ({})", e.msg, e.code);
    e.code
}

#[derive(serde::Serialize)]
struct ErrOut<'a> {
    ok: bool,
    code: i32,
    error: &'a str,
    details: &'a [String],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn bad(r: Result<impl std::fmt::Debug, Usage>) -> String {
        match r {
            Err(Usage::Bad(m)) => m,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn brief_arguments() {
        let b = parse_brief(&a(&["T-1"])).unwrap();
        assert_eq!((b.id.as_str(), b.body, b.known.len(), b.c.json, b.c.root.is_none()), ("T-1", BodyMode::Head, 0, false, true));
        let b = parse_brief(&a(&["--json", "T-1", "--body=full", "--known", "col:review=abcd1234, rules=ffff0000", "--known", "col:x=1111", "--root", "d"])).unwrap();
        assert_eq!((b.body, b.c.json, b.c.root), (BodyMode::Full, true, Some(PathBuf::from("d"))));
        assert_eq!(b.known, ["col:review=abcd1234", "rules=ffff0000", "col:x=1111"]);
        assert_eq!(parse_brief(&a(&["T-1", "--body", "none"])).unwrap().body, BodyMode::None);
        assert!(bad(parse_brief(&a(&["T-1", "--body", "x"]))).contains("head, full или none"));
        assert!(bad(parse_brief(&a(&[]))).contains("не хватает аргумента: ID"));
        assert!(bad(parse_brief(&a(&["a", "b"]))).contains("лишний аргумент: b"));
        assert!(bad(parse_brief(&a(&["T-1", "--wat"]))).contains("неизвестная опция: --wat"));
        assert!(bad(parse_brief(&a(&["T-1", "--known"]))).contains("нужно значение"));
        assert!(bad(parse_brief(&a(&["T-1", "--json=1"]))).contains("неизвестная опция"));
        assert!(matches!(parse_brief(&a(&["-h"])), Err(Usage::Help(t)) if t.starts_with("pumba brief")));
    }

    #[test]
    fn move_arguments() {
        let m = parse_move(&a(&["T-1", "next", "--expect-from", "review", "--note", "ок", "--user-approved", "--at=вчера"])).unwrap();
        assert_eq!((m.id.as_str(), m.to.as_str()), ("T-1", "next"));
        assert_eq!((m.expect_from.as_deref(), m.note.as_deref(), m.user_approved, m.at.as_deref()), (Some("review"), Some("ок"), true, Some("вчера")));
        let m = parse_move(&a(&["T-1", "done"])).unwrap();
        assert!(!m.user_approved && m.note.is_none() && m.expect_from.is_none());
        assert!(bad(parse_move(&a(&["T-1"]))).contains("КОЛОНКА или next"));
        // a note value may look like an option
        assert_eq!(parse_move(&a(&["T-1", "done", "--note", "--важно"])).unwrap().note.as_deref(), Some("--важно"));
    }

    #[test]
    fn note_arguments() {
        let n = parse_note(&a(&["T-1", "готово: тесты зелёные"])).unwrap();
        assert_eq!((n.id.as_str(), n.text.as_str()), ("T-1", "готово: тесты зелёные"));
        // text that starts with a dash: a single dash is text, `--` ends the options
        assert_eq!(parse_note(&a(&["T-1", "- пункт"])).unwrap().text, "- пункт");
        assert_eq!(parse_note(&a(&["T-1", "-5"])).unwrap().text, "-5");
        assert_eq!(parse_note(&a(&["T-1", "--", "--важно"])).unwrap().text, "--важно");
        assert!(bad(parse_note(&a(&["T-1", "два", "слова"]))).contains("в кавычках"));
        assert!(bad(parse_note(&a(&["T-1"]))).contains("ТЕКСТ"));
    }

    #[test]
    fn other_arguments() {
        let r = parse_rules(&a(&["--part", "commits,tests", "--part", "custom", "--json"])).unwrap();
        assert_eq!((r.parts, r.c.json), (vec!["commits".to_string(), "tests".into(), "custom".into()], true));
        assert!(parse_rules(&a(&[])).unwrap().parts.is_empty());
        assert!(bad(parse_rules(&a(&["x"]))).contains("лишний аргумент"));
        assert_eq!(parse_id(&a(&["T-2"]), HELP_UNBLOCKS, "ID").unwrap().id, "T-2");
        assert!(bad(parse_id(&a(&[]), HELP_COLUMN, "КОЛОНКА")).contains("КОЛОНКА"));
        assert!(parse_hash(&a(&["--parts"])).unwrap().parts);
        assert!(!parse_hash(&a(&[])).unwrap().parts);
    }

    #[test]
    fn only_listed_subcommands_are_taken() {
        assert!(run(&a(&[])).is_none());
        assert!(run(&a(&["./brief"])).is_none());
        assert!(run(&a(&["some-folder"])).is_none());
        assert!(run(&a(&["init"])).is_none(), "init is handled by main");
        // every command is in the help text
        for c in SUBCOMMANDS {
            assert!(USAGE.contains(&format!("pumba {c}")), "{c}");
        }
    }
}
