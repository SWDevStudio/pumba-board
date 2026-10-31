//! Tests of the command operations over a board in a temp folder. Time is always passed in
//! (`NOW`) or given as `at`, never read from the clock.
use super::*;
use crate::model::Task;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 2026-10-31 20:41 UTC
const NOW: u64 = 1_793_479_260;

fn fresh() -> (Board, PathBuf) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("pumba-api-test-{}-{nanos}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&root).unwrap();
    let mut b = Board::new(&root);
    b.store.init().unwrap();
    b.reload().unwrap();
    (b, root)
}

/// Write a card file and reload the board.
fn card(b: &mut Board, id: &str, column: &str, extra: &str, body: &str) {
    let kind = if id.starts_with('E') { "epic" } else { "task" };
    let md = format!("---\nid: {id}\ntype: {kind}\ntitle: Карточка {id}\ncolumn: {column}\n{extra}hidden: false\norder: 0\n---\n\n{body}\n");
    std::fs::write(b.store.dir.join("tasks").join(format!("{id}.md")), md).unwrap();
    b.reload().unwrap();
}

fn disk(b: &Board, id: &str) -> Task {
    Task::from_md(&b.store.read_task_raw(id).unwrap(), id).unwrap()
}

fn mv(to: &str) -> MoveReq {
    MoveReq { id: "T-001".into(), to: to.into(), at: Some("2026-10-31 20:41 UTC".into()), ..Default::default() }
}

fn err(r: Result<MoveOut, ApiErr>) -> ApiErr {
    r.unwrap_err()
}

fn set_rule(b: &mut Board, id: &str, value: &str) {
    b.cfg.rules.iter_mut().find(|r| r.id == id).unwrap().value = value.into();
    b.commit(&[], true).unwrap();
}

// ---------------------------------------------------------------- brief

/// E-001 holds T-001 (in progress); T-001 depends on the done T-004; T-002 waits for T-001 and T-003.
fn demo(b: &mut Board) {
    card(b, "E-001", "in-progress", "", "Эпик.");
    card(b, "T-004", "done", "", "готово");
    card(b, "T-003", "backlog", "", "ещё не начата");
    card(b, "T-001", "in-progress", "parent: E-001\nsprint: 1\ndepends: T-004\n", "Первая строка.\nВторая строка.");
    card(b, "T-002", "backlog", "depends: T-001, T-003\n", "ждёт");
}

