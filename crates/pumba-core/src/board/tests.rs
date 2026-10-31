//! Board logic without any UI: the same scenarios the TUI tests drive through keys and forms,
//! here through the `Board` API (apply + commit, the way the TUI form path does it).
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Board on a fresh temp dir with an initialised and loaded `.pumba/`.
fn fresh() -> (Board, PathBuf) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let n = N.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("pumba-board-test-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut b = Board::new(&root);
    assert!(!b.ok);
    b.store.init().unwrap();
    assert_eq!(b.reload(), Ok(None));
    assert!(b.ok);
    (b, root)
}

/// New-card input as the TUI form starts it: the project default model and effort.
fn input(b: &Board, title: &str) -> TaskInput {
    TaskInput { title: title.into(), model: b.cfg.default_model.clone(), effort: b.cfg.default_effort.clone(), ..Default::default() }
}

/// Apply + commit like the TUI form: on any error the memory goes back to the snapshot.
fn save(b: &mut Board, kind: Kind, edit: Option<&str>, ci: usize, i: &TaskInput) -> Result<String, String> {
    let snap = (b.cfg.clone(), b.tasks.clone());
    let r = b.apply_task(kind, edit, ci, i).and_then(|(id, save)| b.commit(&save, false).map(|()| id));
    if r.is_err() {
        (b.cfg, b.tasks) = snap;
    }
    r
}

/// Create a card in column `ci`; returns its id.
fn add_in(b: &mut Board, ci: usize, kind: Kind, title: &str, parent: &str, sprint: &str) -> String {
    let mut i = input(b, title);
    i.parent = Some(parent.to_string());
    i.sprint = sprint.parse().ok();
    save(b, kind, None, ci, &i).unwrap()
}

fn add(b: &mut Board, kind: Kind, title: &str, parent: &str, sprint: &str) -> String {
    add_in(b, 0, kind, title, parent, sprint)
}

/// What the edit form shows for a card: its fields, `depends` and the derived «Блокирует».
fn prefill(b: &Board, id: &str) -> TaskInput {
    let t = &b.tasks[b.find(id).unwrap()];
    TaskInput {
        title: t.title.clone(),
        body: t.body.clone(),
        model: t.model.clone(),
        effort: t.effort.clone(),
        sprint: t.sprint,
        parent: t.parent.clone(),
        depends: t.depends.join(", "),
        blocks: b.dependents(id).iter().map(|d| d.id.as_str()).collect::<Vec<_>>().join(", "),
    }
}

/// Edit a card starting from its prefill.
fn edit(b: &mut Board, id: &str, f: impl FnOnce(&mut TaskInput)) -> Result<(), String> {
    let mut i = prefill(b, id);
    f(&mut i);
    save(b, Kind::Task, Some(id), 0, &i).map(|_| ())
}

fn set_depends(b: &mut Board, id: &str, v: &str) -> Result<(), String> {
    edit(b, id, |i| i.depends = v.into())
}

fn set_blocks(b: &mut Board, id: &str, v: &str) -> Result<(), String> {
    edit(b, id, |i| i.blocks = v.into())
}

fn task<'a>(b: &'a Board, id: &str) -> &'a Task {
    b.tasks.iter().find(|t| t.id == id).unwrap()
}

fn deps(b: &Board, id: &str) -> Vec<String> {
    task(b, id).depends.clone()
}

fn blk(b: &Board, id: &str) -> Vec<String> {
    b.blockers(task(b, id)).iter().map(|x| x.id.clone()).collect()
}

fn ids(b: &Board, ci: usize, view: View) -> Vec<String> {
    b.column_tasks(ci, view).iter().map(|&i| b.tasks[i].id.clone()).collect()
}

fn col_ids(b: &Board) -> Vec<String> {
    b.cfg.columns.iter().map(|c| c.id.clone()).collect()
}

fn col<'a>(b: &'a Board, id: &str) -> &'a crate::model::Column {
    b.cfg.columns.iter().find(|c| c.id == id).unwrap()
}

/// Move a card to column `ci` and write it.
fn mv(b: &mut Board, id: &str, ci: usize) {
    assert!(b.move_card_to(id, ci));
    b.commit(&[id.to_string()], false).unwrap();
}

fn trash(b: &mut Board, id: &str) {
    assert!(b.trash_card(id));
    b.commit(&[id.to_string()], false).unwrap();
}

fn restore(b: &mut Board, id: &str) {
    assert!(b.restore_card(id));
    b.commit(&[id.to_string()], false).unwrap();
}

fn raw(root: &Path, id: &str) -> String {
    std::fs::read_to_string(root.join(format!(".pumba/tasks/{id}.md"))).unwrap()
}

fn agents_file(root: &Path) -> String {
    std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap()
}

fn files_of(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root.join(".pumba/tasks"))
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();
    v.sort();
    v
}

fn column(name: &str) -> ColumnInput {
    ColumnInput { name: name.into(), ..Default::default() }
}

/// Create a column through `apply_column` + commit; returns its index.
fn add_column(b: &mut Board, name: &str) -> usize {
    let at = b.apply_column(None, &column(name)).unwrap().unwrap();
    b.commit(&[], true).unwrap();
    at
}

/// Rewrite a card file behind the board's back after a pause long enough for a new mtime.
fn external_write(root: &Path, id: &str, f: impl Fn(&str) -> String) {
    std::thread::sleep(Duration::from_millis(30));
    let p = root.join(format!(".pumba/tasks/{id}.md"));
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, f(&text)).unwrap();
}

const NOTES: &str = "\n## Ход работы\n- 2026-10-31 14:05 — «В работе» → «На ревью»: проверки зелёные.\n";

fn append_notes(root: &Path, id: &str) {
    external_write(root, id, |t| format!("{}{NOTES}", t.trim_end()));
}

fn disk_task(b: &Board, id: &str) -> Task {
    Task::from_md(&b.store.read_task_raw(id).unwrap(), id).unwrap()
}

const ALL: View = View { show_hidden: false, sprint_only: false };

// ------------------------------------------------------------ ids and creation

