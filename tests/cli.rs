//! The `pumba` command line for agents, end to end: the built binary on a board in a temp folder.
//! Times are given with `--at`, so nothing here depends on the clock.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Out {
    code: i32,
    out: String,
    err: String,
}

fn pumba(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_pumba"))
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .expect("run pumba");
    Out { code: o.status.code().unwrap(), out: String::from_utf8_lossy(&o.stdout).into_owned(), err: String::from_utf8_lossy(&o.stderr).into_owned() }
}

/// A project folder with an initialised board (no agents installed).
fn project() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("pumba-cli-{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    let o = pumba(&dir, &["init", "--no-agents"]);
    assert_eq!(o.code, 0, "{}{}", o.out, o.err);
    assert!(dir.join(".pumba/board.json").is_file());
    dir
}

fn card(dir: &Path, id: &str, column: &str, extra: &str, body: &str) {
    let kind = if id.starts_with('E') { "epic" } else { "task" };
    let md = format!("---\nid: {id}\ntype: {kind}\ntitle: Карточка {id}\ncolumn: {column}\n{extra}hidden: false\norder: 0\n---\n\n{body}\n");
    std::fs::write(dir.join(".pumba/tasks").join(format!("{id}.md")), md).unwrap();
}

fn read_card(dir: &Path, id: &str) -> String {
    std::fs::read_to_string(dir.join(".pumba/tasks").join(format!("{id}.md"))).unwrap()
}

fn column_of(dir: &Path, id: &str) -> String {
    read_card(dir, id).lines().find_map(|l| l.strip_prefix("column: ")).unwrap().to_string()
}