#[test]
fn brief_has_every_part() {
    let (mut b, root) = fresh();
    demo(&mut b);
    let r = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((r.status.as_str(), r.exit, r.api), ("OK", 0, 1));
    assert_eq!((r.id.as_str(), r.kind.as_str(), r.sprint), ("T-001", "task", Some(1)));
    assert_eq!((r.column.id.as_str(), r.column.pos, r.column.total), ("in-progress", 2, 5));
    assert_eq!(r.column.agent, "pumba-worker");
    assert_eq!(r.column.next.as_ref().map(|n| n.id.as_str()), Some("review"));
    assert_eq!(r.column.last, "done");
    assert_eq!(r.parent.as_ref().map(|p| (p.id.as_str(), p.column.as_str())), Some(("E-001", "in-progress")));
    assert_eq!(r.depends.len(), 1);
    assert!(r.depends[0].done && !r.depends[0].blocks());
    assert_eq!(r.dependents, [DependentView { id: "T-002".into(), waiting: vec!["T-003".into()] }]);
    assert_eq!(r.board, "auto");
    assert_eq!(r.rules.len(), 6);
    assert_eq!(r.rules_hash, crate::rulehash::rules_hash(&b.cfg));
    let p = r.parts.as_ref().unwrap();
    assert!(p.before.contains("В РАБОТЕ") && p.rule.contains("pumba-planner") && p.after.contains("ИТОГ: PASS"));
    let body = r.body.as_ref().unwrap();
    assert_eq!((body.lines, body.shown), (2, 2));

    let text = r.render();
    let first = text.lines().next().unwrap();
    assert_eq!(first, "OK T-001 task «Карточка T-001» sprint=1 model=default effort=default api=1");
    assert!(text.contains("col=in-progress «В работе» (2/5) agent=pumba-worker next=review «На ревью» last=done\n"), "{text}");
    assert!(text.contains("parent=E-001 «Карточка E-001» in-progress | depends: T-004 done ✓ | dependents: T-002 (ждёт ещё T-003)\n"), "{text}");
    assert!(text.contains("board=auto commits=conventional branching=per-task merge=squash tests=required autonomy=ask\n"), "{text}");
    assert!(text.contains(&format!("hash rules={} col:in-progress={}\n", r.rules_hash, r.column.hash)), "{text}");
    assert!(text.contains("log: -; visits=0\n"));
    for h in ["## Перед работой\n", "## Работа\n", "## После работы\n", "## Тело (2 строк)\nПервая строка.\nВторая строка.\n"] {
        assert!(text.contains(h), "{h}\n{text}");
    }
    // JSON carries the same data
    let j: serde_json::Value = serde_json::from_str(&to_json(&r)).unwrap();
    assert_eq!(j["status"], "OK");
    assert_eq!(j["column"]["next"]["id"], "review");
    assert!(j["parts"]["rule"].as_str().unwrap().contains("pumba-planner"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_normalizes_the_id() {
    let (mut b, root) = fresh();
    demo(&mut b);
    for q in ["t1", "T001", "1", "t-1"] {
        assert_eq!(brief(&b, q, &BriefOpts::default(), NOW).unwrap().id, "T-001", "{q}");
    }
    assert_eq!(normalize_id(" e2 "), "E-002");
    assert_eq!(normalize_id("abc"), "ABC");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_known_skips_parts() {
    let (mut b, root) = fresh();
    demo(&mut b);
    let full = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    let col = format!("col:in-progress={}", full.column.hash);
    let short = |known: Vec<String>| brief(&b, "T-001", &BriefOpts { known, body: BodyMode::Head }, NOW).unwrap();

    let r = short(vec![col.clone()]);
    assert!(r.parts.is_none());
    assert_eq!(r.known, ["col:in-progress"]);
    let text = r.render();
    assert!(text.contains("known: col:in-progress\n") && !text.contains("## Работа"), "{text}");
    assert!(text.len() < full.render().len() / 2, "{} vs {}", text.len(), full.render().len());
    assert!(text.contains("## Тело"), "the body is not part of the column");

    // a prefix and an ellipsis are accepted, a wrong or too short hash is not
    assert!(short(vec![format!("col:in-progress={}…", &full.column.hash[..6])]).parts.is_none());
    assert!(short(vec!["col:in-progress=ffffffffffff".into()]).parts.is_some());
    assert!(short(vec!["col:in-progress=".into()]).parts.is_some());
    assert!(short(vec![format!("col:in-progress={}", &full.column.hash[..3])]).parts.is_some());
    assert!(short(vec![format!("col:review={}", full.column.hash)]).parts.is_some(), "another column");

    // the rules entry drops the labels of the project rules except the board mode
    let r = short(vec![format!("rules={}", full.rules_hash)]);
    assert_eq!(r.known, ["rules"]);
    let text = r.render();
    assert!(text.contains("\nboard=auto\n") && !text.contains("tests=required"), "{text}");
    // both
    let r = short(vec![col, format!("rules={}", full.rules_hash)]);
    assert_eq!(r.known, ["col:in-progress", "rules"]);
    // the column rule changed: the old hash no longer matches, the text comes back
    b.cfg.columns[1].after.push_str(" Ещё условие.");
    b.commit(&[], true).unwrap();
    let r = brief(&b, "T-001", &BriefOpts { known: vec![format!("col:in-progress={}", full.column.hash)], body: BodyMode::Head }, NOW).unwrap();
    assert!(r.parts.unwrap().after.contains("Ещё условие."));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_statuses() {
    let (mut b, root) = fresh();
    demo(&mut b);
    // blocked: T-002 waits for T-001 (in progress) and T-003 (backlog)
    let r = brief(&b, "T-002", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((r.status.as_str(), r.exit), ("BLOCKED", 5));
    assert!(r.parts.is_none() && r.body.is_none());
    let text = r.render();
    assert_eq!(text.lines().next().unwrap(), "BLOCKED T-001 «В работе» (in-progress), T-003 «Бэклог» (backlog)");
    assert!(text.lines().nth(1).unwrap().starts_with("T-002 task "));
    // deleted
    card(&mut b, "T-005", "backlog", "deleted: true\n", "x");
    let r = brief(&b, "T-005", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((r.status.as_str(), r.exit), ("DELETED", 4));
    assert_eq!(r.render(), "DELETED T-005 «Карточка T-005»\n");
    // a trashed or unknown dependency does not block
    card(&mut b, "T-006", "backlog", "depends: T-005, T-099\n", "x");
    let r = brief(&b, "T-006", &BriefOpts::default(), NOW).unwrap();
    assert_eq!(r.status, "OK");
    assert!(r.render().contains("T-005 в корзине (не блокирует), T-099 нет (не блокирует)"));
    // unknown column: shown in the first one
    card(&mut b, "T-007", "ghost", "", "x");
    let r = brief(&b, "T-007", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((r.status.as_str(), r.exit, r.column.id.as_str()), ("UNKNOWN-COLUMN", 0, "backlog"));
    assert!(r.render().lines().next().unwrap().ends_with("api=1 column=ghost"));
    // no pipeline: no column has an agent or a next
    for c in &mut b.cfg.columns {
        c.agent.clear();
        c.next.clear();
    }
    b.commit(&[], true).unwrap();
    let r = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((r.status.as_str(), r.exit), ("NO-PIPELINE", 0));
    assert!(r.column.next.is_none() && r.render().contains("agent=- next=- "));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_not_found_suggests_similar() {
    let (mut b, root) = fresh();
    demo(&mut b);
    let e = brief(&b, "T-011", &BriefOpts::default(), NOW).unwrap_err();
    assert_eq!(e.code, 3);
    assert_eq!(e.out[0], "NOT-FOUND T-011");
    assert!(e.out[1].starts_with("похожие: ") && e.out[1].contains("T-001"), "{:?}", e.out);
    let e = brief(&b, "zzz-9999999", &BriefOpts::default(), NOW).unwrap_err();
    assert_eq!((e.code, e.out.len()), (3, 1));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_body_modes_log_and_visits() {
    let (mut b, root) = fresh();
    let many: String = (1..=50).map(|i| format!("строка {i}\n")).collect();
    let notes = "\n## Ход работы\n\
        - 2026-10-30 08:00 UTC — «Бэклог» → «В работе»: давно\n\
        - 2026-10-31 14:05 UTC — «В работе» → «На ревью»: ревью\n\
        - 2026-10-31 15:00 UTC — «На ревью» → «В работе»: замечания\n\
        - 2026-10-31 16:00 UTC — просто заметка\n\
        - 2026-10-31 17:00 UTC — «В работе» → «На ревью»: ещё\n\
        - 2026-10-31 18:00 UTC — «На ревью» → «В работе»: снова\n";
    card(&mut b, "T-001", "in-progress", "", &format!("{many}{notes}"));
    let r = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    // the last three transitions; the plain note is not a transition
    assert_eq!(r.log.len(), 3);
    assert_eq!((r.log[0].when.as_str(), r.log[2].to.as_str()), ("2026-10-31 15:00 UTC", "В работе"));
    // entries into «В работе» in the last 24 hours before NOW: 15:00 and 18:00 (the one of 30.10 is older)
    assert_eq!(r.visits, 2);
    let t = r.render();
    assert!(t.contains("log: 2026-10-31 15:00 UTC «На ревью»→«В работе»; 2026-10-31 17:00 UTC «В работе»→«На ревью»; 2026-10-31 18:00 UTC «На ревью»→«В работе»; visits=2\n"), "{t}");
    // head: 40 of 50 lines of the text, then the last 3 notes
    let body = r.body.as_ref().unwrap();
    assert_eq!((body.lines, body.shown, body.notes_total), (50, 40, 6));
    assert!(t.contains("## Тело (40 из 50 строк; всё: --body full)\n") && t.contains("строка 40\n") && !t.contains("строка 41"), "{t}");
    assert!(t.contains("## Ход работы (последние 3 из 6)\n- 2026-10-31 16:00 UTC — просто заметка\n"));
    // full: the whole body as it is; none: no body
    let r = brief(&b, "T-001", &BriefOpts { known: vec![], body: BodyMode::Full }, NOW).unwrap();
    let t = r.render();
    assert!(t.contains("строка 50\n") && t.contains("- 2026-10-30 08:00 UTC — «Бэклог»"), "{t}");
    let r = brief(&b, "T-001", &BriefOpts { known: vec![], body: BodyMode::None }, NOW).unwrap();
    assert!(r.body.is_none() && !r.render().contains("## Тело"));
    // a long time later nothing counts as a visit
    assert_eq!(brief(&b, "T-001", &BriefOpts::default(), NOW + 3 * 86_400).unwrap().visits, 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn brief_is_compact() {
    // the target is about 400 tokens without the column text (about 3.3 bytes per token)
    let (mut b, root) = fresh();
    demo(&mut b);
    let r = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    let text = r.render();
    let p = r.parts.as_ref().unwrap();
    let column_text = p.before.len() + p.rule.len() + p.after.len() + 60;
    let rest = text.len() - column_text.min(text.len());
    assert!(rest < 1300, "{rest} bytes without the column text:\n{text}");
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------- move

#[test]
fn move_to_column_and_next() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    card(&mut b, "T-009", "in-progress", "order: 7\n", "x");
    let o = move_card(&mut b, &mv("in-progress"), NOW).unwrap();
    assert_eq!((o.from.as_str(), o.to.as_str(), o.changed), ("backlog", "in-progress", true));
    assert_eq!(o.render(), "T-001: backlog → in-progress\n");
    let t = disk(&b, "T-001");
    assert_eq!(t.column, "in-progress");
    assert!(t.order > 0, "put at the end of the column: {}", t.order);
    // no --note: the body is not touched
    assert_eq!(t.body, "Текст.");
    // AGENTS.md follows
    let agents = std::fs::read_to_string(b.store.dir.join("AGENTS.md")).unwrap();
    assert!(agents.contains("| T-001 | task | in-progress |"), "{agents}");
    // next: in-progress -> review -> done
    assert_eq!(move_card(&mut b, &mv("next"), NOW).unwrap().to, "review");
    assert_eq!(move_card(&mut b, &mv("next"), NOW).unwrap().to, "done");
    assert_eq!(disk(&b, "T-001").column, "done");
    // by column name and without case
    assert_eq!(move_card(&mut b, &mv("БЭКЛОГ"), NOW).unwrap().to, "backlog");
    assert_eq!(move_card(&mut b, &mv("In-Progress"), NOW).unwrap().to, "in-progress");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_codes() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    // 3: no card (with similar ids), no column (with the list), no next
    let mut r = mv("review");
    r.id = "T-777".into();
    let e = err(move_card(&mut b, &r, NOW));
    assert_eq!(e.code, 3);
    let e = err(move_card(&mut b, &mv("nowhere"), NOW));
    assert_eq!(e.code, 3);
    assert!(e.out.iter().any(|l| l.contains("in-progress «В работе»")), "{:?}", e.out);
    let e = err(move_card(&mut b, &mv("next"), NOW));
    assert_eq!((e.code, e.msg.contains("нет next")), (3, true));
    assert!(e.out.len() > 3);
    // 6: --expect-from does not match
    let mut r = mv("in-progress");
    r.expect_from = Some("review".into());
    assert_eq!(err(move_card(&mut b, &r, NOW)).code, 6);
    assert_eq!(disk(&b, "T-001").column, "backlog");
    r.expect_from = Some("backlog".into());
    assert!(move_card(&mut b, &r, NOW).is_ok());
    // 4: trash
    let mut t = b.tasks[0].clone();
    t.id = "T-002".into();
    t.column = "backlog".into();
    t.deleted = true;
    b.tasks.push(t);
    b.commit(&["T-002".into()], false).unwrap();
    let mut r = mv("review");
    r.id = "T-002".into();
    assert_eq!(err(move_card(&mut b, &r, NOW)).code, 4);
    assert_eq!(disk(&b, "T-002").column, "backlog");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_forward_refused_while_blocked() {
    let (mut b, root) = fresh();
    card(&mut b, "T-002", "in-progress", "", "dep");
    card(&mut b, "T-001", "backlog", "depends: T-002, T-404\n", "x");
    let e = err(move_card(&mut b, &mv("in-progress"), NOW));
    assert_eq!(e.code, 5);
    assert_eq!(e.out, ["T-002 — «В работе» (`in-progress`)"]);
    assert_eq!(disk(&b, "T-001").column, "backlog");
    // the last column is forward too
    assert_eq!(err(move_card(&mut b, &mv("done"), NOW)).code, 5);
    // back (to a column before the card's) is allowed
    card(&mut b, "T-001", "review", "depends: T-002\n", "x");
    assert_eq!(move_card(&mut b, &mv("backlog"), NOW).unwrap().to, "backlog");
    // the dependency done: forward works
    card(&mut b, "T-002", "done", "", "dep");
    assert_eq!(move_card(&mut b, &mv("in-progress"), NOW).unwrap().to, "in-progress");
    // a trashed dependency does not hold the card
    card(&mut b, "T-002", "backlog", "deleted: true\n", "dep");
    assert_eq!(move_card(&mut b, &mv("review"), NOW).unwrap().to, "review");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_manual_board_needs_approval() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "x");
    set_rule(&mut b, "board", "manual");
    let e = err(move_card(&mut b, &mv("in-progress"), NOW));
    assert_eq!(e.code, 7);
    assert!(e.out.iter().any(|l| l.contains("Перенести T-001") && l.contains("«Бэклог» в «В работе»")), "{:?}", e.out);
    assert!(e.out.iter().any(|l| l.contains("--user-approved")));
    assert_eq!(disk(&b, "T-001").column, "backlog");
    let mut r = mv("in-progress");
    r.user_approved = true;
    assert_eq!(move_card(&mut b, &r, NOW).unwrap().to, "in-progress");
    // already there is a success even without approval
    r.user_approved = false;
    assert!(!move_card(&mut b, &r, NOW).unwrap().changed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_is_idempotent() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "x");
    let mut r = mv("in-progress");
    r.note = Some("взял".into());
    r.expect_from = Some("backlog".into());
    assert!(move_card(&mut b, &r, NOW).unwrap().changed);
    let (raw, agents) = (b.store.read_task_raw("T-001").unwrap(), std::fs::read_to_string(b.store.dir.join("AGENTS.md")).unwrap());
    // the same call again (a retry): exit 0, no change, no second note, even with --expect-from
    let o = move_card(&mut b, &r, NOW + 60).unwrap();
    assert!(!o.changed);
    assert_eq!(o.render(), "T-001: уже в in-progress\n");
    assert_eq!(b.store.read_task_raw("T-001").unwrap(), raw);
    assert_eq!(std::fs::read_to_string(b.store.dir.join("AGENTS.md")).unwrap(), agents);
    assert_eq!(raw.matches("взял").count(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_with_note_writes_transition_line() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    let mut r = mv("in-progress");
    r.note = Some("беру\nв работу".into());
    move_card(&mut b, &r, NOW).unwrap();
    assert_eq!(disk(&b, "T-001").body, "Текст.\n\n## Ход работы\n- 2026-10-31 20:41 UTC — «Бэклог» → «В работе»: беру в работу");
    // no `at`: the time comes from `now`
    let mut r = mv("review");
    r.at = None;
    r.note = Some("проверки зелёные".into());
    move_card(&mut b, &r, NOW).unwrap();
    let body = disk(&b, "T-001").body;
    assert!(body.ends_with("\n- 2026-10-31 20:41 UTC — «В работе» → «На ревью»: проверки зелёные"), "{body}");
    assert_eq!(body.matches(LOG_HEADING).count(), 1);
    // brief reads it back as a transition
    let br = brief(&b, "T-001", &BriefOpts::default(), NOW).unwrap();
    assert_eq!((br.log.len(), br.visits), (2, 1));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_keeps_text_appended_outside() {
    // the card file got a note from another writer while we were about to move it
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    let path = b.store.dir.join("tasks/T-001.md");
    let mut hits = 0;
    let r = move_card_hooked(&mut b, &mv("in-progress"), NOW, &mut || {
        hits += 1;
        if hits == 1 {
            let t = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, format!("{}\n## Ход работы\n- чужая запись\n", t.trim_end())).unwrap();
        }
    })
    .unwrap();
    assert!(r.changed);
    assert_eq!(hits, 2, "the first attempt saw the changed file and was repeated");
    let t = disk(&b, "T-001");
    assert_eq!(t.column, "in-progress");
    assert!(t.body.contains("- чужая запись"), "{}", t.body);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn move_conflict_after_three_attempts() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    let path = b.store.dir.join("tasks/T-001.md");
    let mut hits = 0;
    let e = move_card_hooked(&mut b, &mv("in-progress"), NOW, &mut || {
        hits += 1;
        let t = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{}\nещё {hits}\n", t.trim_end())).unwrap();
    })
    .unwrap_err();
    assert_eq!((e.code, hits), (6, ATTEMPTS));
    // nothing was written by us: the column is still the old one
    assert_eq!(disk(&b, "T-001").column, "backlog");
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------------------- note

fn nreq(text: &str) -> NoteReq {
    NoteReq { id: "T-001".into(), text: text.into(), at: Some("2026-10-31 20:41 UTC".into()), ..Default::default() }
}

#[test]
fn note_appends_under_the_heading_once() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    let o = add_note(&mut b, &nreq("первая"), NOW).unwrap();
    assert_eq!((o.changed, o.render().as_str()), (true, "T-001: заметка добавлена\n"));
    let mut r = nreq("вторая");
    r.at = Some("2026-10-31 20:42 UTC".into());
    add_note(&mut b, &r, NOW).unwrap();
    assert_eq!(
        disk(&b, "T-001").body,
        "Текст.\n\n## Ход работы\n- 2026-10-31 20:41 UTC — первая\n- 2026-10-31 20:42 UTC — вторая"
    );
    // the same text as the last note: success, no change
    let raw = b.store.read_task_raw("T-001").unwrap();
    assert!(!add_note(&mut b, &nreq("вторая"), NOW + 600).unwrap().changed);
    assert_eq!(b.store.read_task_raw("T-001").unwrap(), raw);
    // an older text is added again (only the last line counts)
    assert!(add_note(&mut b, &nreq("первая"), NOW).unwrap().changed);
    // current time when no `at`
    let mut r = nreq("с часами");
    r.at = None;
    add_note(&mut b, &r, NOW).unwrap();
    assert!(disk(&b, "T-001").body.ends_with("- 2026-10-31 20:41 UTC — с часами"));
    assert_eq!(disk(&b, "T-001").body.matches(LOG_HEADING).count(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn note_errors() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "Текст.");
    assert_eq!(add_note(&mut b, &nreq("  \n "), NOW).unwrap_err().code, 2);
    let mut r = nreq("x");
    r.id = "T-050".into();
    assert_eq!(add_note(&mut b, &r, NOW).unwrap_err().code, 3);
    card(&mut b, "T-002", "backlog", "deleted: true\n", "x");
    r.id = "T-002".into();
    assert_eq!(add_note(&mut b, &r, NOW).unwrap_err().code, 4);
    set_rule(&mut b, "board", "manual");
    let e = add_note(&mut b, &nreq("тихо"), NOW).unwrap_err();
    assert_eq!(e.code, 7);
    assert_eq!(disk(&b, "T-001").body, "Текст.");
    let mut r = nreq("тихо");
    r.user_approved = true;
    assert!(add_note(&mut b, &r, NOW).unwrap().changed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn append_log_cases() {
    assert_eq!(append_log("", "- a"), "## Ход работы\n- a");
    assert_eq!(append_log("Текст.\n\n\n", "- a"), "Текст.\n\n## Ход работы\n- a");
    assert_eq!(append_log("T\n\n## Ход работы\n- a\n\n", "- b"), "T\n\n## Ход работы\n- a\n- b");
    // a section in the middle: the line goes at its end, the rest stays
    assert_eq!(append_log("T\n\n## Ход работы\n- a\n\n## Потом\nхвост", "- b"), "T\n\n## Ход работы\n- a\n- b\n\n## Потом\nхвост");
    // the heading is not created twice
    let once = append_log("T", "- a");
    assert_eq!(append_log(&once, "- b").matches(LOG_HEADING).count(), 1);
    // a `#` that is not a heading does not end the section
    assert_eq!(append_log("## Ход работы\n- a\n#тег\n", "- b"), "## Ход работы\n- a\n#тег\n- b");
}

// ---------------------------------------------------------------- others

#[test]
fn rules_and_parts() {
    let (mut b, root) = fresh();
    let all = rules(&b, &[]).unwrap();
    assert_eq!(all.rules.len(), 6);
    assert!(all.custom.is_none());
    let t = all.render();
    assert!(t.starts_with("## Правила проекта\n- Стиль коммитов: Conventional Commits. Пиши сообщения"), "{t}");
    assert!(!t.contains("## Свои правила"));
    b.cfg.custom_rules = "Отвечай по-русски.\n".into();
    b.commit(&[], true).unwrap();
    let r = rules(&b, &["tests".into(), "custom".into()]).unwrap();
    assert_eq!(r.rules.len(), 1);
    assert_eq!(r.rules[0].id, "tests");
    assert_eq!(r.custom.as_deref(), Some("Отвечай по-русски."));
    assert!(r.render().ends_with("## Свои правила\nОтвечай по-русски.\n"));
    let r = rules(&b, &["merge".into()]).unwrap();
    assert!(r.custom.is_none() && r.rules.len() == 1);
    let e = rules(&b, &["bogus".into()]).unwrap_err();
    assert_eq!(e.code, 2);
    assert!(e.msg.contains("bogus") && e.msg.contains("custom"));
    // trimmed text of a selected option is what agents_md prints
    assert!(rules(&b, &[]).unwrap().rules.iter().all(|r| !r.text.is_empty()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn column_command() {
    let (b, root) = fresh();
    let c = column(&b, "review").unwrap();
    assert_eq!((c.column.pos, c.column.agent.as_str()), (3, "pumba-reviewer"));
    assert_eq!(c.column.next.as_ref().unwrap().id, "done");
    let t = c.render();
    assert!(t.starts_with("col=review «На ревью» (3/5) agent=pumba-reviewer next=done «Готово» last=done\nhash col:review="), "{t}");
    assert!(t.contains("## Перед работой\n") && t.contains("## Работа\n") && t.contains("## После работы\n"));
    // an empty part is left out
    let t = column(&b, "backlog").unwrap().render();
    assert!(t.contains("## Перед работой") && !t.contains("## Работа") && t.contains("## После работы"));
    let e = column(&b, "zzz").unwrap_err();
    assert_eq!(e.code, 3);
    assert!(e.out.len() == 6 && e.out[1].contains("backlog"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unblocks_states() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "in-progress", "", "x");
    card(&mut b, "T-003", "backlog", "", "x");
    card(&mut b, "T-002", "backlog", "depends: T-001\n", "x");
    card(&mut b, "T-004", "backlog", "depends: T-001, T-003\n", "x");
    let u = unblocks(&b, "T-001").unwrap();
    assert!(!u.done);
    assert_eq!(u.dependents.iter().map(|d| d.state.as_str()).collect::<Vec<_>>(), ["pending", "pending"]);
    let t = u.render();
    assert!(t.contains("T-002 «Карточка T-002» — разблокируется, когда T-001 дойдёт до «Готово»\n"), "{t}");
    assert!(t.contains("T-004 «Карточка T-004» — разблокируется, когда T-001 дойдёт до «Готово»; ждёт ещё T-003\n"), "{t}");
    card(&mut b, "T-001", "done", "", "x");
    let t = unblocks(&b, "T-001").unwrap().render();
    assert!(t.contains("T-002 «Карточка T-002» — разблокирована\n") && t.contains("T-004 «Карточка T-004» — ждёт ещё T-003\n"), "{t}");
    assert_eq!(unblocks(&b, "T-002").unwrap().render(), "T-002: никого не блокирует\n");
    assert_eq!(unblocks(&b, "T-099").unwrap_err().code, 3);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rules_hash_stable_and_follows_rules() {
    let (mut b, root) = fresh();
    let h = rules_hash(&b);
    assert_eq!(h.hash.len(), 12);
    assert_eq!(h.render(false), format!("{}\n", h.hash));
    let parts = h.render(true);
    assert!(parts.starts_with(&format!("rules={}\nproto=", h.hash)), "{parts}");
    for name in ["cols=", "col:backlog=", "col:in-progress=", "col:done=", "rule:board=", "rule:commits=", "custom="] {
        assert!(parts.contains(&format!("\n{name}")), "{name}\n{parts}");
    }
    assert_eq!(rules_hash(&b), h, "stable between calls");
    // a card body, a note, a move, the sprint: not rules
    card(&mut b, "T-001", "backlog", "", "Текст.");
    let with_card = rules_hash(&b);
    assert_eq!(with_card, h);
    add_note(&mut b, &nreq("заметка"), NOW).unwrap();
    move_card(&mut b, &mv("in-progress"), NOW).unwrap();
    assert_eq!(rules_hash(&b), h);
    // the board file written to disk and read back gives the same hash
    let reread = open_board(Some(&root), &root).unwrap();
    assert_eq!(rules_hash(&reread), h);
    // a column rule, the selected rule, the column order, the custom rules: rules
    let mut seen = vec![h.hash.clone()];
    let mut check = |b: &mut Board, f: &dyn Fn(&mut Board)| {
        f(b);
        b.commit(&[], true).unwrap();
        let n = rules_hash(b).hash;
        assert!(!seen.contains(&n), "{n} repeats");
        seen.push(n);
    };
    check(&mut b, &|b| b.cfg.columns[2].before.push_str(" Новое условие."));
    check(&mut b, &|b| b.cfg.rules[0].value = "free".into());
    check(&mut b, &|b| b.cfg.columns.swap(1, 2));
    check(&mut b, &|b| b.cfg.custom_rules = "Своё правило".into());
    // and AGENTS.md carries the current one
    let agents = std::fs::read_to_string(b.store.dir.join("AGENTS.md")).unwrap();
    assert!(agents.contains(&format!("<!-- pumba-rules-hash: {} · protocol 1 -->", rules_hash(&b).hash)), "{agents}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn open_board_errors_with_code_9() {
    let dir = std::env::temp_dir().join(format!("pumba-api-nobrd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // there may be a board far above the temp dir on a developer machine: only check the usual case
    let e = open_board(Some(&dir), &dir);
    if let Err(e) = e {
        assert_eq!(e.code, 9);
        assert!(e.msg.contains("доски нет"), "{}", e.msg);
    }
    // a board.json that does not parse
    std::fs::create_dir_all(dir.join(".pumba")).unwrap();
    std::fs::write(dir.join(".pumba/board.json"), "{ not json").unwrap();
    let e = open_board(None, &dir).err().unwrap();
    assert_eq!(e.code, 9);
    // found upward from a subfolder
    std::fs::write(dir.join(".pumba/board.json"), r#"{"columns":[{"id":"a","name":"A"}]}"#).unwrap();
    std::fs::create_dir_all(dir.join("sub/deep")).unwrap();
    let b = open_board(None, &dir.join("sub/deep")).unwrap();
    assert_eq!(b.root(), dir);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stamps() {
    assert_eq!(fmt_stamp(0), "1970-01-01 00:00 UTC");
    assert_eq!(fmt_stamp(NOW), "2026-10-31 20:41 UTC");
    assert_eq!(fmt_stamp(1_709_251_140), "2024-02-29 23:59 UTC");
    assert_eq!(fmt_stamp(951_782_400), "2000-02-29 00:00 UTC");
    for s in [0, 59, 86_399, 951_782_400, 1_709_251_140, NOW, 4_102_444_800] {
        let want = s - s % 60;
        assert_eq!(parse_stamp(&fmt_stamp(s)), Some(want), "{s}");
    }
    assert_eq!(parse_stamp("2026-10-31 20:41"), Some(NOW - NOW % 60));
    for bad in ["", "вчера", "2026-13-01 10:00", "2026-10-31", "2026-10-31 25:00", "31.10 14:05"] {
        assert_eq!(parse_stamp(bad), None, "{bad}");
    }
}

#[test]
fn store_writes_leave_no_temp_files() {
    let (mut b, root) = fresh();
    card(&mut b, "T-001", "backlog", "", "x");
    move_card(&mut b, &mv("in-progress"), NOW).unwrap();
    for dir in [b.store.dir.clone(), b.store.dir.join("tasks")] {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            assert!(!n.contains(".tmp"), "{n}");
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