#[test]
fn ids_and_creation() {
    let (mut b, root) = fresh();
    assert_eq!(add(&mut b, Kind::Task, "A", "", ""), "T-001");
    assert_eq!(add(&mut b, Kind::Epic, "Epic", "", ""), "E-001");
    assert_eq!(add(&mut b, Kind::Task, "B", "E-001", ""), "T-002");
    assert_eq!(b.tasks[2].parent.as_deref(), Some("E-001"));
    assert_eq!(b.tasks[2].column, "backlog");
    assert!(b.tasks[1].order > b.tasks[0].order);
    // an epic never gets a parent
    let mut i = input(&b, "E2");
    i.parent = Some("E-001".into());
    let e2 = save(&mut b, Kind::Epic, None, 0, &i).unwrap();
    assert_eq!((e2.as_str(), task(&b, &e2).parent.clone()), ("E-002", None));
    // gaps: next id follows the max suffix, not the count
    let mut t = b.tasks[0].clone();
    t.id = "T-010".into();
    b.tasks.push(t);
    assert_eq!(add(&mut b, Kind::Task, "C", "", ""), "T-011");
    assert!(root.join(".pumba/tasks/T-011.md").is_file());
    assert!(agents_file(&root).contains("| T-011 |"));
    // a new card lands at the end of the given column
    assert_eq!(add_in(&mut b, 2, Kind::Task, "D", "", ""), "T-012");
    assert_eq!(task(&b, "T-012").column, "review");

    // validation: empty title is rejected and nothing is added
    let n = b.tasks.len();
    assert_eq!(b.apply_task(Kind::Task, None, 0, &input(&b, "  ")), Err("Нужно указать название".to_string()));
    assert_eq!(b.tasks.len(), n);
    // a column that does not exist
    assert!(b.apply_task(Kind::Task, None, 99, &input(&b, "X")).is_err());
    assert_eq!(b.tasks.len(), n);

    // edit in place: title trimmed, body normalised like `Task::from_md`
    edit(&mut b, "T-001", |i| {
        assert_eq!(i.title, "A");
        i.title = "  A2 ".into();
        i.model = "opus".into();
        i.body = "\n\nText  \n\n".into();
    })
    .unwrap();
    let back = b.store.load().unwrap().1;
    let a = back.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!((a.title.as_str(), a.model.as_str(), a.body.as_str()), ("A2", "opus", "Text"));
    assert_eq!(task(&b, "T-001").body, "Text");
    // a card that is gone cannot be edited
    let e = b.apply_task(Kind::Task, Some("T-099"), 0, &input(&b, "X")).unwrap_err();
    assert_eq!(e, "Задачи больше нет");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_card_id_skips_a_file_created_outside() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    // an agent creates T-002 right now; the board has not noticed yet
    std::fs::write(root.join(".pumba/tasks/T-002.md"), "---\ntitle: Agent card\ncolumn: backlog\n---\n\nmine\n").unwrap();
    assert_eq!(b.new_id(Kind::Task), "T-003");
    let id = add(&mut b, Kind::Task, "B", "", "");
    assert_eq!(id, "T-003");
    assert!(raw(&root, "T-002").contains("Agent card"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn card_created_outside_under_a_known_id_is_not_overwritten() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    // memory holds a card the board never loaded nor wrote, and its file appeared meanwhile
    let mut t = b.tasks[0].clone();
    t.id = "T-009".into();
    b.tasks.push(t);
    std::fs::write(root.join(".pumba/tasks/T-009.md"), "---\ntitle: Theirs\ncolumn: backlog\n---\n\nx\n").unwrap();
    let e = b.commit(&["T-009".to_string()], false).unwrap_err();
    assert_eq!(e, "T-009.md уже есть на диске (создан снаружи); обновите доску");
    assert!(raw(&root, "T-009").contains("Theirs"));
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------------- moving and order

#[test]
fn moving_and_reordering() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    mv(&mut b, "T-001", 1);
    assert_eq!(b.tasks[0].column, "in-progress");
    mv(&mut b, "T-001", 4);
    assert_eq!(b.tasks[0].column, "done");
    // outside the board or an unknown card: nothing changes
    assert!(!b.move_card_to("T-001", 5));
    assert!(!b.move_card_to("NOPE", 1));
    assert_eq!(b.tasks[0].column, "done");
    mv(&mut b, "T-001", 0);
    assert_eq!(ids(&b, 0, ALL), ["T-002", "T-001"]); // moved card goes last
    let w = b.reorder_card(0, ALL, 1, -1).unwrap();
    b.commit(&w, false).unwrap();
    assert_eq!(ids(&b, 0, ALL), ["T-001", "T-002"]);
    let w = b.reorder_card(0, ALL, 0, 1).unwrap();
    b.commit(&w, false).unwrap();
    assert_eq!(ids(&b, 0, ALL), ["T-002", "T-001"]);
    // out of range: nothing to do
    assert!(b.reorder_card(0, ALL, 1, 1).is_none());
    assert!(b.reorder_card(0, ALL, 0, -1).is_none());
    assert!(b.reorder_card(0, ALL, 5, 0).is_none());
    assert!(b.reorder_card(1, ALL, 0, 0).is_none());
    // persisted order survives a reload
    b.reload().unwrap();
    assert_eq!(ids(&b, 0, ALL), ["T-002", "T-001"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reorder_renumbers_and_writes_only_changed_cards() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    // a gap in the orders (an external edit): the column is renumbered 0.. before the swap
    b.tasks[2].order = 7;
    let w = b.reorder_card(0, ALL, 0, 1).unwrap();
    assert_eq!(w, ["T-001", "T-002", "T-003"]);
    assert_eq!(b.tasks.iter().map(|t| t.order).collect::<Vec<_>>(), [1, 0, 2]);
    // a hidden card is skipped by the default view: the visible neighbours swap
    let (mut b2, root2) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b2, Kind::Task, n, "", "");
    }
    assert!(b2.toggle_card_hidden("T-002"));
    let w = b2.reorder_card(0, ALL, 0, 1).unwrap();
    assert_eq!(w, ["T-001", "T-003"]);
    b2.commit(&w, false).unwrap();
    assert_eq!(ids(&b2, 0, ALL), ["T-003", "T-001"]);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

// ------------------------------------------------------- pinned columns

#[test]
fn pinned_columns_do_not_move() {
    let (mut b, root) = fresh();
    let all = col_ids(&b);
    assert_eq!(all.len(), 5);
    assert!(b.is_pinned(0) && b.is_pinned(4) && !b.is_pinned(1) && !b.is_pinned(3));
    for i in [0, 4] {
        for d in [-1, 1] {
            let name = b.cfg.columns[i].name.clone();
            let e = b.move_column_by(i, d).unwrap_err();
            assert_eq!(e, format!("Колонки «{name}» нельзя двигать: первая и последняя закреплены"));
            assert_eq!(col_ids(&b), all);
        }
    }
    // a middle column moves inside the middle only
    assert_eq!(b.move_column_by(1, 1), Ok(2));
    assert_eq!((b.cfg.columns[1].id.as_str(), b.cfg.columns[2].id.as_str()), (all[2].as_str(), all[1].as_str()));
    assert_eq!(b.move_column_by(2, 1), Ok(3));
    // the last middle slot is index 3: one more step would pass the pinned last column
    let name = b.cfg.columns[3].name.clone();
    assert_eq!(b.move_column_by(3, 1), Err(format!("Колонки «{name}» нельзя двигать: первая и последняя закреплены")));
    assert_eq!(b.cfg.columns[4].id, all[4]);
    assert_eq!(b.move_column_by(3, -1), Ok(2));
    assert_eq!(b.move_column_by(2, -1), Ok(1));
    assert_eq!(col_ids(&b), all);
    // and not before the pinned first one
    assert!(b.move_column_by(1, -1).is_err());
    assert!(b.move_column_by(9, 1).is_err());
    assert_eq!(col_ids(&b), all);
    // nothing was written so far
    assert_eq!(b.store.load().unwrap().0.columns.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), all);
    // a move is written to board.json only
    let before = files_of(&root);
    b.move_column_by(1, 1).unwrap();
    b.commit(&[], true).unwrap();
    assert_eq!(b.store.load().unwrap().0.columns[2].id, all[1]);
    assert_eq!(files_of(&root), before);
    // a board of two (both pinned) and of one column
    b.cfg.columns.truncate(2);
    for i in 0..2 {
        for d in [-1, 1] {
            assert!(b.move_column_by(i, d).is_err());
        }
    }
    b.cfg.columns.truncate(1);
    assert!(b.is_pinned(0));
    assert!(b.move_column_by(0, 1).is_err());
    assert_eq!(b.cfg.columns.len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pinned_columns_are_not_deleted() {
    let (mut b, root) = fresh();
    let ids_ = col_ids(&b);
    for i in [0, ids_.len() - 1] {
        let name = b.cfg.columns[i].name.clone();
        let e = b.remove_column(&ids_[i]).unwrap_err();
        assert_eq!(e, format!("Колонку «{name}» нельзя удалить: первая и последняя закреплены"));
        assert_eq!(col_ids(&b), ids_);
    }
    assert_eq!(b.remove_column("ghost"), Err("Колонки больше нет".to_string()));
    // the middle ones go one by one until only the two pinned remain
    while b.cfg.columns.len() > 2 {
        let id = b.cfg.columns[1].id.clone();
        let (pos, moved) = b.remove_column(&id).unwrap();
        assert_eq!((pos, moved.len()), (1, 0));
        b.commit(&moved, true).unwrap();
    }
    assert_eq!(col_ids(&b), [ids_[0].clone(), ids_[ids_.len() - 1].clone()]);
    assert!(b.remove_column(&ids_[ids_.len() - 1]).is_err());
    assert_eq!(b.store.load().unwrap().0.columns.len(), 2);
    // a one-column board keeps its own message
    b.cfg.columns.truncate(1);
    assert_eq!(b.remove_column(&ids_[0]), Err("Нельзя удалить единственную колонку".to_string()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_column_goes_before_the_last() {
    let (mut b, root) = fresh();
    assert_eq!(add_column(&mut b, "QA"), 4);
    assert_eq!(col_ids(&b), ["backlog", "in-progress", "review", "on-dev", "qa", "done"]);
    assert_eq!(add_column(&mut b, "Ops"), 5);
    assert_eq!(col_ids(&b), ["backlog", "in-progress", "review", "on-dev", "qa", "ops", "done"]);
    // ids come from the name and never collide
    assert_eq!(add_column(&mut b, "My Col!"), 6);
    assert_eq!(add_column(&mut b, "my col"), 7);
    assert_eq!(add_column(&mut b, "Колонка"), 8);
    assert_eq!(&col_ids(&b)[6..], ["my-col", "my-col-2", "column", "done"]);
    assert_eq!(b.store.load().unwrap().0.columns[7].id, "my-col-2");
    // a board of two: the new column is the only middle one
    b.cfg.columns.truncate(1);
    let first = b.cfg.columns[0].clone();
    b.cfg.columns.push(first);
    b.cfg.columns[1].id = "fin".into();
    assert_eq!(add_column(&mut b, "Mid"), 1);
    assert_eq!(col_ids(&b), ["backlog", "mid", "fin"]);
    // a single-column board just appends
    b.cfg.columns.truncate(1);
    assert_eq!(add_column(&mut b, "Second"), 1);
    assert_eq!(col_ids(&b), ["backlog", "second"]);
    // old boards: pinning is by position, whatever the ids are
    b.cfg.columns[0].id = "alpha".into();
    b.cfg.columns[1].id = "omega".into();
    assert_eq!(add_column(&mut b, "Between"), 1);
    assert_eq!(col_ids(&b), ["alpha", "between", "omega"]);
    assert!(b.move_column_by(2, -1).is_err());
    assert_eq!(col_ids(&b), ["alpha", "between", "omega"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn columns_and_rules() {
    let (mut b, root) = fresh();
    add_column(&mut b, "My Col!");
    add_column(&mut b, "my col");
    assert_eq!(col_ids(&b), ["backlog", "in-progress", "review", "on-dev", "my-col", "my-col-2", "done"]);
    assert_eq!(b.move_column_by(5, -1), Ok(4));
    b.commit(&[], true).unwrap();
    assert_eq!(b.cfg.columns[4].id, "my-col-2");
    assert_eq!(b.cfg.columns[6].id, "done");
    // deleting a column moves its cards to the first remaining column
    add_in(&mut b, 1, Kind::Task, "A", "", "");
    assert_eq!(b.tasks[0].column, "in-progress");
    let (pos, moved) = b.remove_column("in-progress").unwrap();
    assert_eq!((pos, moved.clone()), (1, vec!["T-001".to_string()]));
    b.commit(&moved, true).unwrap();
    assert_eq!(b.tasks[0].column, "backlog");
    assert_eq!(b.cfg.columns.len(), 6);
    assert_eq!(b.store.load().unwrap().0.columns.len(), 6);
    assert_eq!(b.store.load().unwrap().1[0].column, "backlog");

    // rules: the chosen option and the custom text (trailing whitespace trimmed) are written back
    b.apply_rules(&[("tests".into(), "tdd".into()), ("commits".into(), String::new())], "Be nice\n  \n");
    b.commit(&[], true).unwrap();
    let cfg = b.store.load().unwrap().0;
    assert_eq!(cfg.rules.iter().find(|r| r.id == "tests").unwrap().value, "tdd");
    // an empty value keeps the rule as it was
    assert_eq!(cfg.rules.iter().find(|r| r.id == "commits").unwrap().value, "conventional");
    assert_eq!(cfg.custom_rules, "Be nice");
    assert!(agents_file(&root).contains("Be nice"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn column_input_new_and_edit_agent_and_next() {
    let (mut b, root) = fresh();
    let qa = ColumnInput {
        name: "QA".into(),
        before: "Проверь, что тесты есть.\nНет — остановись.  ".into(),
        rule: "Прогони приёмку.".into(),
        after: "Приёмка прошла — дальше.\n".into(),
        color: "green".into(),
        agent: "  pumba-tester  ".into(),
        next: "review".into(),
    };
    assert_eq!(b.apply_column(None, &qa), Ok(Some(4)));
    b.commit(&[], true).unwrap();
    let c = col(&b, "qa");
    assert_eq!((c.agent.as_str(), c.next.as_str(), c.color.as_str()), ("pumba-tester", "review", "green"));
    // trailing whitespace is trimmed, inner lines are kept
    assert_eq!(
        (c.before.as_str(), c.rule.as_str(), c.after.as_str()),
        ("Проверь, что тесты есть.\nНет — остановись.", "Прогони приёмку.", "Приёмка прошла — дальше.")
    );
    let disk = b.store.load().unwrap().0;
    assert_eq!(disk.columns.iter().find(|c| c.id == "qa").unwrap(), col(&b, "qa"));
    let md = agents_file(&root);
    assert!(md.contains(
        "### QA (`qa`)\n\n**Перед работой:** Проверь, что тесты есть.\nНет — остановись.\n\n**Работа** (исполнитель: `pumba-tester`): Прогони приёмку.\n\n**После работы:** Приёмка прошла — дальше. После успеха: переместить в «На ревью» (`review`)."
    ), "{md}");

    // edit: the id never changes, every field is replaced
    let ip = col(&b, "in-progress").clone();
    let e = ColumnInput {
        name: "Разработка".into(),
        before: ip.before.clone(),
        rule: ip.rule.clone(),
        after: "Только отчёт.".into(),
        color: ip.color.clone(),
        agent: String::new(),
        next: "on-dev".into(),
    };
    assert_eq!(b.apply_column(Some("in-progress"), &e), Ok(None));
    b.commit(&[], true).unwrap();
    let c = col(&b, "in-progress");
    assert_eq!((c.name.as_str(), c.agent.as_str(), c.next.as_str()), ("Разработка", "", "on-dev"));
    assert_eq!((c.before.as_str(), c.rule.as_str(), c.after.as_str()), (ip.before.as_str(), ip.rule.as_str(), "Только отчёт."));
    let disk = b.store.load().unwrap().0;
    assert_eq!(disk.columns.iter().find(|c| c.id == "in-progress").unwrap(), col(&b, "in-progress"));
    // an empty `next` clears the pointer; other columns are untouched
    let mut e2 = e.clone();
    e2.next.clear();
    b.apply_column(Some("in-progress"), &e2).unwrap();
    assert!(col(&b, "in-progress").next.is_empty());
    assert_eq!(col(&b, "backlog").next, "");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn column_input_validation() {
    let (mut b, root) = fresh();
    let before = b.cfg.clone();
    let mut c = column("X");
    c.next = "ghost".into();
    let e = b.apply_column(Some("in-progress"), &c).unwrap_err();
    assert_eq!(e, "Следующая колонка «ghost»: такой колонки нет");
    assert!(b.apply_column(None, &c).is_err());
    c.next = "in-progress".into();
    assert_eq!(b.apply_column(Some("in-progress"), &c), Err("Колонка не может вести сама в себя".to_string()));
    assert_eq!(b.apply_column(None, &column(" ")), Err("Нужно указать название".to_string()));
    assert_eq!(b.apply_column(Some("ghost"), &column("Y")), Err("Колонки больше нет".to_string()));
    assert_eq!(b.cfg, before);
    // a stale pointer (an external edit) is fixed by saving with a valid one
    b.cfg.columns.iter_mut().find(|c| c.id == "review").unwrap().next = "gone".into();
    let r = col(&b, "review").clone();
    let fix = ColumnInput { name: r.name, before: r.before, rule: r.rule, after: r.after, color: r.color, agent: r.agent, next: String::new() };
    b.apply_column(Some("review"), &fix).unwrap();
    assert!(col(&b, "review").next.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn deleting_a_column_clears_pointers_to_it() {
    let (mut b, root) = fresh();
    assert_eq!(col(&b, "in-progress").next, "review");
    let (_, moved) = b.remove_column("review").unwrap();
    b.commit(&moved, true).unwrap();
    assert!(b.cfg.columns.iter().all(|c| c.id != "review"));
    // in-progress pointed at review: cleared; review's own pointer left with it; the rest is intact
    assert_eq!(col(&b, "in-progress").next, "");
    assert_eq!(col(&b, "in-progress").agent, "pumba-worker");
    for c in &b.cfg.columns {
        assert!(c.next.is_empty() || b.cfg.columns.iter().any(|o| o.id == c.next), "{}", c.id);
    }
    let disk = b.store.load().unwrap().0;
    assert_eq!(disk, b.cfg);
    let md = agents_file(&root);
    let ip = md.find("### В работе (`in-progress`)").unwrap();
    let section = &md[ip..ip + md[ip..].find("\n### ").unwrap()];
    assert!(section.contains("**Работа** (исполнитель: `pumba-worker`): "), "{section}");
    assert!(section.contains("в «Ждёт разработчика» (`on-dev`) и остановись. После успеха: остановиться и отчитаться; карточку переносит человек."), "{section}");
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------------------ visibility

#[test]
fn visibility_filters() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "1");
    add(&mut b, Kind::Task, "B", "", "");
    add(&mut b, Kind::Epic, "E", "", "");
    add(&mut b, Kind::Task, "C", "E-001", "1");
    assert_eq!(ids(&b, 0, ALL).len(), 4);
    let shown = View { show_hidden: true, sprint_only: false };
    let sprint = View { show_hidden: false, sprint_only: true };
    // hide B
    assert!(b.toggle_card_hidden("T-002"));
    b.commit(&["T-002".to_string()], false).unwrap();
    assert!(b.store.load().unwrap().1.iter().find(|t| t.id == "T-002").unwrap().hidden);
    assert_eq!(ids(&b, 0, ALL), ["T-001", "E-001", "T-003"]);
    assert_eq!(ids(&b, 0, shown).len(), 4);
    // sprint only: A and C by sprint, epic via its child; B hidden anyway (and has no sprint)
    assert_eq!(ids(&b, 0, sprint), ["T-001", "E-001", "T-003"]);
    assert_eq!(ids(&b, 0, View { show_hidden: true, sprint_only: true }), ["T-001", "E-001", "T-003"]);
    assert!(b.toggle_card_sprint("T-001")); // A leaves the sprint
    assert_eq!(task(&b, "T-001").sprint, None);
    assert_eq!(ids(&b, 0, sprint), ["E-001", "T-003"]);
    assert!(b.toggle_card_sprint("T-003"));
    assert!(ids(&b, 0, sprint).is_empty());
    // toggling again puts the card into the current sprint
    assert!(b.toggle_card_sprint("T-001"));
    assert_eq!(task(&b, "T-001").sprint, Some(1));
    assert!(!b.toggle_card_sprint("NOPE") && !b.toggle_card_hidden("NOPE"));
    assert!(b.toggle_card_sprint("T-003"));
    assert_eq!(ids(&b, 0, sprint), ["T-001", "E-001", "T-003"]);
    // a trashed card is never visible, a trashed child does not pull its epic in
    assert!(b.trash_card("T-003"));
    assert!(!b.visible(task(&b, "T-003"), View { show_hidden: true, sprint_only: false }));
    assert_eq!(ids(&b, 0, sprint), ["T-001"]);
    // a card of an unknown column shows in the first one
    b.tasks.iter_mut().find(|t| t.id == "T-002").unwrap().column = "gone".into();
    assert_eq!(b.effective_col(task(&b, "T-002")), 0);
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------- epic progress and sprints

#[test]
fn epic_progress_and_sprint_rollover() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Epic, "E", "", "");
    add(&mut b, Kind::Task, "A", "E-001", "1");
    add(&mut b, Kind::Task, "B", "E-001", "1");
    add(&mut b, Kind::Task, "C", "E-001", "1");
    assert_eq!(b.epic_progress("E-001"), (0, 3));
    assert_eq!(b.epic_progress("NOPE"), (0, 0));
    mv(&mut b, "T-001", 4);
    assert_eq!(b.epic_progress("E-001"), (1, 3));
    let moved = b.next_sprint();
    assert_eq!(moved, ["T-002", "T-003"]);
    b.commit(&moved, true).unwrap();
    assert_eq!(b.cfg.sprint, 2);
    let sprint = |id: &str| task(&b, id).sprint;
    assert_eq!((sprint("T-001"), sprint("T-002"), sprint("E-001")), (Some(1), Some(2), None));
    assert_eq!(b.store.load().unwrap().0.sprint, 2);
    assert!(raw(&root, "T-002").contains("sprint: 2\n"));
    let _ = std::fs::remove_dir_all(root);
}

// ----------------------------------------------------------------- trash

#[test]
fn soft_delete_keeps_file_and_hides_card() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "Первая", "", "");
    add(&mut b, Kind::Task, "Вторая", "", "");
    trash(&mut b, "T-001");
    // a second trash of the same card does nothing
    assert!(!b.trash_card("T-001") && !b.trash_card("NOPE"));
    assert!(root.join(".pumba/tasks/T-001.md").is_file());
    let md = raw(&root, "T-001");
    assert!(md.contains("deleted: true\n") && md.contains("deleted_at: "), "{md}");
    assert_eq!(ids(&b, 0, ALL), ["T-002"]);
    // never shown, even with hidden cards revealed
    assert_eq!(ids(&b, 0, View { show_hidden: true, sprint_only: false }), ["T-002"]);
    assert_eq!(b.trashed().iter().map(|&i| b.tasks[i].id.clone()).collect::<Vec<_>>(), ["T-001"]);
    // survives a reload; AGENTS.md has no index row for it
    b.reload().unwrap();
    assert_eq!(ids(&b, 0, ALL), ["T-002"]);
    assert!(task(&b, "T-001").deleted_at.is_some());
    let agents = agents_file(&root);
    assert!(agents.contains("| T-002 |") && !agents.contains("| T-001 |"));
    assert!(agents.contains("`deleted: true`") && agents.contains("полностью игнорируй"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trash_order_and_restore() {
    let (mut b, root) = fresh();
    assert!(b.trashed().is_empty());
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    for id in ["T-001", "T-002", "T-003"] {
        trash(&mut b, id);
    }
    // newest deleted first; no timestamp sorts last, ties by id
    for (id, v) in [("T-001", Some(300)), ("T-002", Some(100)), ("T-003", Some(200))] {
        b.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = v;
    }
    let order = |b: &Board| b.trashed().iter().map(|&i| b.tasks[i].id.clone()).collect::<Vec<_>>();
    assert_eq!(order(&b), ["T-001", "T-003", "T-002"]);
    b.tasks.iter_mut().find(|t| t.id == "T-001").unwrap().deleted_at = None;
    b.tasks.iter_mut().find(|t| t.id == "T-003").unwrap().deleted_at = Some(100);
    assert_eq!(order(&b), ["T-002", "T-003", "T-001"]);
    assert!(ids(&b, 0, ALL).is_empty());
    // restore: back on the board, the trash keys are gone from the file
    restore(&mut b, "T-002");
    assert_eq!(order(&b), ["T-003", "T-001"]);
    assert_eq!(ids(&b, 0, ALL), ["T-002"]);
    let md = raw(&root, "T-002");
    assert!(!md.contains("deleted"), "{md}");
    assert!(task(&b, "T-002").deleted_at.is_none());
    assert!(!b.restore_card("NOPE"));
    // restored into a missing column: lands in the first column
    b.tasks.iter_mut().find(|t| t.id == "T-001").unwrap().column = "gone".into();
    restore(&mut b, "T-001");
    assert_eq!(b.effective_col(task(&b, "T-001")), 0);
    assert!(ids(&b, 0, ALL).contains(&"T-001".to_string()));
    // nothing purges a file
    for id in ["T-001", "T-002", "T-003"] {
        assert!(root.join(format!(".pumba/tasks/{id}.md")).is_file());
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ids_are_not_reused_after_trash() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    add(&mut b, Kind::Epic, "E", "", "");
    trash(&mut b, "T-002");
    trash(&mut b, "E-001");
    assert_eq!(add(&mut b, Kind::Task, "C", "", ""), "T-003");
    assert_eq!(add(&mut b, Kind::Epic, "F", "", ""), "E-002");
    assert!(raw(&root, "T-002").contains("deleted: true"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trashed_cards_follow_column_changes() {
    let (mut b, root) = fresh();
    add_in(&mut b, 1, Kind::Task, "A", "", "");
    trash(&mut b, "T-001");
    let (_, moved) = b.remove_column("in-progress").unwrap();
    assert_eq!(moved, ["T-001"]);
    b.commit(&moved, true).unwrap();
    let t = task(&b, "T-001");
    assert!(t.deleted);
    assert_eq!(t.column, "backlog");
    assert_eq!(b.store.load().unwrap().1[0].column, "backlog");
    // swapping columns never touches card files
    let before = raw(&root, "T-001");
    assert_eq!(b.move_column_by(1, 1), Ok(2));
    b.commit(&[], true).unwrap();
    assert_eq!(task(&b, "T-001").column, "backlog");
    assert_eq!(raw(&root, "T-001"), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epic_soft_delete_keeps_children() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Epic, "E", "", "");
    add(&mut b, Kind::Task, "A", "E-001", "1");
    add(&mut b, Kind::Task, "B", "E-001", "1");
    mv(&mut b, "T-001", 4);
    assert_eq!(b.epic_progress("E-001"), (1, 2));
    // trashing a child leaves the epic with one child
    trash(&mut b, "T-002");
    assert_eq!(b.epic_progress("E-001"), (1, 1));
    trash(&mut b, "E-001");
    assert!(raw(&root, "E-001").contains("deleted: true"));
    for id in ["T-001", "T-002"] {
        assert!(raw(&root, id).contains("parent: E-001\n"), "{id}");
    }
    // the child still shows with its parent link
    assert_eq!(ids(&b, 4, ALL), ["T-001"]);
    assert_eq!(task(&b, "T-001").parent.as_deref(), Some("E-001"));
    // an edit keeps the link to the trashed epic
    edit(&mut b, "T-001", |i| i.title = "A2".into()).unwrap();
    assert_eq!(task(&b, "T-001").parent.as_deref(), Some("E-001"));
    // rollover skips the trashed card
    let moved = b.next_sprint();
    b.commit(&moved, true).unwrap();
    assert_eq!((task(&b, "T-001").sprint, task(&b, "T-002").sprint), (Some(1), Some(1)));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_edit_reload_and_delete() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Epic, "E", "", "");
    add(&mut b, Kind::Task, "A", "E-001", "");
    let p = root.join(".pumba/tasks/T-001.md");
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replace("column: backlog", "column: done")).unwrap();
    b.reload().unwrap();
    assert_eq!(b.effective_col(task(&b, "T-001")), 4);
    // trashing the epic keeps its file and the children's parent
    trash(&mut b, "E-001");
    assert!(root.join(".pumba/tasks/E-001.md").exists());
    let back = b.store.load().unwrap().1;
    assert_eq!(back.iter().find(|t| t.id == "T-001").unwrap().parent.as_deref(), Some("E-001"));
    assert!(back.iter().find(|t| t.id == "E-001").unwrap().deleted);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------------- depends / blocks

#[test]
fn blockers_and_depends() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    add(&mut b, Kind::Epic, "E", "", "");
    let e = set_depends(&mut b, "T-002", "T-001, E-001, ZZZ-9").unwrap_err(); // unknown id rejected
    assert_eq!(e, "Зависимость ZZZ-9: такой карточки нет");
    set_depends(&mut b, "T-002", "T-001, E-001, T-001").unwrap(); // duplicate dropped silently
    assert_eq!(deps(&b, "T-002"), ["T-001", "E-001"]);
    assert_eq!(blk(&b, "T-002"), ["T-001", "E-001"]);
    // unknown ids (external edit) are ignored
    let mut ghost = task(&b, "T-002").clone();
    ghost.depends = vec!["NOPE".into()];
    assert!(b.blockers(&ghost).is_empty());
    // finishing T-001 leaves only the epic
    mv(&mut b, "T-001", 4);
    assert_eq!(blk(&b, "T-002"), ["E-001"]);
    mv(&mut b, "E-001", 4);
    assert!(blk(&b, "T-002").is_empty());
    mv(&mut b, "E-001", 3);
    assert_eq!(blk(&b, "T-002"), ["E-001"]);
    b.reload().unwrap();
    assert_eq!(deps(&b, "T-002"), ["T-001", "E-001"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn parse_depends_and_blocks() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    assert_eq!(b.parse_depends(" T-001,,T-002  T-001, ", None), Ok(vec!["T-001".to_string(), "T-002".to_string()]));
    assert_eq!(b.parse_depends("", Some("T-001")), Ok(vec![]));
    assert_eq!(b.parse_depends("T-001", Some("T-001")), Err("T-001 не может зависеть от самой себя".to_string()));
    assert_eq!(b.parse_blocks("T-002,T-003 T-002", None, &[]), Ok(vec!["T-002".to_string(), "T-003".to_string()]));
    assert_eq!(b.parse_blocks("T-001", Some("T-001"), &[]), Err("T-001 не может блокировать саму себя".to_string()));
    assert_eq!(
        b.parse_blocks("T-002", Some("T-001"), &["T-002".to_string()]),
        Err("T-002: нельзя одновременно зависеть от карточки и блокировать её".to_string())
    );
    assert_eq!(b.parse_blocks("ZZZ-1", None, &[]), Err("Блокирует ZZZ-1: такой карточки нет".to_string()));
    trash(&mut b, "T-003");
    assert_eq!(b.parse_depends("T-003", Some("T-001")), Err("Зависимость T-003: карточка в корзине".to_string()));
    assert_eq!(b.parse_blocks("T-003", Some("T-001"), &[]), Err("Блокирует T-003: карточка в корзине".to_string()));
    // check_cycles sees the edges of the pending change only
    assert_eq!(b.check_cycles("T-001", &["T-002".to_string()], &[], &[]), Ok(()));
    assert_eq!(
        b.check_cycles("T-001", &["T-002".to_string()], &["T-002".to_string()], &[]),
        Err("Цикл зависимостей: T-001 → T-002 → T-001".to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn self_and_cycle_rejected() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    let e = set_depends(&mut b, "T-001", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("самой себя"), "{e}");
    set_depends(&mut b, "T-001", "T-002").unwrap();
    set_depends(&mut b, "T-002", "T-003").unwrap();
    let e = set_depends(&mut b, "T-003", "T-001").unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-003 → T-001 → T-002 → T-003"), "{e}");
    // T-001 already depends on T-002 (prefilled in T-002's «Блокирует»): the same id in both fields is rejected
    let e = set_depends(&mut b, "T-002", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("одновременно"), "{e}");
    assert!(deps(&b, "T-003").is_empty());
    set_depends(&mut b, "T-001", "T-002, T-003").unwrap(); // a diamond is fine
    let back = b.store.load().unwrap().1;
    assert!(back.iter().find(|t| t.id == "T-003").unwrap().depends.is_empty());
    // a rejected apply changes nothing in memory
    add(&mut b, Kind::Task, "D", "", "");
    set_depends(&mut b, "T-004", "T-001").unwrap();
    let snap = b.tasks.clone();
    let mut i = prefill(&b, "T-003");
    i.depends = "T-004".into();
    assert!(b.apply_task(Kind::Task, Some("T-003"), 0, &i).unwrap_err().contains("Цикл"));
    assert_eq!(b.tasks, snap);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn depends_roundtrip_and_trash_keeps_depends() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    set_depends(&mut b, "T-003", "T-001,T-002").unwrap();
    set_depends(&mut b, "T-002", "T-001").unwrap();
    assert_eq!(prefill(&b, "T-003").depends, "T-001, T-002");
    // epics get the field too
    add(&mut b, Kind::Epic, "E", "", "");
    assert_eq!(prefill(&b, "E-001").depends, "");
    let mut i = prefill(&b, "E-001");
    i.depends = "T-001".into();
    save(&mut b, Kind::Task, Some("E-001"), 0, &i).unwrap();
    assert_eq!(deps(&b, "E-001"), ["T-001"]);
    set_depends(&mut b, "E-001", "").unwrap();
    assert!(raw(&root, "T-003").contains("depends: T-001, T-002\n"));
    b.reload().unwrap();
    assert_eq!(deps(&b, "T-003"), ["T-001", "T-002"]);
    // clearing the field removes the key
    set_depends(&mut b, "T-002", "").unwrap();
    assert!(!raw(&root, "T-002").contains("depends"));
    set_depends(&mut b, "T-002", "T-001").unwrap();

    // trashing T-001 removes nothing from other cards, in memory or on disk
    trash(&mut b, "T-001");
    assert_eq!(deps(&b, "T-003"), ["T-001", "T-002"]);
    assert_eq!(deps(&b, "T-002"), ["T-001"]);
    let back = b.store.load().unwrap().1;
    assert_eq!(back.iter().find(|t| t.id == "T-003").unwrap().depends, ["T-001", "T-002"]);
    assert!(root.join(".pumba/tasks/T-001.md").is_file());
    let agents = agents_file(&root);
    assert!(agents.contains("| T-003 | task | backlog | - | T-001, T-002 | - | C |"), "{agents}");
    assert!(!agents.contains("| T-001 | task"), "{agents}");
    assert!(agents.contains("ОБЯЗАН ОТКАЗАТЬСЯ"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trashed_dependency_does_not_block_and_survives() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    add(&mut b, Kind::Task, "C", "", "");
    set_depends(&mut b, "T-002", "T-001").unwrap();
    assert_eq!(blk(&b, "T-002"), ["T-001"]);
    trash(&mut b, "T-001");
    assert!(blk(&b, "T-002").is_empty());
    assert!(raw(&root, "T-002").contains("depends: T-001\n"));
    // a stored entry survives an edit that keeps it...
    set_depends(&mut b, "T-002", "T-001").unwrap();
    assert_eq!(deps(&b, "T-002"), ["T-001"]);
    assert!(raw(&root, "T-002").contains("depends: T-001\n"));
    // ...but a trashed id cannot be newly picked
    let e = set_depends(&mut b, "T-003", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("корзине"), "{e}");
    assert!(deps(&b, "T-003").is_empty());
    // restore: the card is back and blocks again
    restore(&mut b, "T-001");
    assert_eq!(blk(&b, "T-002"), ["T-001"]);
    assert!(!raw(&root, "T-001").contains("deleted"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dependents_are_sorted_and_skip_trash() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C", "D"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    set_depends(&mut b, "T-004", "T-001").unwrap();
    set_depends(&mut b, "T-002", "T-001, T-003").unwrap();
    let ids = |b: &Board, id: &str| b.dependents(id).iter().map(|t| t.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&b, "T-001"), ["T-002", "T-004"]);
    assert_eq!(ids(&b, "T-003"), ["T-002"]);
    assert!(ids(&b, "T-002").is_empty() && ids(&b, "NOPE").is_empty());
    trash(&mut b, "T-002");
    assert_eq!(ids(&b, "T-001"), ["T-004"]);
    assert!(ids(&b, "T-003").is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn blocks_reverse_edges_on_disk() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    // B keeps its other fields and its own depends when A starts blocking it
    edit(&mut b, "T-002", |i| {
        i.body = "Body of B".into();
        i.depends = "T-003".into();
        i.effort = "high".into();
        i.sprint = Some(1);
    })
    .unwrap();
    let before_b = raw(&root, "T-002");
    assert!(before_b.contains("depends: T-003\n"));
    assert_eq!(prefill(&b, "T-001").blocks, "");

    set_blocks(&mut b, "T-001", "T-002, T-003, T-002").unwrap(); // duplicate dropped silently
    assert_eq!(deps(&b, "T-002"), ["T-003", "T-001"]);
    assert_eq!(deps(&b, "T-003"), ["T-001"]);
    assert!(deps(&b, "T-001").is_empty());
    // on disk: no `blocks` key anywhere, other fields of B untouched
    assert!(files_of(&root).iter().all(|s| !s.contains("blocks")));
    let after_b = raw(&root, "T-002");
    assert_eq!(after_b, before_b.replace("depends: T-003\n", "depends: T-003, T-001\n"));
    assert!(after_b.contains("effort: high\n") && after_b.contains("sprint: 1\n") && after_b.contains("Body of B"));
    assert!(raw(&root, "T-003").contains("depends: T-001\n"));
    assert_eq!(blk(&b, "T-003"), ["T-001"]);
    assert!(agents_file(&root).contains("| T-001 | task | backlog | - | - | T-002, T-003 | A |"));
    b.reload().unwrap();
    assert_eq!(deps(&b, "T-002"), ["T-003", "T-001"]);
    // prefill comes from the dependents; the dependent's own prefill shows the other side
    assert_eq!(prefill(&b, "T-001").blocks, "T-002, T-003");
    let p3 = prefill(&b, "T-003");
    assert_eq!((p3.depends.as_str(), p3.blocks.as_str()), ("T-001", "T-002"));

    // removing reverses it: only the removed card changes
    set_blocks(&mut b, "T-001", "T-003").unwrap();
    assert_eq!(deps(&b, "T-002"), ["T-003"]);
    assert!(raw(&root, "T-002").contains("depends: T-003\n"));
    assert_eq!(deps(&b, "T-003"), ["T-001"]);
    // clearing the last edge drops the depends key of that card
    set_blocks(&mut b, "T-001", "").unwrap();
    assert!(deps(&b, "T-003").is_empty());
    assert!(!raw(&root, "T-003").contains("depends"));
    assert!(blk(&b, "T-003").is_empty());
    // re-adding after a reload works the same (the file is the single source of truth)
    b.reload().unwrap();
    set_blocks(&mut b, "T-001", "T-003").unwrap();
    assert!(raw(&root, "T-003").contains("depends: T-001\n"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn blocks_validation_and_cycles() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    let clean = files_of(&root);
    // same id in both fields, self, unknown
    let e = edit(&mut b, "T-001", |i| {
        i.depends = "T-002".into();
        i.blocks = "T-002".into();
    })
    .unwrap_err();
    assert!(e.contains("T-002") && e.contains("одновременно"), "{e}");
    let e = set_blocks(&mut b, "T-001", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("саму себя"), "{e}");
    let e = set_blocks(&mut b, "T-001", "T-002, ZZZ-9").unwrap_err();
    assert!(e.contains("ZZZ-9") && e.contains("нет"), "{e}");
    assert_eq!(files_of(&root), clean);
    assert!(b.tasks.iter().all(|t| t.depends.is_empty()));

    // cycle through the combination: T-001 -> T-002 -> T-003, then T-001 blocks T-003
    set_depends(&mut b, "T-001", "T-002").unwrap();
    set_depends(&mut b, "T-002", "T-003").unwrap();
    let clean = files_of(&root);
    let e = set_blocks(&mut b, "T-001", "T-003").unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-001 → T-002 → T-003 → T-001"), "{e}");
    assert!(deps(&b, "T-003").is_empty());
    assert_eq!(files_of(&root), clean);
    // the same ring from T-003: it depends on T-001 while T-002 (prefilled in «Блокирует») depends on it
    let e = set_depends(&mut b, "T-003", "T-001").unwrap_err();
    assert!(e.contains("Цикл"), "{e}");
    assert_eq!(files_of(&root), clean);
    // flipping an edge is not a cycle: T-001 stops depending on T-002, T-002 depends on T-001 instead
    edit(&mut b, "T-001", |i| {
        i.depends = String::new();
        i.blocks = "T-002".into();
    })
    .unwrap();
    assert!(deps(&b, "T-001").is_empty());
    assert_eq!(deps(&b, "T-002"), ["T-003", "T-001"]);

    // via A's depends after A blocks B and B blocks C
    let (mut b, root2) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    set_blocks(&mut b, "T-001", "T-002").unwrap();
    set_blocks(&mut b, "T-002", "T-003").unwrap();
    assert_eq!((deps(&b, "T-002"), deps(&b, "T-003")), (vec!["T-001".to_string()], vec!["T-002".to_string()]));
    let clean = files_of(&root2);
    let e = set_depends(&mut b, "T-001", "T-003").unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-001 → T-003 → T-002 → T-001"), "{e}");
    // B blocks A via B's prefill: B already depends on A, so the same id is rejected
    let e = set_blocks(&mut b, "T-002", "T-001, T-003").unwrap_err();
    assert!(e.contains("T-001") && e.contains("одновременно"), "{e}");
    // C blocks A closes the ring
    let e = set_blocks(&mut b, "T-003", "T-001").unwrap_err();
    assert!(e.contains("Цикл"), "{e}");
    assert_eq!(files_of(&root2), clean);
    assert!(deps(&b, "T-001").is_empty());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn blocks_trash_rules() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C", "D"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    set_blocks(&mut b, "T-001", "T-002").unwrap();
    trash(&mut b, "T-002");
    trash(&mut b, "T-004");
    // trashed dependent: not in the prefill, not counted
    assert_eq!(prefill(&b, "T-001").blocks, "");
    assert!(b.dependents("T-001").is_empty());
    // a new trashed pick is rejected; the existing trashed edge may be typed again without a change
    let e = set_blocks(&mut b, "T-001", "T-004").unwrap_err();
    assert!(e.contains("T-004") && e.contains("корзине"), "{e}");
    edit(&mut b, "T-001", |i| {
        i.blocks = "T-002, T-003".into();
        i.title = "A2".into();
    })
    .unwrap();
    // the trashed dependent stays untouched even when it is absent from the field
    set_blocks(&mut b, "T-001", "T-003").unwrap();
    assert_eq!(deps(&b, "T-002"), ["T-001"]);
    assert!(raw(&root, "T-002").contains("depends: T-001\n") && raw(&root, "T-002").contains("deleted: true"));
    assert_eq!(deps(&b, "T-003"), ["T-001"]);
    // clearing the field removes only live dependents
    set_blocks(&mut b, "T-001", "").unwrap();
    assert!(deps(&b, "T-003").is_empty());
    assert_eq!(deps(&b, "T-002"), ["T-001"]);
    // restore: the card is a dependent again
    restore(&mut b, "T-002");
    assert_eq!(b.dependents("T-001").len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_card_with_blocks() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    add(&mut b, Kind::Task, "C", "", "");
    set_depends(&mut b, "T-002", "T-001").unwrap();
    // the id exists only at save: the reverse edges name it
    let mut i = input(&b, "N");
    i.depends = "T-003".into();
    i.blocks = "T-001".into();
    let (id, w) = b.apply_task(Kind::Task, None, 0, &i).unwrap();
    assert_eq!((id.as_str(), w), ("T-004", vec!["T-004".to_string(), "T-001".to_string()]));
    b.commit(&["T-004".to_string(), "T-001".to_string()], false).unwrap();
    assert_eq!(deps(&b, "T-001"), ["T-004"]);
    assert_eq!(deps(&b, "T-004"), ["T-003"]);
    assert!(raw(&root, "T-001").contains("depends: T-004\n"));
    assert!(raw(&root, "T-004").contains("depends: T-003\n"));
    assert_eq!(b.dependents("T-004").iter().map(|t| t.id.clone()).collect::<Vec<_>>(), ["T-001"]);
    // a cycle through the not-yet-existing card: M depends on B, B depends on A, M blocks A
    let n = b.tasks.len();
    let clean = files_of(&root);
    let mut m = input(&b, "M");
    m.depends = "T-002".into();
    m.blocks = "T-001".into();
    let e = save(&mut b, Kind::Task, None, 0, &m).unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-005"), "{e}");
    assert_eq!((b.tasks.len(), files_of(&root)), (n, clean));
    assert!(!root.join(".pumba/tasks/T-005.md").exists());
    // unknown / trashed ids cannot be picked for a new card either
    let mut m = input(&b, "M");
    m.blocks = "ZZZ-1".into();
    assert!(save(&mut b, Kind::Task, None, 0, &m).is_err());
    trash(&mut b, "T-003");
    m.blocks = "T-003".into();
    let e = save(&mut b, Kind::Task, None, 0, &m).unwrap_err();
    assert!(e.contains("корзине"), "{e}");
    // epics get the field and the same behaviour
    let mut ep = input(&b, "Эпик");
    ep.blocks = "T-002".into();
    assert_eq!(save(&mut b, Kind::Epic, None, 0, &ep).unwrap(), "E-001");
    assert_eq!(deps(&b, "T-002"), ["T-001", "E-001"]);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------------ rollback on IO error

#[test]
fn blocks_save_rolls_back_files_on_io_error() {
    let (mut b, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut b, Kind::Task, n, "", "");
    }
    let clean = (raw(&root, "T-001"), raw(&root, "T-002"));
    let agents = agents_file(&root);
    let w = b.agents_writes;
    // T-003 cannot be written: its file name is taken by a directory
    let p3 = root.join(".pumba/tasks/T-003.md");
    std::fs::remove_file(&p3).unwrap();
    std::fs::create_dir(&p3).unwrap();
    let e = edit(&mut b, "T-001", |i| {
        i.title = "A changed".into();
        i.blocks = "T-002, T-003".into();
    })
    .unwrap_err();
    // memory (restored by the caller) and disk as before: T-001, T-002 were written first, then restored
    assert!(e.starts_with("Ошибка записи"), "{e}");
    assert_eq!(b.tasks[0].title, "A");
    assert!(deps(&b, "T-002").is_empty() && deps(&b, "T-003").is_empty());
    assert_eq!((raw(&root, "T-001"), raw(&root, "T-002")), clean);
    assert_eq!(agents_file(&root), agents);
    assert_eq!(b.agents_writes, w);
    assert!(b.ok);
    // a new card whose save fails leaves no file behind
    let mut i = input(&b, "N");
    i.blocks = "T-002, T-003".into();
    assert!(save(&mut b, Kind::Task, None, 0, &i).is_err());
    assert!(!root.join(".pumba/tasks/T-004.md").exists());
    assert_eq!(b.tasks.len(), 3);
    assert_eq!(raw(&root, "T-002"), clean.1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn broken_board_json_is_never_overwritten() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    let p = root.join(".pumba/board.json");
    std::fs::write(&p, "{ not json").unwrap();
    let e = b.reload().unwrap_err();
    assert!(e.starts_with("Не удалось загрузить .pumba: "), "{e}");
    assert!(!b.ok && b.unreadable.is_empty());
    assert_eq!(b.apply_column(None, &column("X")), Ok(Some(4)));
    let e = b.commit(&[], true).unwrap_err();
    assert_eq!(e, "board.json не удалось загрузить; сначала исправьте его (ничего не записано)");
    assert!(b.write(&["T-001".to_string()], false).is_err());
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ not json");
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------- reload keeps AGENTS.md fresh

#[test]
fn reload_regenerates_agents_md_once_after_an_external_move() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    assert!(agents_file(&root).contains("| T-001 | task | backlog |"));
    let w = b.agents_writes;
    assert!(!b.changed_on_disk());
    // an agent moves the card: the change is noticed and a reload refreshes the index
    external_write(&root, "T-001", |t| t.replace("column: backlog", "column: in-progress"));
    assert!(b.changed_on_disk());
    assert_eq!(b.reload(), Ok(None));
    assert_eq!(b.agents_writes, w + 1);
    assert!(agents_file(&root).contains("| T-001 | task | in-progress |"));
    assert_eq!(b.tasks[0].column, "in-progress");
    // our own AGENTS.md write is invisible to the signature
    assert!(!b.changed_on_disk());
    // an unchanged reload does not write
    let stamp = std::fs::metadata(root.join(".pumba/AGENTS.md")).unwrap().modified().unwrap();
    b.reload().unwrap();
    assert_eq!(b.agents_writes, w + 1);
    assert_eq!(std::fs::metadata(root.join(".pumba/AGENTS.md")).unwrap().modified().unwrap(), stamp);
    // a new external card is picked up and indexed once
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(root.join(".pumba/tasks/T-050.md"), "---\ntitle: From agent\ncolumn: backlog\n---\n\nb\n").unwrap();
    assert!(b.changed_on_disk());
    b.reload().unwrap();
    assert_eq!(b.agents_writes, w + 2);
    assert!(agents_file(&root).contains("| T-050 | task | backlog |"));
    // an external board.json edit (a new rule text) too
    std::thread::sleep(Duration::from_millis(30));
    let p = root.join(".pumba/board.json");
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replace("\"sprint\": 1", "\"sprint\": 4")).unwrap();
    assert!(b.changed_on_disk());
    b.reload().unwrap();
    assert_eq!((b.agents_writes, b.cfg.sprint), (w + 3, 4));
    assert!(agents_file(&root).contains("Текущий спринт: 4"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reload_does_not_write_after_a_failed_load() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    let w = b.agents_writes;
    let md = agents_file(&root);
    let p = root.join(".pumba/board.json");
    let good = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, "{ broken").unwrap();
    assert!(b.reload().is_err());
    assert!(!b.ok);
    assert_eq!((b.agents_writes, agents_file(&root)), (w, md.clone()));
    // fixed again, even with a changed card: the previous load failed, so nothing is written yet
    external_write(&root, "T-001", |t| t.replace("column: backlog", "column: done"));
    std::fs::write(&p, good).unwrap();
    assert_eq!(b.reload(), Ok(None));
    assert!(b.ok);
    assert_eq!((b.agents_writes, agents_file(&root)), (w, md));
    // the next change is indexed again
    external_write(&root, "T-001", |t| t.replace("column: done", "column: review"));
    assert!(b.changed_on_disk());
    b.reload().unwrap();
    assert_eq!(b.agents_writes, w + 1);
    assert!(agents_file(&root).contains("| T-001 | task | review |"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn empty_column_list_falls_back_to_defaults() {
    let (mut b, root) = fresh();
    let p = root.join(".pumba/board.json");
    let text = std::fs::read_to_string(&p).unwrap();
    let mut cfg: crate::model::Config = serde_json::from_str(&text).unwrap();
    cfg.columns.clear();
    std::fs::write(&p, serde_json::to_string(&cfg).unwrap()).unwrap();
    b.reload().unwrap();
    assert_eq!(b.cfg.columns, crate::model::Config::default().columns);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------------- unreadable cards

#[test]
fn unreadable_cards_are_reported_not_hidden() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    assert!(b.unreadable.is_empty() && b.unreadable_note().is_none());
    let tasks = root.join(".pumba/tasks");
    std::fs::write(tasks.join("T-007.md"), "no frontmatter").unwrap();
    b.reload().unwrap();
    assert_eq!(b.unreadable, ["T-007.md"]);
    assert_eq!(b.unreadable_note().as_deref(), Some("Не прочитано: T-007.md"));
    assert_eq!(b.tasks.len(), 1);
    // more than three: the first three names and the rest as a count
    for n in ["T-008", "T-009", "T-010", "T-011"] {
        std::fs::write(tasks.join(format!("{n}.md")), [0xffu8, 0xfe]).unwrap();
    }
    b.reload().unwrap();
    assert_eq!(b.unreadable_note().as_deref(), Some("Не прочитано: T-007.md, T-008.md, T-009.md и ещё 2"));
    // a card with a BOM and CRLF is read, not reported
    std::fs::write(tasks.join("T-020.md"), "\u{feff}---\r\ntitle: Win\r\ncolumn: backlog\r\n---\r\n\r\nbody\r\n").unwrap();
    for n in ["T-007", "T-008", "T-009", "T-010", "T-011"] {
        std::fs::remove_file(tasks.join(format!("{n}.md"))).unwrap();
    }
    b.reload().unwrap();
    assert!(b.unreadable.is_empty() && b.unreadable_note().is_none());
    assert!(b.tasks.iter().any(|t| t.id == "T-020" && t.title == "Win"));
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ lost-update protection

#[test]
fn card_move_keeps_text_appended_outside() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    append_notes(&root, "T-001");
    // no reload in between: the board still holds the old body
    assert!(!b.tasks[0].body.contains("Ход работы"));
    mv(&mut b, "T-001", 1);
    let t = disk_task(&b, "T-001");
    assert_eq!(t.column, "in-progress");
    assert!(t.body.contains("## Ход работы") && t.body.contains("проверки зелёные"), "{}", t.body);
    // memory caught up as well
    assert!(task(&b, "T-001").body.contains("Ход работы"));
    // every other operation that only rewrites the card keeps it too: hide, sprint, reorder
    for n in 0..2 {
        append_notes(&root, "T-001");
        if n == 0 {
            assert!(b.toggle_card_hidden("T-001"));
        } else {
            assert!(b.toggle_card_sprint("T-001"));
        }
        b.commit(&["T-001".to_string()], false).unwrap();
        let t = disk_task(&b, "T-001");
        assert_eq!(t.body.matches("## Ход работы").count(), 2 + n, "{n}: {}", t.body);
    }
    append_notes(&root, "T-001");
    mv(&mut b, "T-001", 0);
    let shown = View { show_hidden: true, sprint_only: false };
    assert_eq!(ids(&b, 0, shown), ["T-002", "T-001"]);
    let w = b.reorder_card(0, shown, 1, -1).unwrap();
    b.commit(&w, false).unwrap();
    assert_eq!(disk_task(&b, "T-001").body.matches("## Ход работы").count(), 4);
    // trash and restore
    append_notes(&root, "T-002");
    trash(&mut b, "T-002");
    let t = disk_task(&b, "T-002");
    assert!(t.deleted && t.body.contains("проверки зелёные"));
    append_notes(&root, "T-002");
    restore(&mut b, "T-002");
    let t = disk_task(&b, "T-002");
    assert!(!t.deleted && t.body.matches("## Ход работы").count() == 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn card_move_keeps_a_column_changed_outside_for_untouched_fields() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    // an agent moves the card to review while we only hide it
    external_write(&root, "T-001", |t| t.replace("column: backlog", "column: review"));
    assert!(b.toggle_card_hidden("T-001"));
    b.commit(&["T-001".to_string()], false).unwrap();
    let t = disk_task(&b, "T-001");
    assert!(t.hidden);
    assert_eq!(t.column, "review");
    // the resync after the write put the card in its real column
    assert_eq!(b.effective_col(task(&b, "T-001")), 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_change_to_other_cards_is_resynced_after_our_write() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    add(&mut b, Kind::Task, "B", "", "");
    external_write(&root, "T-002", |t| t.replace("column: backlog", "column: done"));
    // `write` reports the external change; `commit` resyncs
    assert!(b.move_card_to("T-001", 1));
    assert_eq!(b.write(&["T-001".to_string()], false), Ok(true));
    assert_eq!(task(&b, "T-002").column, "backlog");
    assert!(!b.changed_on_disk());
    b.reload().unwrap();
    assert_eq!(task(&b, "T-002").column, "done");
    assert_eq!(task(&b, "T-001").column, "in-progress");
    external_write(&root, "T-002", |t| t.replace("column: done", "column: review"));
    mv(&mut b, "T-001", 4);
    assert_eq!(task(&b, "T-002").column, "review");
    assert_eq!(task(&b, "T-001").column, "done");
    assert!(agents_file(&root).contains("| T-002 | task | review |"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn edit_keeps_disk_body_when_description_untouched() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    edit(&mut b, "T-001", |i| i.body = "Original text".into()).unwrap();
    assert!(b.ext.is_empty());
    // the input was taken, then an agent appends notes before it is saved
    let mut i = prefill(&b, "T-001");
    assert_eq!(i.body, "Original text");
    append_notes(&root, "T-001");
    i.title = "A renamed".into();
    save(&mut b, Kind::Task, Some("T-001"), 0, &i).unwrap();
    assert_eq!(b.ext, [("T-001".to_string(), true)]);
    let t = disk_task(&b, "T-001");
    assert_eq!(t.title, "A renamed");
    assert!(t.body.starts_with("Original text") && t.body.contains("проверки зелёные"), "{}", t.body);
    assert_eq!(b.tasks[0].body, t.body);

    // the user edited the description: their text is saved and noted
    let mut i = prefill(&b, "T-001");
    append_notes(&root, "T-001");
    i.body = "Rewritten by the user".into();
    save(&mut b, Kind::Task, Some("T-001"), 0, &i).unwrap();
    assert_eq!(b.ext, [("T-001".to_string(), false)]);
    assert_eq!(disk_task(&b, "T-001").body, "Rewritten by the user");

    // nothing changed outside: nothing noted
    let i = prefill(&b, "T-001");
    save(&mut b, Kind::Task, Some("T-001"), 0, &i).unwrap();
    assert!(b.ext.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unreadable_disk_copy_blocks_the_write() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    let p = root.join(".pumba/tasks/T-001.md");
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&p, "garbage written by a crashed editor").unwrap();
    assert!(b.move_card_to("T-001", 1));
    let e = b.commit(&["T-001".to_string()], false).unwrap_err();
    assert_eq!(e, "T-001.md изменён снаружи и не читается; ничего не записано");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "garbage written by a crashed editor");
    let _ = std::fs::remove_dir_all(root);
}

// ---------------------------------------------------- models and defaults

#[test]
fn default_model_and_effort_for_new_cards() {
    let (mut b, root) = fresh();
    assert_eq!((b.cfg.default_model.as_str(), b.cfg.default_effort.as_str()), ("default", "default"));
    let first = add(&mut b, Kind::Task, "Old", "", "");
    let lists = |b: &Board, dm: &str, de: &str| ModelsInput {
        models: b.cfg.models.clone(),
        efforts: b.cfg.efforts.clone(),
        default_model: dm.into(),
        default_effort: de.into(),
    };
    let notes = b.apply_models(lists(&b, "sonnet", "high"));
    assert!(notes.is_empty());
    b.commit(&[], true).unwrap();
    let cfg = b.store.load().unwrap().0;
    assert_eq!((cfg.default_model.as_str(), cfg.default_effort.as_str()), ("sonnet", "high"));
    assert!(agents_file(&root).contains("Для новых карточек по умолчанию: модель sonnet, уровень high."));
    // an existing card keeps its own values; a new one built from the defaults stores them
    assert_eq!((task(&b, &first).model.as_str(), task(&b, &first).effort.as_str()), ("default", "default"));
    let id = add(&mut b, Kind::Task, "New", "", "");
    assert_eq!((task(&b, &id).model.as_str(), task(&b, &id).effort.as_str()), ("sonnet", "high"));
    assert!(raw(&root, &id).contains("model: sonnet\n") && raw(&root, &id).contains("effort: high\n"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn default_falls_back_when_removed_from_list() {
    let (mut b, root) = fresh();
    b.cfg.default_model = "opus".into();
    b.cfg.default_effort = "max".into();
    let mut m = ModelsInput {
        models: vec!["sonnet".into(), "haiku".into()],
        efforts: b.cfg.efforts.clone(),
        default_model: "opus".into(),
        default_effort: "max".into(),
    };
    assert_eq!(b.apply_models(m.clone()), ["Модель по умолчанию сброшена"]);
    assert_eq!((b.cfg.default_model.as_str(), b.cfg.default_effort.as_str()), ("default", "max"));
    assert_eq!(b.cfg.models, ["sonnet", "haiku"]);
    b.commit(&[], true).unwrap();
    let cfg = b.store.load().unwrap().0;
    assert_eq!((cfg.default_model.as_str(), cfg.default_effort.as_str()), ("default", "max"));
    // effort removed too
    m.default_model = "default".into();
    m.efforts = vec!["low".into()];
    assert_eq!(b.apply_models(m.clone()), ["Уровень по умолчанию сброшен"]);
    assert_eq!(b.cfg.default_effort, "default");
    // both at once; "default" is always valid, even with empty lists
    m.default_model = "ghost".into();
    m.default_effort = "ghost".into();
    assert_eq!(b.apply_models(m.clone()), ["Модель по умолчанию сброшена", "Уровень по умолчанию сброшен"]);
    let empty = ModelsInput { default_model: "default".into(), default_effort: "default".into(), ..Default::default() };
    assert!(b.apply_models(empty).is_empty());
    assert!(b.cfg.models.is_empty() && b.cfg.efforts.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn removed_model_survives_card_edit() {
    let (mut b, root) = fresh();
    add(&mut b, Kind::Task, "A", "", "");
    edit(&mut b, "T-001", |i| {
        i.model = "haiku".into();
        i.effort = "max".into();
    })
    .unwrap();
    // drop haiku and max from the lists
    let m = ModelsInput { models: vec!["opus".into()], efforts: vec!["low".into()], default_model: "default".into(), default_effort: "default".into() };
    b.apply_models(m);
    b.commit(&[], true).unwrap();
    let i = prefill(&b, "T-001");
    assert_eq!((i.model.as_str(), i.effort.as_str()), ("haiku", "max"));
    save(&mut b, Kind::Task, Some("T-001"), 0, &i).unwrap();
    let t = &b.store.load().unwrap().1[0];
    assert_eq!((t.model.as_str(), t.effort.as_str()), ("haiku", "max"));
    let _ = std::fs::remove_dir_all(root);
}