fn json(o: &Out) -> serde_json::Value {
    serde_json::from_str(o.out.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {}", o.out))
}

/// Set `rules[id].value` in board.json (like the TUI would).
fn set_rule(dir: &Path, id: &str, value: &str) {
    let p = dir.join(".pumba/board.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    for r in v["rules"].as_array_mut().unwrap() {
        if r["id"] == id {
            r["value"] = value.into();
        }
    }
    std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

const AT: &str = "2026-10-31 20:41 UTC";

// ---------------------------------------------------------------- brief

#[test]
fn brief_text_and_json() {
    let d = project();
    card(&d, "E-001", "in-progress", "", "Эпик.");
    card(&d, "T-004", "done", "", "готово");
    card(&d, "T-001", "in-progress", "parent: E-001\nsprint: 1\ndepends: T-004\n", "Сделать форму.\nВторая строка.");
    card(&d, "T-002", "backlog", "depends: T-001\n", "ждёт");

    let o = pumba(&d, &["brief", "T-001"]);
    assert_eq!(o.code, 0, "{}", o.err);
    assert!(o.err.is_empty(), "{}", o.err);
    let lines: Vec<&str> = o.out.lines().collect();
    assert_eq!(lines[0], "OK T-001 task «Карточка T-001» sprint=1 model=default effort=default api=1");
    assert_eq!(lines[1], "col=in-progress «В работе» (2/5) agent=pumba-worker next=review «На ревью» last=done");
    assert_eq!(lines[2], "parent=E-001 «Карточка E-001» in-progress | depends: T-004 done ✓ | dependents: T-002");
    assert!(lines[3].starts_with("board=auto ") && lines[3].contains("tests=required"), "{}", lines[3]);
    assert!(lines[4].starts_with("hash rules=") && lines[4].contains(" col:in-progress="), "{}", lines[4]);
    assert_eq!(lines[5], "log: -; visits=0");
    for h in ["## Перед работой", "## Работа", "## После работы", "## Тело (2 строк)", "Сделать форму."] {
        assert!(o.out.contains(h), "{h}\n{}", o.out);
    }

    // lowercase and short ids are accepted
    assert_eq!(pumba(&d, &["brief", "t1"]).out, o.out);
    // --root works from another folder, also from a subfolder of the board
    let other = std::env::temp_dir();
    assert_eq!(pumba(&other, &["brief", "T-001", "--root", d.to_str().unwrap()]).out, o.out);
    std::fs::create_dir_all(d.join("sub/dir")).unwrap();
    assert_eq!(pumba(&d.join("sub/dir"), &["brief", "T-001"]).out, o.out);

    // --known: the same column hash is not repeated
    let hash = lines[4].split(" col:in-progress=").nth(1).unwrap().to_string();
    let k = pumba(&d, &["brief", "T-001", "--known", &format!("col:in-progress={hash}")]);
    assert_eq!(k.code, 0);
    assert!(k.out.contains("known: col:in-progress") && !k.out.contains("## Работа"), "{}", k.out);
    assert!(k.out.len() * 2 < o.out.len(), "{} vs {}", k.out.len(), o.out.len());
    // body none
    assert!(!pumba(&d, &["brief", "T-001", "--body", "none"]).out.contains("## Тело"));

    // JSON: one valid object with the same data
    let j = json(&pumba(&d, &["brief", "T-001", "--json"]));
    assert_eq!((j["status"].as_str(), j["exit"].as_i64(), j["id"].as_str()), (Some("OK"), Some(0), Some("T-001")));
    assert_eq!(j["column"]["next"]["id"], "review");
    assert_eq!(j["depends"][0]["done"], true);
    assert!(j["parts"]["before"].as_str().unwrap().contains("В РАБОТЕ"));
    assert_eq!(j["body"]["text"], "Сделать форму.\nВторая строка.");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn brief_size_without_column_text_is_small() {
    let d = project();
    card(&d, "T-001", "in-progress", "sprint: 1\n", "Коротко.");
    let o = pumba(&d, &["brief", "T-001"]);
    let j = json(&pumba(&d, &["brief", "T-001", "--json"]));
    let col: usize = ["before", "rule", "after"].iter().map(|k| j["parts"][k].as_str().unwrap().len()).sum();
    // about 400 tokens at 3.3 bytes per token; headers of the three parts are counted as the rest
    let rest = o.out.len() - col;
    assert!(rest < 1320, "{rest} bytes without the column text:\n{}", o.out);
}

#[test]
fn brief_statuses_and_codes() {
    let d = project();
    card(&d, "T-001", "in-progress", "", "x");
    card(&d, "T-002", "backlog", "depends: T-001\n", "x");
    card(&d, "T-003", "backlog", "deleted: true\n", "x");

    let o = pumba(&d, &["brief", "T-002"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.out.lines().next().unwrap(), "BLOCKED T-001 «В работе» (in-progress)");
    assert!(o.err.contains("(5)"), "{}", o.err);
    assert_eq!(json(&pumba(&d, &["brief", "T-002", "--json"])).get("status").unwrap(), "BLOCKED");

    let o = pumba(&d, &["brief", "T-003"]);
    assert_eq!((o.code, o.out.trim()), (4, "DELETED T-003 «Карточка T-003»"));

    let o = pumba(&d, &["brief", "T-011"]);
    assert_eq!(o.code, 3);
    assert!(o.out.contains("NOT-FOUND T-011") && o.out.contains("похожие: T-001"), "{}", o.out);
    assert!(o.err.contains("карточки T-011 нет (3)"), "{}", o.err);
    let j = json(&pumba(&d, &["brief", "T-011", "--json"]));
    assert_eq!((j["ok"].as_bool(), j["code"].as_i64()), (Some(false), Some(3)));

    assert_eq!(pumba(&d, &["brief"]).code, 2);
    assert_eq!(pumba(&d, &["brief", "T-001", "--nope"]).code, 2);
    assert_eq!(pumba(&d, &["brief", "T-001", "--body", "x"]).code, 2);
    let _ = std::fs::remove_dir_all(d);
}

// ---------------------------------------------------------------- move

#[test]
fn move_to_columns_and_next() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    let o = pumba(&d, &["move", "T-001", "in-progress"]);
    assert_eq!((o.code, o.out.as_str()), (0, "T-001: backlog → in-progress\n"), "{}", o.err);
    assert_eq!(column_of(&d, "T-001"), "in-progress");
    // AGENTS.md is regenerated
    let agents = std::fs::read_to_string(d.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains("| T-001 | task | in-progress |"), "{agents}");
    let o = pumba(&d, &["move", "T-001", "next", "--expect-from", "in-progress"]);
    assert_eq!(o.out, "T-001: in-progress → review\n");
    assert_eq!(pumba(&d, &["move", "t1", "next"]).out, "T-001: review → done\n");
    // done has no next
    let o = pumba(&d, &["move", "T-001", "next"]);
    assert_eq!(o.code, 3);
    assert!(o.err.contains("нет next") && o.out.contains("backlog «Бэклог»"), "{}{}", o.out, o.err);
    // an unknown column lists the columns
    let o = pumba(&d, &["move", "T-001", "nowhere"]);
    assert_eq!(o.code, 3);
    assert!(o.out.contains("колонки:") && o.out.contains("done «Готово»"));
    // back to the beginning
    assert_eq!(pumba(&d, &["move", "T-001", "backlog"]).code, 0);
    assert_eq!(column_of(&d, "T-001"), "backlog");
    // json
    let j = json(&pumba(&d, &["move", "T-001", "review", "--json"]));
    assert_eq!((j["from"].as_str(), j["to"].as_str(), j["changed"].as_bool()), (Some("backlog"), Some("review"), Some(true)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn move_is_idempotent_and_checks_expect_from() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    let args = ["move", "T-001", "in-progress", "--expect-from", "backlog", "--note", "беру", "--at", AT];
    assert_eq!(pumba(&d, &args).code, 0);
    let (card_before, agents_before) = (read_card(&d, "T-001"), std::fs::read(d.join(".pumba/AGENTS.md")).unwrap());
    // the same call again: success, no change, no second note
    let o = pumba(&d, &args);
    assert_eq!((o.code, o.out.as_str()), (0, "T-001: уже в in-progress\n"));
    assert_eq!(read_card(&d, "T-001"), card_before);
    assert_eq!(std::fs::read(d.join(".pumba/AGENTS.md")).unwrap(), agents_before);
    assert_eq!(card_before.matches("беру").count(), 1);
    // the card is elsewhere than expected: 6 and nothing changes
    let o = pumba(&d, &["move", "T-001", "review", "--expect-from", "backlog"]);
    assert_eq!(o.code, 6, "{}", o.err);
    assert_eq!(column_of(&d, "T-001"), "in-progress");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn move_refused_when_blocked_trashed_or_manual() {
    let d = project();
    card(&d, "T-001", "in-progress", "", "dep");
    card(&d, "T-002", "backlog", "depends: T-001\n", "x");
    card(&d, "T-003", "backlog", "deleted: true\n", "x");
    // blocked: 5, the blocker is named, nothing changes
    let o = pumba(&d, &["move", "T-002", "in-progress"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.out, "T-001 — «В работе» (`in-progress`)\n");
    assert_eq!(column_of(&d, "T-002"), "backlog");
    assert_eq!(pumba(&d, &["move", "T-002", "done"]).code, 5);
    // the blocker done: the card moves
    assert_eq!(pumba(&d, &["move", "T-001", "done"]).code, 0);
    assert_eq!(pumba(&d, &["move", "T-002", "in-progress"]).code, 0);
    // trash: 4
    let o = pumba(&d, &["move", "T-003", "in-progress"]);
    assert_eq!(o.code, 4, "{}", o.err);
    assert_eq!(column_of(&d, "T-003"), "backlog");
    // no card: 3
    assert_eq!(pumba(&d, &["move", "T-077", "done"]).code, 3);
    // manual board: 7 until --user-approved
    set_rule(&d, "board", "manual");
    let o = pumba(&d, &["move", "T-002", "review"]);
    assert_eq!(o.code, 7, "{}", o.err);
    assert!(o.out.contains("Перенести T-002") && o.out.contains("--user-approved"), "{}", o.out);
    assert_eq!(column_of(&d, "T-002"), "in-progress");
    let o = pumba(&d, &["move", "T-002", "review", "--user-approved"]);
    assert_eq!((o.code, column_of(&d, "T-002").as_str()), (0, "review"), "{}", o.err);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn move_note_goes_under_the_work_heading() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    let o = pumba(&d, &["move", "T-001", "in-progress", "--note", "беру в работу", "--at", AT]);
    assert_eq!(o.code, 0, "{}", o.err);
    let text = read_card(&d, "T-001");
    assert!(text.ends_with("Текст.\n\n## Ход работы\n- 2026-10-31 20:41 UTC — «Бэклог» → «В работе»: беру в работу\n"), "{text}");
    // brief reads the transition back
    let j = json(&pumba(&d, &["brief", "T-001", "--json"]));
    assert_eq!(j["log"][0]["to"], "В работе");
    assert!(pumba(&d, &["brief", "T-001"]).out.contains("log: 2026-10-31 20:41 UTC «Бэклог»→«В работе»;"));
    let _ = std::fs::remove_dir_all(d);
}

// ---------------------------------------------------------------- note

#[test]
fn note_appends_without_duplicating_the_heading() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    let o = pumba(&d, &["note", "T-001", "первая", "--at", AT]);
    assert_eq!((o.code, o.out.as_str()), (0, "T-001: заметка добавлена\n"), "{}", o.err);
    let o = pumba(&d, &["note", "T-001", "вторая", "--at", "2026-10-31 20:42 UTC"]);
    assert_eq!(o.code, 0);
    let text = read_card(&d, "T-001");
    assert!(text.ends_with("Текст.\n\n## Ход работы\n- 2026-10-31 20:41 UTC — первая\n- 2026-10-31 20:42 UTC — вторая\n"), "{text}");
    assert_eq!(text.matches("## Ход работы").count(), 1);
    // the same text as the last note: exit 0, nothing changes
    let o = pumba(&d, &["note", "T-001", "вторая", "--at", "2026-10-31 20:50 UTC"]);
    assert_eq!((o.code, o.out.as_str()), (0, "T-001: такая заметка уже последняя\n"));
    assert_eq!(read_card(&d, "T-001"), text);
    // text that starts with dashes goes after `--`
    assert_eq!(pumba(&d, &["note", "T-001", "--at", AT, "--", "--важно"]).code, 0);
    assert!(read_card(&d, "T-001").ends_with(" — --важно\n"));
    // errors
    assert_eq!(pumba(&d, &["note", "T-001", "  "]).code, 2);
    assert_eq!(pumba(&d, &["note", "T-009", "x"]).code, 3);
    assert_eq!(pumba(&d, &["note", "T-001"]).code, 2);
    assert_eq!(pumba(&d, &["note", "T-001", "a", "b"]).code, 2);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn note_without_at_writes_a_utc_stamp() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    assert_eq!(pumba(&d, &["note", "T-001", "с часами"]).code, 0);
    let line = read_card(&d, "T-001").lines().last().unwrap().to_string();
    // `- YYYY-MM-DD HH:MM UTC — с часами`
    let stamp = line.strip_prefix("- ").and_then(|l| l.strip_suffix(" — с часами")).unwrap_or_else(|| panic!("{line}"));
    assert!(stamp.len() == 20 && stamp.ends_with(" UTC") && stamp.as_bytes()[4] == b'-' && stamp.as_bytes()[13] == b':', "{stamp}");
    assert!(stamp.starts_with("202"), "{stamp}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn note_manual_board_and_trash() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    card(&d, "T-002", "backlog", "deleted: true\n", "x");
    assert_eq!(pumba(&d, &["note", "T-002", "x"]).code, 4);
    set_rule(&d, "board", "manual");
    let o = pumba(&d, &["note", "T-001", "тихо"]);
    assert_eq!(o.code, 7);
    assert!(!read_card(&d, "T-001").contains("тихо"));
    assert_eq!(pumba(&d, &["note", "T-001", "тихо", "--user-approved"]).code, 0);
    assert!(read_card(&d, "T-001").contains("тихо"));
    let _ = std::fs::remove_dir_all(d);
}

// ------------------------------------------------- rules, column, unblocks

#[test]
fn rules_column_and_unblocks() {
    let d = project();
    card(&d, "T-001", "in-progress", "", "x");
    card(&d, "T-002", "backlog", "depends: T-001\n", "x");
    card(&d, "T-003", "backlog", "depends: T-001, T-004\n", "x");
    card(&d, "T-004", "backlog", "", "x");

    let o = pumba(&d, &["rules"]);
    assert_eq!(o.code, 0, "{}", o.err);
    assert!(o.out.starts_with("## Правила проекта\n- Стиль коммитов: Conventional Commits."), "{}", o.out);
    assert_eq!(o.out.lines().filter(|l| l.starts_with("- ")).count(), 6);
    let o = pumba(&d, &["rules", "--part", "tests,merge"]);
    assert_eq!(o.out.lines().filter(|l| l.starts_with("- ")).count(), 2, "{}", o.out);
    let j = json(&pumba(&d, &["rules", "--part", "tests", "--json"]));
    assert_eq!(j["rules"][0]["id"], "tests");
    assert_eq!(pumba(&d, &["rules", "--part", "bogus"]).code, 2);

    let o = pumba(&d, &["column", "review"]);
    assert!(o.out.starts_with("col=review «На ревью» (3/5) agent=pumba-reviewer next=done «Готово» last=done\nhash col:review="), "{}", o.out);
    assert!(o.out.contains("## Работа"));
    assert_eq!(pumba(&d, &["column", "zzz"]).code, 3);
    assert_eq!(json(&pumba(&d, &["column", "review", "--json"]))["column"]["id"], "review");

    let o = pumba(&d, &["unblocks", "T-001"]);
    assert_eq!(o.code, 0);
    assert_eq!(
        o.out,
        "T-002 «Карточка T-002» — разблокируется, когда T-001 дойдёт до «Готово»\n\
         T-003 «Карточка T-003» — разблокируется, когда T-001 дойдёт до «Готово»; ждёт ещё T-004\n"
    );
    assert_eq!(pumba(&d, &["move", "T-001", "done"]).code, 0);
    let o = pumba(&d, &["unblocks", "T-001"]);
    assert_eq!(o.out, "T-002 «Карточка T-002» — разблокирована\nT-003 «Карточка T-003» — ждёт ещё T-004\n");
    let j = json(&pumba(&d, &["unblocks", "T-001", "--json"]));
    assert_eq!((j["done"].as_bool(), j["dependents"][0]["state"].as_str(), j["dependents"][1]["waiting"][0].as_str()), (Some(true), Some("unblocked"), Some("T-004")));
    assert_eq!(pumba(&d, &["unblocks", "T-099"]).code, 3);
    let _ = std::fs::remove_dir_all(d);
}

// ---------------------------------------------------------------- hash

#[test]
fn rules_hash_follows_rules_only() {
    let d = project();
    card(&d, "T-001", "backlog", "", "Текст.");
    let h = pumba(&d, &["rules-hash"]);
    assert_eq!(h.code, 0, "{}", h.err);
    let hash = h.out.trim().to_string();
    assert_eq!(hash.len(), 12);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(pumba(&d, &["rules-hash"]).out, h.out, "stable");
    // AGENTS.md carries the same fingerprint
    let agents = std::fs::read_to_string(d.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains(&format!("<!-- pumba-rules-hash: {hash} · protocol 1 -->")), "{agents}");
    // cards, notes and moves are not rules
    assert_eq!(pumba(&d, &["note", "T-001", "заметка"]).code, 0);
    assert_eq!(pumba(&d, &["move", "T-001", "in-progress"]).code, 0);
    card(&d, "T-002", "backlog", "", "другое тело");
    assert_eq!(pumba(&d, &["rules-hash"]).out, h.out);
    // parts
    let p = pumba(&d, &["rules-hash", "--parts"]);
    let lines: Vec<&str> = p.out.lines().collect();
    assert_eq!(lines[0], format!("rules={hash}"));
    for name in ["proto", "cols", "col:backlog", "col:in-progress", "col:review", "col:on-dev", "col:done", "rule:commits", "rule:board", "custom"] {
        assert!(lines.iter().any(|l| l.starts_with(&format!("{name}="))), "{name}\n{}", p.out);
    }
    let j = json(&pumba(&d, &["rules-hash", "--parts", "--json"]));
    assert_eq!((j["hash"].as_str(), j["protocol"].as_i64()), (Some(hash.as_str()), Some(1)));
    assert!(j["parts"].as_array().unwrap().len() >= 14);
    // a rule changes: the hash changes, and AGENTS.md follows after the next write
    set_rule(&d, "tests", "optional");
    let h2 = pumba(&d, &["rules-hash"]);
    assert_ne!(h2.out, h.out);
    assert_eq!(pumba(&d, &["move", "T-001", "review"]).code, 0);
    let agents = std::fs::read_to_string(d.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains(&format!("<!-- pumba-rules-hash: {} · protocol 1 -->", h2.out.trim())), "{agents}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn rules_hash_changes_with_column_rule_and_order() {
    let d = project();
    let base = pumba(&d, &["rules-hash"]).out;
    let p = d.join(".pumba/board.json");
    let orig = std::fs::read_to_string(&p).unwrap();
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_str(&orig).unwrap();
        f(&mut v);
        std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        pumba(&d, &["rules-hash"]).out
    };
    let a = edit(&|v| v["columns"][1]["after"] = "Другое условие.".into());
    let b = edit(&|v| v["columns"].as_array_mut().unwrap().swap(1, 2));
    let c = edit(&|v| v["custom_rules"] = "Своё правило".into());
    let col_color = edit(&|v| v["columns"][1]["color"] = "blue".into());
    assert!(a != base && b != base && c != base && a != b && b != c);
    assert_eq!(col_color, base, "the color is not a rule");
    let _ = std::fs::remove_dir_all(d);
}

// ------------------------------------------------------------ no board

#[test]
fn no_board_is_code_9() {
    // a folder with no .pumba/ above it (the temp dir has none)
    let d = std::env::temp_dir().join(format!("pumba-cli-noboard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    if pumba_core_find(&d) {
        return; // a board far above the temp dir: the test would be meaningless
    }
    for args in [vec!["brief", "T-001"], vec!["move", "T-001", "done"], vec!["note", "T-001", "x"], vec!["rules"], vec!["unblocks", "T-001"], vec!["column", "done"], vec!["rules-hash"]] {
        let o = pumba(&d, &args);
        assert_eq!(o.code, 9, "{args:?}: {}{}", o.out, o.err);
        assert!(o.err.contains("доски нет") && o.err.contains("(9)") && o.err.contains("pumba init"), "{}", o.err);
    }
    let o = pumba(&d, &["rules-hash", "--json"]);
    assert_eq!(o.code, 9);
    assert_eq!(json(&o)["code"], 9);
    // --root pointing to a folder without a board
    let o = pumba(&std::env::temp_dir(), &["rules-hash", "--root", d.to_str().unwrap()]);
    assert_eq!(o.code, 9);
    // a board.json that does not parse
    let bad = d.join("bad");
    std::fs::create_dir_all(bad.join(".pumba")).unwrap();
    std::fs::write(bad.join(".pumba/board.json"), "{ not json").unwrap();
    let o = pumba(&bad, &["brief", "T-001"]);
    assert_eq!(o.code, 9, "{}", o.err);
    let _ = std::fs::remove_dir_all(d);
}

fn pumba_core_find(d: &Path) -> bool {
    pumba_core::model::Store::find(d).is_some()
}

// ------------------------------------------------------- command names

#[test]
fn command_names_win_over_paths_and_help_says_so() {
    let d = project();
    // `pumba brief` with no card is the command (usage error), not a TUI on a folder named brief
    std::fs::create_dir_all(d.join("brief")).unwrap();
    let o = pumba(&d, &["brief"]);
    assert_eq!(o.code, 2, "{}", o.err);
    assert!(o.err.contains("не хватает аргумента: ID"), "{}", o.err);
    let h = pumba(&d, &["--help"]);
    assert_eq!(h.code, 0);
    for c in ["brief", "move", "note", "rules", "unblocks", "column", "rules-hash"] {
        assert!(h.out.contains(&format!("pumba {c}")), "{c}");
    }
    assert!(h.out.contains("./brief"), "{}", h.out);
    assert!(h.out.contains("pumba init"), "the old commands are still in the help");
    assert!(pumba(&d, &["brief", "--help"]).out.starts_with("pumba brief ID"));
    let _ = std::fs::remove_dir_all(d);
}
