//! Key, form and picker flows of the TUI; board logic itself is tested in `pumba-core`.
use super::*;
use super::forms::opt;
use crate::editor::TextBuf;
use crate::form::{Field, FieldKind, Purpose, Suggest};
use pumba_core::model::{Column, Config, Store, Task};
use std::sync::atomic::{AtomicUsize, Ordering};

fn k(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

/// App on a fresh temp dir with an initialised `.pumba/`.
fn fresh() -> (App, PathBuf) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let n = N.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("pumba-test-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut app = App::new(root.clone(), false);
    assert!(matches!(app.mode, Mode::Confirm(_, Action::Init)));
    app.on_key(k('y'));
    assert!(matches!(app.mode, Mode::InitTools(_)));
    app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    (app, root)
}

fn take_form(app: &mut App) -> Box<Form> {
    match std::mem::replace(&mut app.mode, Mode::Board) {
        Mode::Form(f) => f,
        _ => panic!("no form open"),
    }
}

fn set_text(f: &mut Form, key: &str, v: &str) {
    for fl in &mut f.fields {
        if let (true, FieldKind::Text(tb)) = (fl.key == key, &mut fl.kind) {
            *tb = TextBuf::new(v, tb.multiline);
        }
    }
}

fn set_sel(f: &mut Form, key: &str, v: &str) {
    for fl in &mut f.fields {
        if let (true, FieldKind::Select { opts, idx }) = (fl.key == key, &mut fl.kind) {
            *idx = opts.iter().position(|o| o.value == v).expect("option");
        }
    }
}

/// Create a card through the same form + apply path the UI uses; returns its id.
fn add(app: &mut App, kind: Kind, title: &str, parent: &str, sprint: &str) -> String {
    app.open_task_form(kind, None);
    let mut f = take_form(app);
    set_text(&mut f, "title", title);
    set_sel(&mut f, "sprint", sprint);
    if kind == Kind::Task {
        set_sel(&mut f, "parent", parent);
    }
    app.apply_form(&f).unwrap();
    app.tasks.last().unwrap().id.clone()
}

/// Move the selected card to the last column (done) with the L key.
fn to_done(app: &mut App) {
    for _ in 0..app.cfg.columns.len() {
        app.on_key(k('L'));
    }
    assert_eq!(app.sel_col, app.cfg.columns.len() - 1);
}

fn ids(app: &App, ci: usize) -> Vec<String> {
    app.col_task_idx(ci).iter().map(|&i| app.tasks[i].id.clone()).collect()
}

fn cur(app: &App) -> String {
    app.selected_id().unwrap()
}

#[test]
fn ids_and_creation() {
    let (mut app, root) = fresh();
    assert_eq!(add(&mut app, Kind::Task, "A", "", ""), "T-001");
    assert_eq!(add(&mut app, Kind::Epic, "Epic", "", ""), "E-001");
    assert_eq!(add(&mut app, Kind::Task, "B", "E-001", ""), "T-002");
    assert_eq!(cur(&app), "T-002");
    assert_eq!(app.tasks[2].parent.as_deref(), Some("E-001"));
    assert_eq!(app.tasks[2].column, "backlog");
    assert!(app.tasks[1].order > app.tasks[0].order);
    // gaps: next id follows the max suffix, not the count
    let mut t = app.tasks[0].clone();
    t.id = "T-010".into();
    app.tasks.push(t);
    assert_eq!(add(&mut app, Kind::Task, "C", "", ""), "T-011");
    assert!(root.join(".pumba/tasks/T-011.md").is_file());
    assert!(std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap().contains("| T-011 |"));

    // validation: empty title is rejected and nothing is added
    app.open_task_form(Kind::Task, None);
    let f = take_form(&mut app);
    let n = app.tasks.len();
    assert!(app.apply_form(&f).is_err());
    assert_eq!(app.tasks.len(), n);

    // edit in place via the form
    app.select_task("T-001");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    assert_eq!(f.text("title"), "A");
    set_text(&mut f, "title", "A2");
    set_sel(&mut f, "model", "opus");
    app.apply_form(&f).unwrap();
    let back = app.store.load().unwrap().1;
    let a = back.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!((a.title.as_str(), a.model.as_str()), ("A2", "opus"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn moving_and_reordering() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    add(&mut app, Kind::Task, "B", "", "");
    app.select_task("T-001");
    app.on_key(k('L'));
    assert_eq!((app.sel_col, cur(&app)), (1, "T-001".to_string()));
    assert_eq!(app.tasks[0].column, "in-progress");
    for _ in 0..5 {
        app.on_key(k('L')); // the last two are no-ops: already in the last column
    }
    assert_eq!(app.sel_col, 4);
    assert_eq!(app.tasks[0].column, "done");
    app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
    for _ in 0..3 {
        app.on_key(k('H'));
    }
    assert_eq!((app.sel_col, cur(&app)), (0, "T-001".to_string()));
    assert_eq!(ids(&app, 0), ["T-002", "T-001"]); // moved card goes last
    app.on_key(k('K'));
    assert_eq!(ids(&app, 0), ["T-001", "T-002"]);
    assert_eq!((app.sel_idx, cur(&app)), (0, "T-001".to_string()));
    app.on_key(k('J'));
    assert_eq!(ids(&app, 0), ["T-002", "T-001"]);
    // persisted order survives a reload
    app.reload();
    assert_eq!(ids(&app, 0), ["T-002", "T-001"]);
    assert_eq!(cur(&app), "T-001");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn latin_key_maps_layout() {
    let pairs = [
        ('р', 'h'), ('о', 'j'), ('л', 'k'), ('д', 'l'), ('Р', 'H'), ('О', 'J'), ('Л', 'K'), ('Д', 'L'),
        ('й', 'q'), ('т', 'n'), ('Т', 'N'), ('у', 'e'), ('в', 'd'), ('ч', 'x'), ('Ч', 'X'), ('м', 'v'),
        ('ы', 's'), ('з', 'p'), ('З', 'P'), ('с', 'c'), ('к', 'r'), ('Б', '<'), ('Ю', '>'), ('ъ', ']'),
        ('ь', 'm'), ('Ь', 'M'),
    ];
    for (from, to) in pairs {
        assert_eq!(latin_key(from), to, "{from}");
    }
    assert_eq!(latin_key('h'), 'h');
    assert_eq!(latin_key('?'), '?');
    assert_eq!(latin_key('ж'), 'ж');
}

fn col_ids(app: &App) -> Vec<String> {
    app.cfg.columns.iter().map(|c| c.id.clone()).collect()
}

fn pinned_status(app: &mut App, what: &str) -> String {
    let (msg, red, _) = app.status.take().expect("status");
    assert!(red && msg.contains(what) && msg.ends_with("первая и последняя закреплены"), "{msg}");
    msg
}

#[test]
fn column_move_hotkeys() {
    let (mut app, root) = fresh();
    let all = col_ids(&app);
    assert_eq!(all.len(), 5);
    let (id1, id2) = (all[1].clone(), all[2].clone());
    let ctrl = |c| KeyEvent::new(c, KeyModifiers::CONTROL);
    let alt = |c| KeyEvent::new(c, KeyModifiers::ALT);
    // a middle column moves inside the middle only
    app.status = None;
    app.sel_col = 1;
    app.on_key(ctrl(KeyCode::Right));
    assert_eq!(app.sel_col, 2);
    assert_eq!((&app.cfg.columns[1].id, &app.cfg.columns[2].id), (&id2, &id1));
    assert!(app.status.is_none());
    app.on_key(alt(KeyCode::Right));
    assert_eq!(app.sel_col, 3);
    assert_eq!(app.cfg.columns[3].id, id1);
    // the last middle slot is index 3: one more step would pass the pinned last column
    app.on_key(ctrl(KeyCode::Right));
    assert_eq!(app.sel_col, 3);
    assert_eq!(app.cfg.columns[3].id, id1);
    assert_eq!(app.cfg.columns[4].id, all[4]);
    let name = app.cfg.columns[3].name.clone();
    assert!(pinned_status(&mut app, &name).starts_with("Колонки «"));
    app.on_key(ctrl(KeyCode::Left));
    app.on_key(alt(KeyCode::Left));
    assert_eq!(app.sel_col, 1);
    assert_eq!(col_ids(&app), all);
    // and not before the pinned first one
    app.on_key(ctrl(KeyCode::Left));
    assert_eq!((app.sel_col, col_ids(&app)), (1, all.clone()));
    let name = app.cfg.columns[1].name.clone();
    pinned_status(&mut app, &name);
    // Russian layout: Ю = '>' moves right, Б = '<' moves left, plain arrows only select
    app.on_key(k('Ю'));
    assert_eq!((app.sel_col, app.cfg.columns[2].id.clone()), (2, id1.clone()));
    app.on_key(k('Б'));
    assert_eq!((app.sel_col, col_ids(&app)), (1, all.clone()));
    app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.sel_col, 2);
    assert_eq!(col_ids(&app), all);
    app.on_key(k('р')); // Russian h
    assert_eq!(app.sel_col, 1);
    assert!(app.status.is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pinned_columns_do_not_move() {
    let (mut app, root) = fresh();
    let all = col_ids(&app);
    for (sel, keys) in [
        (0, vec![ctrl_key(KeyCode::Right), ctrl_key(KeyCode::Left), k('>'), k('<'), k('Ю'), k('Б')]),
        (4, vec![ctrl_key(KeyCode::Right), ctrl_key(KeyCode::Left), k('>'), k('<'), k('Ю'), k('Б')]),
    ] {
        for key in keys {
            app.sel_col = sel;
            app.status = None;
            app.on_key(key);
            assert_eq!((app.sel_col, col_ids(&app)), (sel, all.clone()), "{key:?}");
            let name = app.cfg.columns[sel].name.clone();
            let msg = pinned_status(&mut app, &name);
            assert_eq!(msg, format!("Колонки «{name}» нельзя двигать: первая и последняя закреплены"));
        }
    }
    // nothing was written either
    assert_eq!(app.store.load().unwrap().0.columns.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), all);
    // a board of two (both pinned) and of one column
    app.cfg.columns.truncate(2);
    app.scrolls.truncate(2);
    for sel in 0..2 {
        app.sel_col = sel;
        app.on_key(k('>'));
        app.on_key(k('<'));
        assert_eq!(app.cfg.columns.len(), 2);
        assert_eq!((app.cfg.columns[0].id.as_str(), app.cfg.columns[1].id.as_str()), (all[0].as_str(), all[1].as_str()));
    }
    app.cfg.columns.truncate(1);
    app.scrolls.truncate(1);
    app.sel_col = 0;
    app.on_key(k('>'));
    assert_eq!(app.cfg.columns.len(), 1);
    let name = app.cfg.columns[0].name.clone();
    pinned_status(&mut app, &name);
    let _ = std::fs::remove_dir_all(root);
}

fn ctrl_key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::CONTROL)
}

#[test]
fn visibility_filters() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "1");
    add(&mut app, Kind::Task, "B", "", "");
    add(&mut app, Kind::Epic, "E", "", "");
    add(&mut app, Kind::Task, "C", "E-001", "1");
    assert_eq!(ids(&app, 0).len(), 4);
    // hide B
    app.select_task("T-002");
    app.on_key(k('x'));
    assert_eq!(ids(&app, 0), ["T-001", "E-001", "T-003"]);
    app.on_key(k('v'));
    assert_eq!(ids(&app, 0).len(), 4);
    app.on_key(k('v'));
    // sprint only: A and C by sprint, epic via its child; B hidden anyway
    app.on_key(k('s'));
    assert_eq!(ids(&app, 0), ["T-001", "E-001", "T-003"]);
    app.select_task("T-001");
    app.on_key(k('p')); // A leaves the sprint
    assert_eq!(ids(&app, 0), ["E-001", "T-003"]);
    app.select_task("T-003");
    app.on_key(k('p'));
    assert!(ids(&app, 0).is_empty());
    assert!(app.selected_idx().is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epic_progress_and_sprint_rollover() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Epic, "E", "", "");
    add(&mut app, Kind::Task, "A", "E-001", "1");
    add(&mut app, Kind::Task, "B", "E-001", "1");
    add(&mut app, Kind::Task, "C", "E-001", "1");
    assert_eq!(app.epic_progress("E-001"), (0, 3));
    app.select_task("T-001");
    to_done(&mut app); // A -> done
    assert_eq!(app.epic_progress("E-001"), (1, 3));
    app.on_key(k(']'));
    assert!(matches!(app.mode, Mode::Confirm(_, Action::NextSprint)));
    app.on_key(k('y'));
    assert_eq!(app.cfg.sprint, 2);
    let sprint = |id: &str| app.tasks.iter().find(|t| t.id == id).unwrap().sprint;
    assert_eq!((sprint("T-001"), sprint("T-002"), sprint("E-001")), (Some(1), Some(2), None));
    assert_eq!(app.store.load().unwrap().0.sprint, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_edit_reload_and_delete() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Epic, "E", "", "");
    add(&mut app, Kind::Task, "A", "E-001", "");
    app.select_task("T-001");
    let p = root.join(".pumba/tasks/T-001.md");
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, text.replace("column: backlog", "column: done")).unwrap();
    app.reload();
    let a = app.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(app.effective_col(a), 4);
    assert_eq!((app.sel_col, cur(&app)), (4, "T-001".to_string()));

    // trashing the epic keeps its file and the children's parent
    app.select_task("E-001");
    app.on_key(k('d'));
    app.on_key(k('y'));
    assert!(root.join(".pumba/tasks/E-001.md").exists());
    let back = app.store.load().unwrap().1;
    assert_eq!(back.iter().find(|t| t.id == "T-001").unwrap().parent.as_deref(), Some("E-001"));
    assert!(back.iter().find(|t| t.id == "E-001").unwrap().deleted);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn columns_and_rules() {
    let (mut app, root) = fresh();
    for name in ["My Col!", "my col"] {
        app.on_key(k('c'));
        let mut f = take_form(&mut app);
        set_text(&mut f, "name", name);
        app.apply_form(&f).unwrap();
    }
    // new columns land right before the pinned last one and become selected
    assert_eq!(col_ids(&app), ["backlog", "in-progress", "review", "on-dev", "my-col", "my-col-2", "done"]);
    assert_eq!((app.sel_col, app.scrolls.len()), (5, 7));
    assert_eq!(app.store.load().unwrap().0.columns[5].id, "my-col-2");

    // move column left (inside the middle), then delete one: cards go to the first remaining column
    app.on_key(k('<'));
    assert_eq!((app.sel_col, app.cfg.columns[4].id.as_str()), (4, "my-col-2"));
    assert_eq!(app.cfg.columns[6].id, "done");
    app.sel_col = 1;
    add(&mut app, Kind::Task, "A", "", "");
    assert_eq!(app.tasks[0].column, "in-progress");
    app.on_key(k('X'));
    app.on_key(k('y'));
    assert_eq!(app.tasks[0].column, "backlog");
    assert_eq!((app.cfg.columns.len(), app.scrolls.len()), (6, 6));
    assert_eq!(app.store.load().unwrap().0.columns.len(), 6);

    // rules form writes back
    app.on_key(k('P'));
    let mut f = take_form(&mut app);
    set_sel(&mut f, "tests", "tdd");
    set_text(&mut f, "custom", "Be nice");
    app.apply_form(&f).unwrap();
    let cfg = app.store.load().unwrap().0;
    assert_eq!(cfg.rules.iter().find(|r| r.id == "tests").unwrap().value, "tdd");
    assert_eq!(cfg.custom_rules, "Be nice");
    assert!(std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap().contains("Be nice"));

    // the first and the last columns cannot be deleted: no confirm, a red status, nothing written
    let ids = col_ids(&app);
    for sel in [0, app.cfg.columns.len() - 1] {
        app.sel_col = sel;
        app.status = None;
        app.on_key(k('X'));
        assert!(matches!(app.mode, Mode::Board));
        let name = app.cfg.columns[sel].name.clone();
        let (msg, red, _) = app.status.clone().unwrap();
        assert!(red);
        assert_eq!(msg, format!("Колонку «{name}» нельзя удалить: первая и последняя закреплены"));
        // the action itself refuses too, even if it were somehow confirmed
        app.run_action(Action::DeleteColumn(ids[sel].clone()));
        assert_eq!(col_ids(&app), ids);
        assert_eq!(app.store.load().unwrap().0.columns.len(), ids.len());
    }
    // the middle ones go one by one until only the two pinned remain
    while app.cfg.columns.len() > 2 {
        app.sel_col = 1;
        app.on_key(k('X'));
        app.on_key(k('y'));
    }
    assert_eq!(col_ids(&app), [ids[0].clone(), ids[ids.len() - 1].clone()]);
    app.sel_col = 1;
    app.on_key(k('X'));
    assert!(app.status.as_ref().unwrap().1 && matches!(app.mode, Mode::Board));
    assert_eq!(app.cfg.columns.len(), 2);
    // a one-column board keeps its own message
    app.cfg.columns.truncate(1);
    app.sel_col = 0;
    app.on_key(k('X'));
    assert!(app.status.as_ref().unwrap().0.contains("единственную"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_column_goes_before_the_last() {
    let (mut app, root) = fresh();
    let mk = |app: &mut App, name: &str| {
        app.on_key(k('c'));
        let mut f = take_form(app);
        set_text(&mut f, "name", name);
        app.apply_form(&f).unwrap();
    };
    // selection elsewhere does not matter
    app.sel_col = 0;
    mk(&mut app, "QA");
    assert_eq!(col_ids(&app), ["backlog", "in-progress", "review", "on-dev", "qa", "done"]);
    assert_eq!((app.sel_col, app.scrolls.len()), (4, 6));
    app.sel_col = 5;
    mk(&mut app, "Ops");
    assert_eq!(col_ids(&app), ["backlog", "in-progress", "review", "on-dev", "qa", "ops", "done"]);
    assert_eq!(app.sel_col, 5);
    // a board of two: the new column is the only middle one
    app.cfg.columns.truncate(1);
    let first = app.cfg.columns[0].clone();
    app.cfg.columns.push(first);
    app.cfg.columns[1].id = "fin".into();
    app.scrolls.truncate(2);
    mk(&mut app, "Mid");
    assert_eq!(col_ids(&app), ["backlog", "mid", "fin"]);
    assert_eq!(app.sel_col, 1);
    // a single-column board just appends
    app.cfg.columns.truncate(1);
    app.scrolls.truncate(1);
    mk(&mut app, "Second");
    assert_eq!(col_ids(&app), ["backlog", "second"]);
    assert_eq!(app.sel_col, 1);
    // old boards: pinning is by position, whatever the ids are
    app.cfg.columns[0].id = "alpha".into();
    app.cfg.columns[1].id = "omega".into();
    mk(&mut app, "Between");
    assert_eq!(col_ids(&app), ["alpha", "between", "omega"]);
    app.sel_col = 2;
    app.on_key(k('<'));
    assert_eq!(col_ids(&app), ["alpha", "between", "omega"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn models_form_saves_lists() {
    let (mut app, root) = fresh();
    for key in ['M', 'Ь'] {
        app.on_key(k(key));
        let f = take_form(&mut app);
        assert_eq!(f.purpose, Purpose::Models);
        assert_eq!(f.title, "Модели и уровни");
        assert_eq!(f.text("models"), "opus\nsonnet\nhaiku");
        assert_eq!(f.text("efforts"), "low\nmedium\nhigh\nxhigh\nmax");
    }
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "models", "  gpt-5  \n\nopus\ndefault\ngpt-5\n  \r\nsonnet\n");
    set_text(&mut f, "efforts", "");
    app.mode = Mode::Form(f);
    app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.mode, Mode::Board));
    assert_eq!(app.cfg.models, ["gpt-5", "opus", "sonnet"]);
    assert!(app.cfg.efforts.is_empty());
    assert_eq!(app.status.as_ref().map(|s| (s.0.as_str(), s.1)), Some(("Списки сохранены", false)));
    let cfg = app.store.load().unwrap().0;
    assert_eq!(cfg.models, ["gpt-5", "opus", "sonnet"]);
    assert!(cfg.efforts.is_empty());
    assert!(root.join(".pumba/AGENTS.md").is_file());

    // an empty list leaves only "по умолчанию" in the Select
    app.on_key(k('n'));
    let f = take_form(&mut app);
    let opts = |key: &str| match &f.fields.iter().find(|x| x.key == key).unwrap().kind {
        FieldKind::Select { opts, .. } => opts.iter().map(|o| o.value.clone()).collect::<Vec<_>>(),
        _ => panic!(),
    };
    assert_eq!(opts("effort"), ["default"]);
    assert_eq!(opts("model"), ["default", "gpt-5", "opus", "sonnet"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn removed_model_survives_card_edit() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    app.open_task_form(Kind::Task, Some(0));
    let mut f = take_form(&mut app);
    set_sel(&mut f, "model", "haiku");
    set_sel(&mut f, "effort", "max");
    app.apply_form(&f).unwrap();
    // drop haiku and max from the lists
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "models", "opus");
    set_text(&mut f, "efforts", "low");
    app.apply_form(&f).unwrap();
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!((f.sel("model").as_str(), f.sel("effort").as_str()), ("haiku", "max"));
    app.apply_form(&f).unwrap();
    let t = &app.store.load().unwrap().1[0];
    assert_eq!((t.model.as_str(), t.effort.as_str()), ("haiku", "max"));
    let _ = std::fs::remove_dir_all(root);
}

/// Edit `id` through the form with the given depends text; returns the apply result.
fn set_depends(app: &mut App, id: &str, v: &str) -> Result<(), String> {
    app.select_task(id);
    app.on_key(k('e'));
    let mut f = take_form(app);
    set_text(&mut f, "depends", v);
    let snap = (app.cfg.clone(), app.tasks.clone());
    let r = app.apply_form(&f);
    if r.is_err() {
        (app.cfg, app.tasks) = snap; // same restore the key path does
    }
    r
}

fn deps(app: &App, id: &str) -> Vec<String> {
    app.tasks.iter().find(|t| t.id == id).unwrap().depends.clone()
}

#[test]
fn blockers_and_move_warning() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    add(&mut app, Kind::Task, "B", "", "");
    add(&mut app, Kind::Epic, "E", "", "");
    set_depends(&mut app, "T-002", "T-001, E-001, ZZZ-9").unwrap_err(); // unknown id rejected
    set_depends(&mut app, "T-002", "T-001, E-001, T-001").unwrap(); // duplicate dropped silently
    assert_eq!(deps(&app, "T-002"), ["T-001", "E-001"]);
    let b = app.tasks[1].clone();
    let blk = |app: &App, t: &Task| app.blockers(t).iter().map(|x| x.id.clone()).collect::<Vec<_>>();
    assert_eq!(blk(&app, &b), ["T-001", "E-001"]);
    // unknown ids (external edit) are ignored
    let mut ghost = b.clone();
    ghost.depends = vec!["NOPE".into()];
    assert!(app.blockers(&ghost).is_empty());
    // finishing T-001 leaves only the epic
    app.select_task("T-001");
    to_done(&mut app);
    assert_eq!(blk(&app, &b), ["E-001"]);
    app.select_task("E-001");
    to_done(&mut app);
    assert!(blk(&app, &b).is_empty());
    // moving a blocked card out of the first column warns; moving a free card does not
    app.select_task("E-001");
    app.on_key(k('H'));
    assert_eq!(blk(&app, &b), ["E-001"]);
    app.status = None;
    app.select_task("T-002");
    app.on_key(k('L'));
    let st = app.status.as_ref().map(|s| s.0.clone());
    assert_eq!(st.as_deref(), Some("Заблокирована: E-001"));
    app.status = None;
    app.select_task("T-001");
    app.on_key(k('H'));
    assert!(app.status.is_none());
    app.reload();
    assert_eq!(deps(&app, "T-002"), ["T-001", "E-001"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn self_and_cycle_rejected() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    let e = set_depends(&mut app, "T-001", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("самой себя"), "{e}");
    set_depends(&mut app, "T-001", "T-002").unwrap();
    set_depends(&mut app, "T-002", "T-003").unwrap();
    let e = set_depends(&mut app, "T-003", "T-001").unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-003 → T-001 → T-002 → T-003"), "{e}");
    // T-001 already depends on T-002 (prefilled in T-002's «Блокирует»): the same id in both fields is rejected
    let e = set_depends(&mut app, "T-002", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("одновременно"), "{e}");
    assert!(deps(&app, "T-003").is_empty());
    set_depends(&mut app, "T-001", "T-002, T-003").unwrap(); // a diamond is fine
    let back = app.store.load().unwrap().1;
    assert!(back.iter().find(|t| t.id == "T-003").unwrap().depends.is_empty());
    // through the real key path the form stays open with an error status
    add(&mut app, Kind::Task, "D", "", "");
    set_depends(&mut app, "T-004", "T-001").unwrap();
    app.select_task("T-003");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "depends", "T-004");
    app.mode = Mode::Form(f);
    app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.mode, Mode::Form(_)));
    let st = app.status.as_ref().unwrap();
    assert!(st.1 && st.0.contains("Цикл"));
    assert!(deps(&app, "T-003").is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn depends_form_roundtrip_and_trash_keeps_depends() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    set_depends(&mut app, "T-003", "T-001,T-002").unwrap();
    set_depends(&mut app, "T-002", "T-001").unwrap();
    // prefill shows the stored list
    app.select_task("T-003");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!(f.text("depends"), "T-001, T-002");
    // epics get the field too
    add(&mut app, Kind::Epic, "E", "", "");
    app.select_task("E-001");
    app.on_key(k('e'));
    assert_eq!(take_form(&mut app).text("depends"), "");
    let raw = std::fs::read_to_string(root.join(".pumba/tasks/T-003.md")).unwrap();
    assert!(raw.contains("depends: T-001, T-002\n"));
    app.reload();
    assert_eq!(deps(&app, "T-003"), ["T-001", "T-002"]);
    // clearing the field removes the key
    set_depends(&mut app, "T-002", "").unwrap();
    assert!(!std::fs::read_to_string(root.join(".pumba/tasks/T-002.md")).unwrap().contains("depends"));
    set_depends(&mut app, "T-002", "T-001").unwrap();

    // trashing T-001 removes nothing from other cards, in memory or on disk
    app.select_task("T-001");
    app.on_key(k('d'));
    app.on_key(k('y'));
    assert_eq!(deps(&app, "T-003"), ["T-001", "T-002"]);
    assert_eq!(deps(&app, "T-002"), ["T-001"]);
    let back = app.store.load().unwrap().1;
    assert_eq!(back.iter().find(|t| t.id == "T-003").unwrap().depends, ["T-001", "T-002"]);
    assert!(root.join(".pumba/tasks/T-001.md").is_file());
    let agents = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains("| T-003 | task | backlog | - | T-001, T-002 | - | C |"), "{agents}");
    assert!(!agents.contains("| T-001 | task"), "{agents}");
    assert!(agents.contains("ОБЯЗАН ОТКАЗАТЬСЯ"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn broken_board_json_is_never_overwritten() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    let p = root.join(".pumba/board.json");
    std::fs::write(&p, "{ not json").unwrap();
    app.reload();
    assert!(!app.ok && app.status.as_ref().unwrap().1);
    app.on_key(k('c'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "name", "X");
    assert!(app.apply_form(&f).is_err());
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ not json");
    let _ = std::fs::remove_dir_all(root);
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_in(app: &mut App, s: &str) {
    for c in s.chars() {
        app.on_key(k(c));
    }
}

fn picker(app: &App) -> Option<crate::form::PickerView> {
    match &app.mode {
        Mode::Form(f) => f.picker_view(),
        _ => panic!("no form open"),
    }
}

/// Open the new-task form and focus the description (title, body, model, effort, sprint, parent, depends).
fn open_body(app: &mut App) {
    app.on_key(k('n'));
    press(app, KeyCode::Tab);
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.fields[f.focus].key, "body"),
        _ => panic!("no form open"),
    }
}

#[test]
fn at_picker_in_task_description() {
    let (mut app, root) = fresh();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    for f in ["src/ui.rs", "src/model.rs", "README.md", "target/junk.rs"] {
        std::fs::write(root.join(f), "x").unwrap();
    }
    app.on_key(k('n'));
    type_in(&mut app, "T");
    press(&mut app, KeyCode::Tab);
    assert!(app.files.is_none());
    type_in(&mut app, "Fix @ui");
    assert!(app.files.is_some());
    let v = picker(&app).unwrap();
    assert_eq!((v.results[0].as_str(), v.selected), ("src/ui.rs", 0));
    assert!(!v.results.iter().any(|p| p.contains("target")));
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Form(_)));
    type_in(&mut app, "now");
    press(&mut app, KeyCode::Enter); // popup is closed: a plain newline
    type_in(&mut app, "@mo");
    assert_eq!(picker(&app).unwrap().results[0], "src/model.rs");
    // Esc closes only the popup
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(picker(&app).is_none());
    app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(matches!(app.mode, Mode::Board));
    let t = app.tasks.last().unwrap();
    assert_eq!(t.body, "Fix @src/ui.rs now
@mo");
    let back = app.store.load().unwrap().1;
    assert_eq!(back.last().unwrap().body, "Fix @src/ui.rs now
@mo");

    // a new form rebuilds the index: files created meanwhile show up; second Esc cancels the form
    std::fs::write(root.join("brand_new.txt"), "x").unwrap();
    open_body(&mut app);
    assert!(app.files.is_none());
    type_in(&mut app, "@brand");
    assert_eq!(picker(&app).unwrap().results, ["brand_new.txt"]);
    press(&mut app, KeyCode::Down); // single result: wraps onto itself
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Form(_)));
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Board));

    // the models form never opens the picker
    app.on_key(k('M'));
    type_in(&mut app, "@");
    assert!(picker(&app).is_none());
    // rules form: custom rules is multiline and does
    press(&mut app, KeyCode::Esc);
    app.on_key(k('P'));
    if let Mode::Form(f) = &mut app.mode {
        f.focus = f.fields.len() - 1;
    }
    type_in(&mut app, "@read");
    assert_eq!(picker(&app).unwrap().results[0], "README.md");
    press(&mut app, KeyCode::Tab);
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.text("custom"), "@README.md "),
        _ => panic!(),
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Open the new-task form with `title` typed and focus on the dependency field (second to last: «Блокирует» follows it).
fn open_depends(app: &mut App, title: &str) {
    app.on_key(k('n'));
    type_in(app, title);
    press(app, KeyCode::BackTab); // blocks
    press(app, KeyCode::BackTab);
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.fields[f.focus].key, "depends"),
        _ => panic!("no form open"),
    }
}

fn save_key(app: &mut App) {
    app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
}

#[test]
fn depends_suggestions_end_to_end() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Epic, "Авторизация", "", "");
    add(&mut app, Kind::Task, "Форма входа", "", "");
    add(&mut app, Kind::Task, "Refactor parser", "", "");
    add(&mut app, Kind::Task, "Ship it", "", "");
    let ids = |app: &App| picker(app).map(|v| v.results);

    // new card: nothing offered until something is typed
    open_depends(&mut app, "Новая");
    assert!(picker(&app).is_none());
    type_in(&mut app, "T-00");
    assert_eq!(ids(&app).unwrap(), ["T-001", "T-002", "T-003"]);
    press(&mut app, KeyCode::Enter);
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.text("depends"), "T-001, "),
        _ => panic!(),
    }
    type_in(&mut app, "ship");
    assert_eq!(ids(&app).unwrap(), ["T-003"]);
    press(&mut app, KeyCode::Tab);
    match &app.mode {
        Mode::Form(f) => assert_eq!((f.text("depends").as_str(), f.focus), ("T-001, T-003, ", f.fields.len() - 2)),
        _ => panic!(),
    }
    // the trailing ", " is accepted by the parser; save and reload
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Board), "{:?}", app.status);
    assert_eq!(deps(&app, "T-004"), ["T-001", "T-003"]);
    assert_eq!(app.store.load().unwrap().1.iter().find(|t| t.id == "T-004").unwrap().depends, ["T-001", "T-003"]);

    // edit T-002: the card itself is never offered, ids already listed are skipped
    app.select_task("T-002");
    app.on_key(k('e'));
    press(&mut app, KeyCode::BackTab);
    press(&mut app, KeyCode::BackTab);
    type_in(&mut app, "T-00");
    assert_eq!(ids(&app).unwrap(), ["T-001", "T-003", "T-004"]);
    type_in(&mut app, "1, T-004, T-00");
    assert_eq!(ids(&app).unwrap(), ["T-003"]);
    // Esc closes only the popup; the second Esc cancels the form
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Form(_)) && picker(&app).is_none());
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Board));
    assert!(deps(&app, "T-002").is_empty());

    // cycles are still rejected on save: T-004 depends on T-001; T-001 -> T-002 is fine, T-002 -> T-004 closes it
    for (id, dep) in [("T-001", "T-002"), ("T-002", "T-004")] {
        app.select_task(id);
        app.on_key(k('e'));
        press(&mut app, KeyCode::BackTab);
        press(&mut app, KeyCode::BackTab);
        type_in(&mut app, dep);
        press(&mut app, KeyCode::Enter);
        save_key(&mut app);
        if id == "T-001" {
            assert!(matches!(app.mode, Mode::Board), "{:?}", app.status);
        }
    }
    assert!(matches!(app.mode, Mode::Form(_)), "cycle must keep the form open");
    assert!(app.status.as_ref().unwrap().0.contains("Цикл"), "{:?}", app.status);
    let _ = std::fs::remove_dir_all(root);
}

/// Trash the card `id` through the real key path (d, y).
fn trash(app: &mut App, id: &str) {
    app.select_task(id);
    assert_eq!(cur(app), id);
    app.on_key(k('d'));
    app.on_key(k('y'));
}

fn raw(root: &std::path::Path, id: &str) -> String {
    std::fs::read_to_string(root.join(format!(".pumba/tasks/{id}.md"))).unwrap()
}

fn blk(app: &App, id: &str) -> Vec<String> {
    let t = app.tasks.iter().find(|t| t.id == id).unwrap();
    app.blockers(t).iter().map(|x| x.id.clone()).collect()
}

#[test]
fn soft_delete_keeps_file_and_hides_card() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "Первая", "", "");
    add(&mut app, Kind::Task, "Вторая", "", "");
    app.select_task("T-001");
    app.on_key(k('d'));
    match &app.mode {
        Mode::Confirm(m, Action::TrashTask(id)) => {
            assert_eq!(id, "T-001");
            assert_eq!(
                m,
                "Переместить «Первая» в корзину? Файл останется в .pumba/tasks/, восстановить можно в корзине (B)."
            );
        }
        _ => panic!("no confirm"),
    }
    // declining changes nothing
    app.on_key(k('n'));
    assert!(!app.tasks[0].deleted);
    app.select_task("T-001");
    app.on_key(k('d'));
    app.on_key(k('y'));
    assert_eq!(app.status.as_ref().map(|s| (s.0.as_str(), s.1)), Some(("В корзине: T-001. Восстановить: B", false)));
    assert!(root.join(".pumba/tasks/T-001.md").is_file());
    let md = raw(&root, "T-001");
    assert!(md.contains("deleted: true\n") && md.contains("deleted_at: "), "{md}");
    assert_eq!(ids(&app, 0), ["T-002"]);
    assert_eq!(cur(&app), "T-002");
    // never shown, even with hidden cards revealed
    app.on_key(k('v'));
    assert_eq!(ids(&app, 0), ["T-002"]);
    // survives a reload; AGENTS.md has no index row for it
    app.reload();
    assert_eq!(ids(&app, 0), ["T-002"]);
    assert!(app.tasks.iter().find(|t| t.id == "T-001").unwrap().deleted_at.is_some());
    let agents = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains("| T-002 |") && !agents.contains("| T-001 |"));
    assert!(agents.contains("`deleted: true`") && agents.contains("полностью игнорируй"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trashed_dependency_does_not_block_and_survives() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    add(&mut app, Kind::Task, "B", "", "");
    add(&mut app, Kind::Task, "C", "", "");
    set_depends(&mut app, "T-002", "T-001").unwrap();
    assert_eq!(blk(&app, "T-002"), ["T-001"]);
    trash(&mut app, "T-001");
    assert!(blk(&app, "T-002").is_empty());
    assert!(raw(&root, "T-002").contains("depends: T-001\n"));
    // card suggestions never offer trashed cards
    app.select_task("T-002");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert!(f.cards.iter().all(|c| c.id != "T-001") && f.cards.iter().any(|c| c.id == "T-003"));
    // a stored entry survives an edit that keeps it...
    set_depends(&mut app, "T-002", "T-001").unwrap();
    assert_eq!(deps(&app, "T-002"), ["T-001"]);
    assert!(raw(&root, "T-002").contains("depends: T-001\n"));
    // ...but a trashed id cannot be newly picked
    let e = set_depends(&mut app, "T-003", "T-001").unwrap_err();
    assert!(e.contains("T-001") && e.contains("корзине"), "{e}");
    assert!(deps(&app, "T-003").is_empty());
    // restore: the card is back and blocks again
    app.on_key(k('B'));
    app.on_key(k('r'));
    assert_eq!(blk(&app, "T-002"), ["T-001"]);
    assert!(!raw(&root, "T-001").contains("deleted"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epic_soft_delete_keeps_children() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Epic, "E", "", "");
    add(&mut app, Kind::Task, "A", "E-001", "1");
    add(&mut app, Kind::Task, "B", "E-001", "1");
    app.select_task("T-001");
    to_done(&mut app);
    assert_eq!(app.epic_progress("E-001"), (1, 2));
    // trashing a child leaves the epic with one child
    trash(&mut app, "T-002");
    assert_eq!(app.epic_progress("E-001"), (1, 1));
    trash(&mut app, "E-001");
    assert!(raw(&root, "E-001").contains("deleted: true"));
    for id in ["T-001", "T-002"] {
        assert!(raw(&root, id).contains("parent: E-001\n"), "{id}");
    }
    // the child still shows with its parent link; the trashed epic is gone from the board and the select
    assert_eq!(ids(&app, 4), ["T-001"]);
    assert_eq!(app.tasks.iter().find(|t| t.id == "T-001").unwrap().parent.as_deref(), Some("E-001"));
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    let parent = f.fields.iter().find(|x| x.key == "parent").unwrap();
    match &parent.kind {
        FieldKind::Select { opts, idx } => {
            assert_eq!(opts.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(), ["", "E-001"]);
            assert_eq!(opts[*idx].value, "E-001"); // kept, not lost
        }
        _ => panic!(),
    }
    // rollover skips the trashed card
    app.on_key(k(']'));
    app.on_key(k('y'));
    let sprint = |id: &str| app.tasks.iter().find(|t| t.id == id).unwrap().sprint;
    assert_eq!((sprint("T-001"), sprint("T-002")), (Some(1), Some(1)));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn trash_view_open_restore_close() {
    let (mut app, root) = fresh();
    // empty trash
    app.on_key(k('B'));
    assert!(matches!(app.mode, Mode::Trash(0)));
    assert!(app.trashed().is_empty());
    app.on_key(k('r')); // nothing to restore
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Trash(0)));
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Board));

    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    for id in ["T-001", "T-002", "T-003"] {
        trash(&mut app, id);
    }
    // newest deleted first, whatever the order of the keys pressed
    let at = |app: &mut App, id: &str, v: u64| app.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = Some(v);
    at(&mut app, "T-001", 300);
    at(&mut app, "T-002", 100);
    at(&mut app, "T-003", 200);
    let order = |app: &App| app.trashed().iter().map(|&i| app.tasks[i].id.clone()).collect::<Vec<_>>();
    assert_eq!(order(&app), ["T-001", "T-003", "T-002"]);
    assert!(ids(&app, 0).is_empty());

    // 'b' does not open it; the Russian layout key for B does
    app.on_key(k('b'));
    assert!(matches!(app.mode, Mode::Board));
    app.on_key(k('И'));
    assert!(matches!(app.mode, Mode::Trash(0)));
    // navigation clamps at both ends; j/k work
    press(&mut app, KeyCode::Down);
    app.on_key(k('j'));
    press(&mut app, KeyCode::Down);
    assert!(matches!(app.mode, Mode::Trash(2)));
    app.on_key(k('k'));
    assert!(matches!(app.mode, Mode::Trash(1)));
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    assert!(matches!(app.mode, Mode::Trash(0)));
    // restore the last row: selection steps back into range, view stays open
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    app.on_key(k('r'));
    assert!(matches!(app.mode, Mode::Trash(1)));
    assert_eq!(app.status.as_ref().map(|s| s.0.as_str()), Some("Восстановлена: T-002"));
    assert_eq!(order(&app), ["T-001", "T-003"]);
    assert_eq!(ids(&app, 0), ["T-002"]);
    let md = raw(&root, "T-002");
    assert!(!md.contains("deleted"), "{md}");
    // Enter restores the selected one; selection stays in range, view still open
    press(&mut app, KeyCode::Enter);
    assert_eq!(order(&app), ["T-001"]);
    assert!(matches!(app.mode, Mode::Trash(0)));
    app.on_key(k('к')); // Russian r
    assert!(app.trashed().is_empty());
    assert!(matches!(app.mode, Mode::Trash(0)));
    assert_eq!(ids(&app, 0).len(), 3);
    // no key purges anything: every file is still there
    for key in ['d', 'x', 'X', 'D'] {
        app.mode = Mode::Trash(0);
        app.on_key(k(key));
        for id in ["T-001", "T-002", "T-003"] {
            assert!(root.join(format!(".pumba/tasks/{id}.md")).is_file());
        }
    }
    // close with q, B, Esc
    for key in [k('q'), k('B'), KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)] {
        app.mode = Mode::Trash(0);
        app.on_key(key);
        assert!(matches!(app.mode, Mode::Board));
        assert!(!app.quit);
    }
    // restored into a missing column: lands in the first column
    trash(&mut app, "T-001");
    app.tasks.iter_mut().find(|t| t.id == "T-001").unwrap().column = "gone".into();
    app.on_key(k('B'));
    app.on_key(k('r'));
    assert_eq!(app.sel_col, 0);
    let t = app.tasks.iter().find(|t| t.id == "T-001").unwrap();
    assert_eq!(app.effective_col(t), 0);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------- reverse dependencies

/// Edit `id` through the form with the given text fields; same restore as the key path on error.
fn edit(app: &mut App, id: &str, texts: &[(&str, &str)]) -> Result<(), String> {
    app.select_task(id);
    app.on_key(k('e'));
    let mut f = take_form(app);
    for (key, v) in texts {
        set_text(&mut f, key, v);
    }
    let snap = (app.cfg.clone(), app.tasks.clone());
    let r = app.apply_form(&f);
    if r.is_err() {
        (app.cfg, app.tasks) = snap;
    }
    r
}

/// Create a task through the form with the given text fields; returns the new id.
fn create(app: &mut App, texts: &[(&str, &str)]) -> Result<String, String> {
    app.open_task_form(Kind::Task, None);
    let mut f = take_form(app);
    for (key, v) in texts {
        set_text(&mut f, key, v);
    }
    let snap = (app.cfg.clone(), app.tasks.clone());
    let r = app.apply_form(&f);
    if r.is_err() {
        (app.cfg, app.tasks) = snap;
    }
    r.map(|()| app.tasks.last().unwrap().id.clone())
}

fn files_of(root: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root.join(".pumba/tasks"))
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();
    v.sort();
    v
}

#[test]
fn blocks_prefill_and_reverse_edges_on_disk() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    // B keeps its other fields and its own depends when A starts blocking it
    edit(&mut app, "T-002", &[("body", "Body of B"), ("depends", "T-003")]).unwrap();
    app.select_task("T-002");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    set_sel(&mut f, "effort", "high");
    set_sel(&mut f, "sprint", "1");
    app.apply_form(&f).unwrap();
    let before_b = raw(&root, "T-002");
    assert!(before_b.contains("depends: T-003\n"));

    // the blocks field sits right after depends and is empty at first
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    let keys: Vec<&str> = f.fields.iter().map(|x| x.key.as_str()).collect();
    assert_eq!(&keys[keys.len() - 2..], ["depends", "blocks"]);
    assert_eq!(f.text("blocks"), "");
    let fl = f.fields.last().unwrap();
    assert!(fl.side && fl.suggest == Suggest::Cards && fl.label == "Блокирует (id через запятую)");

    edit(&mut app, "T-001", &[("blocks", "T-002, T-003, T-002")]).unwrap(); // duplicate dropped silently
    assert_eq!(deps(&app, "T-002"), ["T-003", "T-001"]);
    assert_eq!(deps(&app, "T-003"), ["T-001"]);
    assert!(deps(&app, "T-001").is_empty());
    // on disk: no `blocks` key anywhere, other fields of B untouched
    assert!(files_of(&root).iter().all(|s| !s.contains("blocks")));
    let after_b = raw(&root, "T-002");
    assert_eq!(after_b, before_b.replace("depends: T-003\n", "depends: T-003, T-001\n"));
    assert!(after_b.contains("effort: high\n") && after_b.contains("sprint: 1\n") && after_b.contains("Body of B"));
    assert!(raw(&root, "T-003").contains("depends: T-001\n"));
    assert_eq!(blk(&app, "T-003"), ["T-001"]);
    let agents = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains("| T-001 | task | backlog | - | - | T-002, T-003 | A |"), "{agents}");
    app.reload();
    assert_eq!(deps(&app, "T-002"), ["T-003", "T-001"]);

    // prefill comes from the dependents; the dependent's own form shows the other side
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!(f.text("blocks"), "T-002, T-003");
    app.select_task("T-003");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!((f.text("depends").as_str(), f.text("blocks").as_str()), ("T-001", "T-002"));

    // removing reverses it: only the removed card changes
    edit(&mut app, "T-001", &[("blocks", "T-003")]).unwrap();
    assert_eq!(deps(&app, "T-002"), ["T-003"]);
    assert!(raw(&root, "T-002").contains("depends: T-003\n"));
    assert_eq!(deps(&app, "T-003"), ["T-001"]);
    // clearing the last edge drops the depends key of that card
    edit(&mut app, "T-001", &[("blocks", "")]).unwrap();
    assert!(deps(&app, "T-003").is_empty());
    assert!(!raw(&root, "T-003").contains("depends"));
    assert!(blk(&app, "T-003").is_empty());
    // re-adding after a reload works the same (the file is the single source of truth)
    app.reload();
    edit(&mut app, "T-001", &[("blocks", "T-003")]).unwrap();
    assert!(raw(&root, "T-003").contains("depends: T-001\n"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn blocks_validation_and_cycles() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    let clean = files_of(&root);
    // same id in both fields, self, unknown
    let e = edit(&mut app, "T-001", &[("depends", "T-002"), ("blocks", "T-002")]).unwrap_err();
    assert!(e.contains("T-002") && e.contains("одновременно"), "{e}");
    let e = edit(&mut app, "T-001", &[("blocks", "T-001")]).unwrap_err();
    assert!(e.contains("T-001") && e.contains("саму себя"), "{e}");
    let e = edit(&mut app, "T-001", &[("blocks", "T-002, ZZZ-9")]).unwrap_err();
    assert!(e.contains("ZZZ-9") && e.contains("нет"), "{e}");
    assert_eq!(files_of(&root), clean);
    assert!(app.tasks.iter().all(|t| t.depends.is_empty()));

    // cycle through the combination: T-001 -> T-002 -> T-003, then T-001 blocks T-003
    edit(&mut app, "T-001", &[("depends", "T-002")]).unwrap();
    edit(&mut app, "T-002", &[("depends", "T-003")]).unwrap();
    let clean = files_of(&root);
    let e = edit(&mut app, "T-001", &[("blocks", "T-003")]).unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-001 → T-002 → T-003 → T-001"), "{e}");
    assert!(deps(&app, "T-003").is_empty());
    assert_eq!(files_of(&root), clean);
    // the same ring from T-003's form: it depends on T-001 while T-002 (prefilled in «Блокирует») depends on it
    let e = edit(&mut app, "T-003", &[("depends", "T-001")]).unwrap_err();
    assert!(e.contains("Цикл"), "{e}");
    assert_eq!(files_of(&root), clean);
    // flipping an edge is not a cycle: T-001 stops depending on T-002, T-002 depends on T-001 instead
    edit(&mut app, "T-001", &[("depends", ""), ("blocks", "T-002")]).unwrap();
    assert!(deps(&app, "T-001").is_empty());
    assert_eq!(deps(&app, "T-002"), ["T-003", "T-001"]);

    // via A's depends after A blocks B and B blocks C
    let (mut app, root2) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    edit(&mut app, "T-001", &[("blocks", "T-002")]).unwrap();
    edit(&mut app, "T-002", &[("blocks", "T-003")]).unwrap();
    assert_eq!((deps(&app, "T-002"), deps(&app, "T-003")), (vec!["T-001".to_string()], vec!["T-002".to_string()]));
    let clean = files_of(&root2);
    let e = edit(&mut app, "T-001", &[("depends", "T-003")]).unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-001 → T-003 → T-002 → T-001"), "{e}");
    // B blocks A via B's form: B already depends on A, so the same id is rejected
    let e = edit(&mut app, "T-002", &[("blocks", "T-001, T-003")]).unwrap_err();
    assert!(e.contains("T-001") && e.contains("одновременно"), "{e}");
    // C blocks A closes the ring
    let e = edit(&mut app, "T-003", &[("blocks", "T-001")]).unwrap_err();
    assert!(e.contains("Цикл"), "{e}");
    assert_eq!(files_of(&root2), clean);
    // through the key path the form stays open with a red status and nothing changes
    app.select_task("T-003");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "blocks", "T-001");
    app.mode = Mode::Form(f);
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(app.status.as_ref().is_some_and(|s| s.1 && s.0.contains("Цикл")));
    assert!(deps(&app, "T-001").is_empty());
    assert_eq!(files_of(&root2), clean);
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn blocks_trash_rules() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C", "D"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    edit(&mut app, "T-001", &[("blocks", "T-002")]).unwrap();
    trash(&mut app, "T-002");
    trash(&mut app, "T-004");
    // the trash list sorts by deleted_at (whole seconds, newest first, ties by id): pin the
    // stamps so the restore below picks T-002 whatever second each trash keypress landed in
    for (id, v) in [("T-002", 200), ("T-004", 100)] {
        app.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = Some(v);
    }
    // trashed dependent: not in the prefill, not offered, not counted
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!(f.text("blocks"), "");
    assert!(f.cards.iter().all(|c| c.id != "T-002" && c.id != "T-004") && f.cards.iter().any(|c| c.id == "T-003"));
    assert!(app.dependents("T-001").is_empty());
    // a new trashed pick is rejected; the existing trashed edge may be typed again without a change
    let e = edit(&mut app, "T-001", &[("blocks", "T-004")]).unwrap_err();
    assert!(e.contains("T-004") && e.contains("корзине"), "{e}");
    edit(&mut app, "T-001", &[("blocks", "T-002, T-003"), ("title", "A2")]).unwrap();
    // the trashed dependent stays untouched even when it is absent from the field
    edit(&mut app, "T-001", &[("blocks", "T-003")]).unwrap();
    assert_eq!(deps(&app, "T-002"), ["T-001"]);
    assert!(raw(&root, "T-002").contains("depends: T-001\n") && raw(&root, "T-002").contains("deleted: true"));
    assert_eq!(deps(&app, "T-003"), ["T-001"]);
    // clearing the field removes only live dependents
    edit(&mut app, "T-001", &[("blocks", "")]).unwrap();
    assert!(deps(&app, "T-003").is_empty());
    assert_eq!(deps(&app, "T-002"), ["T-001"]);
    // restore: the card is a dependent again
    app.on_key(k('B'));
    app.on_key(k('r'));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.dependents("T-001").len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn new_card_with_blocks() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    add(&mut app, Kind::Task, "B", "", "");
    add(&mut app, Kind::Task, "C", "", "");
    set_depends(&mut app, "T-002", "T-001").unwrap();
    // the id exists only at save: the reverse edges name it
    let id = create(&mut app, &[("title", "N"), ("depends", "T-003"), ("blocks", "T-001")]).unwrap();
    assert_eq!(id, "T-004");
    assert_eq!(deps(&app, "T-001"), ["T-004"]);
    assert_eq!(deps(&app, "T-004"), ["T-003"]);
    assert!(raw(&root, "T-001").contains("depends: T-004\n"));
    assert!(raw(&root, "T-004").contains("depends: T-003\n"));
    assert_eq!(app.dependents("T-004").iter().map(|t| t.id.clone()).collect::<Vec<_>>(), ["T-001"]);
    assert_eq!(cur(&app), "T-004");
    // a cycle through the not-yet-existing card: M depends on B, B depends on A, M blocks A
    let n = app.tasks.len();
    let clean = files_of(&root);
    let e = create(&mut app, &[("title", "M"), ("depends", "T-002"), ("blocks", "T-001")]).unwrap_err();
    assert!(e.contains("Цикл") && e.contains("T-005"), "{e}");
    assert_eq!((app.tasks.len(), files_of(&root)), (n, clean));
    assert!(!root.join(".pumba/tasks/T-005.md").exists());
    // unknown / trashed ids cannot be picked for a new card either
    assert!(create(&mut app, &[("title", "M"), ("blocks", "ZZZ-1")]).is_err());
    trash(&mut app, "T-003");
    let e = create(&mut app, &[("title", "M"), ("blocks", "T-003")]).unwrap_err();
    assert!(e.contains("корзине"), "{e}");
    // epics get the field and the same behaviour
    app.open_task_form(Kind::Epic, None);
    let mut f = take_form(&mut app);
    assert_eq!(f.text("blocks"), "");
    set_text(&mut f, "title", "Эпик");
    set_text(&mut f, "blocks", "T-002");
    app.apply_form(&f).unwrap();
    assert_eq!(deps(&app, "T-002"), ["T-001", "E-001"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn blocks_save_rolls_back_files_on_io_error() {
    let (mut app, root) = fresh();
    for n in ["A", "B", "C"] {
        add(&mut app, Kind::Task, n, "", "");
    }
    let clean = (raw(&root, "T-001"), raw(&root, "T-002"));
    let agents = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    // T-003 cannot be written: its file name is taken by a directory
    let p3 = root.join(".pumba/tasks/T-003.md");
    std::fs::remove_file(&p3).unwrap();
    std::fs::create_dir(&p3).unwrap();
    app.select_task("T-001");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "title", "A changed");
    set_text(&mut f, "blocks", "T-002, T-003");
    app.mode = Mode::Form(f);
    save_key(&mut app);
    // form stays open with a red status; memory and disk as before (T-001, T-002 were written first, then restored)
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(app.status.as_ref().is_some_and(|s| s.1 && s.0.contains("Ошибка записи")), "{:?}", app.status);
    assert_eq!(app.tasks[0].title, "A");
    assert!(deps(&app, "T-002").is_empty() && deps(&app, "T-003").is_empty());
    assert_eq!((raw(&root, "T-001"), raw(&root, "T-002")), clean);
    assert_eq!(std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap(), agents);
    // a new card whose save fails leaves no file behind
    app.mode = Mode::Board;
    app.open_task_form(Kind::Task, None);
    let mut f = take_form(&mut app);
    set_text(&mut f, "title", "N");
    set_text(&mut f, "blocks", "T-002, T-003");
    app.mode = Mode::Form(f);
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(!root.join(".pumba/tasks/T-004.md").exists());
    assert_eq!(app.tasks.len(), 3);
    assert_eq!(raw(&root, "T-002"), clean.1);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------- project defaults

#[test]
fn default_model_and_effort_for_new_cards() {
    let (mut app, root) = fresh();
    assert_eq!((app.cfg.default_model.as_str(), app.cfg.default_effort.as_str()), ("default", "default"));
    let first = add(&mut app, Kind::Task, "Old", "", "");
    // the selects come first, then both lists
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    let keys: Vec<&str> = f.fields.iter().map(|x| x.key.as_str()).collect();
    assert_eq!(keys, ["default_model", "default_effort", "models", "efforts"]);
    assert_eq!(f.fields[0].label, "Модель по умолчанию для новых карточек");
    assert_eq!(f.fields[1].label, "Уровень по умолчанию для новых карточек");
    let opts = |f: &Form, key: &str| match &f.fields.iter().find(|x| x.key == key).unwrap().kind {
        FieldKind::Select { opts, .. } => opts.iter().map(|o| o.value.clone()).collect::<Vec<_>>(),
        _ => panic!(),
    };
    assert_eq!(opts(&f, "default_model"), ["default", "opus", "sonnet", "haiku"]);
    assert_eq!(opts(&f, "default_effort"), ["default", "low", "medium", "high", "xhigh", "max"]);
    set_sel(&mut f, "default_model", "sonnet");
    set_sel(&mut f, "default_effort", "high");
    app.apply_form(&f).unwrap();
    assert_eq!(app.status.as_ref().map(|s| (s.0.as_str(), s.1)), Some(("Списки сохранены", false)));
    let cfg = app.store.load().unwrap().0;
    assert_eq!((cfg.default_model.as_str(), cfg.default_effort.as_str()), ("sonnet", "high"));
    let agents = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    assert!(agents.contains("Для новых карточек по умолчанию: модель sonnet, уровень high."));
    // the saved choice is shown again when the form reopens
    app.on_key(k('M'));
    let f = take_form(&mut app);
    assert_eq!((f.sel("default_model").as_str(), f.sel("default_effort").as_str()), ("sonnet", "high"));

    // new task and new epic forms preselect it; an existing card keeps its own values
    for key in ['n', 'N'] {
        app.on_key(k(key));
        let f = take_form(&mut app);
        assert_eq!((f.sel("model").as_str(), f.sel("effort").as_str()), ("sonnet", "high"), "{key}");
    }
    app.select_task(&first);
    app.on_key(k('e'));
    let f = take_form(&mut app);
    assert_eq!((f.sel("model").as_str(), f.sel("effort").as_str()), ("default", "default"));
    // a card created from the preselected form stores the values
    let id = add(&mut app, Kind::Task, "New", "", "");
    let t = app.tasks.iter().find(|t| t.id == id).unwrap();
    assert_eq!((t.model.as_str(), t.effort.as_str()), ("sonnet", "high"));
    assert!(raw(&root, &id).contains("model: sonnet\n") && raw(&root, &id).contains("effort: high\n"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn default_falls_back_when_removed_from_list() {
    let (mut app, root) = fresh();
    app.cfg.default_model = "opus".into();
    app.cfg.default_effort = "max".into();
    // the user drops opus (but keeps max) in the same form
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    assert_eq!((f.sel("default_model").as_str(), f.sel("default_effort").as_str()), ("opus", "max"));
    set_text(&mut f, "models", "sonnet\nhaiku");
    app.apply_form(&f).unwrap();
    assert_eq!((app.cfg.default_model.as_str(), app.cfg.default_effort.as_str()), ("default", "max"));
    let st = app.status.as_ref().unwrap();
    assert!(!st.1 && st.0.contains("Модель по умолчанию сброшена"), "{}", st.0);
    let cfg = app.store.load().unwrap().0;
    assert_eq!((cfg.default_model.as_str(), cfg.default_effort.as_str()), ("default", "max"));
    // effort removed too
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    set_text(&mut f, "efforts", "low");
    app.apply_form(&f).unwrap();
    assert_eq!(app.cfg.default_effort, "default");
    assert!(app.status.as_ref().unwrap().0.contains("Уровень по умолчанию сброшен"));
    // a stale saved default stays visible as an extra option; "default" is always valid
    app.cfg.default_model = "ghost".into();
    app.on_key(k('M'));
    let mut f = take_form(&mut app);
    assert_eq!(f.sel("default_model"), "ghost");
    set_sel(&mut f, "default_model", "default");
    app.apply_form(&f).unwrap();
    assert_eq!(app.cfg.default_model, "default");
    assert_eq!(app.status.as_ref().map(|s| s.0.as_str()), Some("Списки сохранены"));
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ pipeline columns (agent / next)

fn opt_values(f: &Form, key: &str) -> Vec<String> {
    match &f.fields.iter().find(|x| x.key == key).unwrap().kind {
        FieldKind::Select { opts, .. } => opts.iter().map(|o| o.value.clone()).collect(),
        _ => panic!("{key} is not a select"),
    }
}

fn col<'a>(app: &'a App, id: &str) -> &'a Column {
    app.cfg.columns.iter().find(|c| c.id == id).unwrap()
}

#[test]
fn column_form_new_and_edit_agent_and_next() {
    let (mut app, root) = fresh();
    // new column: fields in order, `next` offers «нет» and every column by name
    app.on_key(k('c'));
    let mut f = take_form(&mut app);
    let keys: Vec<&str> = f.fields.iter().map(|x| x.key.as_str()).collect();
    assert_eq!(keys, ["name", "before", "rule", "after", "agent", "next", "color"]);
    // the rule parts on the left, the rest in the side column
    let sides: Vec<bool> = f.fields.iter().map(|x| x.side).collect();
    assert_eq!(sides, [false, false, false, false, true, true, true]);
    let multi = |k: &str| matches!(&f.fields.iter().find(|x| x.key == k).unwrap().kind, FieldKind::Text(tb) if tb.multiline);
    assert!(multi("before") && multi("rule") && multi("after") && !multi("name") && !multi("agent"));
    assert_eq!(f.fields[1].label, "Перед работой (проверки, стоп)");
    assert_eq!(f.fields[2].label, "Работа (что делает исполнитель)");
    assert_eq!(f.fields[3].label, "После работы (проверка, куда)");
    assert_eq!(f.fields[4].label, "Исполнитель (пусто — нет)");
    assert_eq!(f.fields[5].label, "После успеха");
    assert_eq!(opt_values(&f, "next"), ["", "backlog", "in-progress", "review", "on-dev", "done"]);
    match &f.fields[5].kind {
        FieldKind::Select { opts, idx } => {
            assert_eq!((*idx, opts[0].label.as_str(), opts[2].label.as_str()), (0, "нет", "В работе"));
        }
        _ => panic!(),
    }
    set_text(&mut f, "name", "QA");
    set_text(&mut f, "before", "Проверь, что тесты есть.\nНет — остановись.  ");
    set_text(&mut f, "rule", "Прогони приёмку.");
    set_text(&mut f, "after", "Приёмка прошла — дальше.\n");
    set_text(&mut f, "agent", "  pumba-tester  ");
    set_sel(&mut f, "next", "review");
    app.apply_form(&f).unwrap();
    let qa = col(&app, "qa");
    assert_eq!((qa.agent.as_str(), qa.next.as_str()), ("pumba-tester", "review"));
    // trailing whitespace is trimmed, inner lines are kept
    assert_eq!(
        (qa.before.as_str(), qa.rule.as_str(), qa.after.as_str()),
        ("Проверь, что тесты есть.\nНет — остановись.", "Прогони приёмку.", "Приёмка прошла — дальше.")
    );
    let disk = app.store.load().unwrap().0;
    let qa = disk.columns.iter().find(|c| c.id == "qa").unwrap();
    assert_eq!((qa.agent.as_str(), qa.next.as_str()), ("pumba-tester", "review"));
    assert_eq!((qa.before.as_str(), qa.rule.as_str(), qa.after.as_str()), (col(&app, "qa").before.as_str(), "Прогони приёмку.", "Приёмка прошла — дальше."));
    let md = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
    assert!(md.contains(
        "### QA (`qa`)\n\n**Перед работой:** Проверь, что тесты есть.\nНет — остановись.\n\n**Работа** (исполнитель: `pumba-tester`): Прогони приёмку.\n\n**После работы:** Приёмка прошла — дальше. После успеха: переместить в «На ревью» (`review`)."
    ), "{md}");

    // edit: prefilled, the column itself is not offered, the id never changes
    app.sel_col = app.cfg.columns.iter().position(|c| c.id == "in-progress").unwrap();
    app.on_key(k('r'));
    let mut f = take_form(&mut app);
    assert_eq!((f.text("agent").as_str(), f.sel("next").as_str()), ("pumba-worker", "review"));
    let defaults = Config::default();
    let ip = defaults.columns.iter().find(|c| c.id == "in-progress").unwrap();
    assert_eq!((f.text("before"), f.text("rule"), f.text("after")), (ip.before.clone(), ip.rule.clone(), ip.after.clone()));
    // one part edited, the others are kept as they were
    set_text(&mut f, "after", "Только отчёт.");
    assert_eq!(opt_values(&f, "next"), ["", "backlog", "review", "on-dev", "qa", "done"]);
    set_text(&mut f, "name", "Разработка");
    set_text(&mut f, "agent", "");
    set_sel(&mut f, "next", "on-dev");
    app.apply_form(&f).unwrap();
    let c = col(&app, "in-progress");
    assert_eq!((c.name.as_str(), c.agent.as_str(), c.next.as_str()), ("Разработка", "", "on-dev"));
    assert_eq!((c.before.as_str(), c.rule.as_str(), c.after.as_str()), (ip.before.as_str(), ip.rule.as_str(), "Только отчёт."));
    let disk = app.store.load().unwrap().0;
    let c = disk.columns.iter().find(|c| c.id == "in-progress").unwrap();
    assert_eq!((c.agent.as_str(), c.next.as_str()), ("", "on-dev"));
    assert_eq!((c.before.as_str(), c.after.as_str()), (ip.before.as_str(), "Только отчёт."));
    // «нет» clears the pointer
    app.on_key(k('r'));
    let mut f = take_form(&mut app);
    set_sel(&mut f, "next", "");
    app.apply_form(&f).unwrap();
    assert!(col(&app, "in-progress").next.is_empty());
    // another column still pointing at the renamed one is untouched (ids are stable)
    assert_eq!(col(&app, "backlog").next, "");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn column_form_validates_next() {
    let (mut app, root) = fresh();
    app.sel_col = 1; // in-progress
    app.on_key(k('r'));
    let mut f = take_form(&mut app);
    let before = app.cfg.clone();
    // an id that does not exist
    if let Some(Field { kind: FieldKind::Select { opts, idx }, .. }) = f.fields.iter_mut().find(|x| x.key == "next") {
        opts.push(opt("ghost", "ghost", ""));
        *idx = opts.len() - 1;
    }
    let e = app.apply_form(&f).unwrap_err();
    assert!(e.contains("ghost") && e.contains("нет"), "{e}");
    // the column itself
    if let Some(Field { kind: FieldKind::Select { opts, idx }, .. }) = f.fields.iter_mut().find(|x| x.key == "next") {
        opts.push(opt("in-progress", "self", ""));
        *idx = opts.len() - 1;
    }
    let e = app.apply_form(&f).unwrap_err();
    assert!(e.contains("сама в себя"), "{e}");
    assert_eq!(app.cfg, before);
    // through the key path the form stays open with a red status and nothing is written
    app.mode = Mode::Form(f);
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Form(_)));
    assert!(app.status.as_ref().is_some_and(|s| s.1));
    assert_eq!(app.store.load().unwrap().0, before);
    // an empty name is still rejected
    let mut f = take_form(&mut app);
    set_text(&mut f, "name", " ");
    assert!(app.apply_form(&f).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_next_shows_as_none_in_the_form() {
    let (mut app, root) = fresh();
    // an external edit left review pointing at a missing column and on-dev at itself
    app.cfg.columns.iter_mut().find(|c| c.id == "review").unwrap().next = "gone".into();
    app.cfg.columns.iter_mut().find(|c| c.id == "on-dev").unwrap().next = "on-dev".into();
    for (i, want) in [(2usize, ""), (3, "")] {
        app.open_column_form(Some(i));
        let f = take_form(&mut app);
        assert_eq!(f.sel("next"), want);
        app.apply_form(&f).unwrap(); // saving fixes the pointer instead of failing
    }
    assert!(col(&app, "review").next.is_empty() && col(&app, "on-dev").next.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ reload keeps AGENTS.md fresh

fn agents_file(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap()
}

/// Rewrite a card file behind the app's back after a pause long enough for a new mtime.
fn external_write(root: &std::path::Path, id: &str, f: impl Fn(&str) -> String) {
    std::thread::sleep(Duration::from_millis(30));
    let p = root.join(format!(".pumba/tasks/{id}.md"));
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, f(&text)).unwrap();
}

#[test]
fn reload_regenerates_agents_md_once_after_an_external_move() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    assert!(agents_file(&root).contains("| T-001 | task | backlog |"));
    let w = app.agents_writes;
    // an agent moves the card: the next tick reloads and refreshes the index
    external_write(&root, "T-001", |t| t.replace("column: backlog", "column: in-progress"));
    app.tick();
    assert_eq!(app.agents_writes, w + 1);
    assert!(agents_file(&root).contains("| T-001 | task | in-progress |"));
    assert_eq!(app.tasks[0].column, "in-progress");
    // our own AGENTS.md write is invisible to the signature: no further reloads or writes
    for _ in 0..3 {
        app.tick();
    }
    app.reload();
    assert_eq!(app.agents_writes, w + 1);
    // an unchanged reload does not write either
    let stamp = std::fs::metadata(root.join(".pumba/AGENTS.md")).unwrap().modified().unwrap();
    app.reload();
    assert_eq!(app.agents_writes, w + 1);
    assert_eq!(std::fs::metadata(root.join(".pumba/AGENTS.md")).unwrap().modified().unwrap(), stamp);
    // a new external card is picked up and indexed once
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(root.join(".pumba/tasks/T-050.md"), "---\ntitle: From agent\ncolumn: backlog\n---\n\nb\n").unwrap();
    app.tick();
    assert_eq!(app.agents_writes, w + 2);
    assert!(agents_file(&root).contains("| T-050 | task | backlog |"));
    // an external board.json edit (a new rule text) too
    std::thread::sleep(Duration::from_millis(30));
    let p = root.join(".pumba/board.json");
    let raw = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, raw.replace("\"sprint\": 1", "\"sprint\": 4")).unwrap();
    app.tick();
    assert_eq!((app.agents_writes, app.cfg.sprint), (w + 3, 4));
    assert!(agents_file(&root).contains("Текущий спринт: 4"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn reload_does_not_write_after_a_failed_load() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    let w = app.agents_writes;
    let md = agents_file(&root);
    let p = root.join(".pumba/board.json");
    let good = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, "{ broken").unwrap();
    app.reload();
    assert!(!app.ok);
    assert_eq!((app.agents_writes, agents_file(&root)), (w, md.clone()));
    // fixed again, even with a changed card: the previous load failed, so nothing is written yet
    external_write(&root, "T-001", |t| t.replace("column: backlog", "column: done"));
    std::fs::write(&p, good).unwrap();
    app.reload();
    assert!(app.ok);
    assert_eq!((app.agents_writes, agents_file(&root)), (w, md));
    // the next change is indexed again
    external_write(&root, "T-001", |t| t.replace("column: done", "column: review"));
    app.tick();
    assert_eq!(app.agents_writes, w + 1);
    assert!(agents_file(&root).contains("| T-001 | task | review |"));
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ unreadable cards

#[test]
fn unreadable_cards_are_reported_not_hidden() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    assert!(app.unreadable.is_empty() && app.unreadable_note().is_none());
    let tasks = root.join(".pumba/tasks");
    std::fs::write(tasks.join("T-007.md"), "no frontmatter").unwrap();
    app.reload();
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Не прочитано: T-007.md", true));
    assert_eq!(app.tasks.len(), 1);
    // more than three: the first three names and the rest as a count
    for n in ["T-008", "T-009", "T-010", "T-011"] {
        std::fs::write(tasks.join(format!("{n}.md")), [0xffu8, 0xfe]).unwrap();
    }
    app.reload();
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Не прочитано: T-007.md, T-008.md, T-009.md и ещё 2", true));
    // a card with a BOM and CRLF is read, not reported
    std::fs::write(tasks.join("T-020.md"), "\u{feff}---\r\ntitle: Win\r\ncolumn: backlog\r\n---\r\n\r\nbody\r\n").unwrap();
    for n in ["T-007", "T-008", "T-009", "T-010", "T-011"] {
        std::fs::remove_file(tasks.join(format!("{n}.md"))).unwrap();
    }
    app.reload();
    assert!(app.unreadable.is_empty() && app.status.is_none(), "{:?}", app.status);
    assert!(app.tasks.iter().any(|t| t.id == "T-020" && t.title == "Win"));
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ lost-update protection

const NOTES: &str = "\n## Ход работы\n- 2026-10-31 14:05 — «В работе» → «На ревью»: проверки зелёные.\n";

fn append_notes(root: &std::path::Path, id: &str) {
    external_write(root, id, |t| format!("{}{NOTES}", t.trim_end()));
}

fn disk_task(app: &App, id: &str) -> Task {
    Task::from_md(&app.store.read_task_raw(id).unwrap(), id).unwrap()
}

#[test]
fn card_move_keeps_text_appended_outside() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    add(&mut app, Kind::Task, "B", "", "");
    app.select_task("T-001");
    append_notes(&root, "T-001");
    // no reload in between: the app still holds the old body
    assert!(!app.tasks[0].body.contains("Ход работы"));
    app.on_key(k('L'));
    let t = disk_task(&app, "T-001");
    assert_eq!(t.column, "in-progress");
    assert!(t.body.contains("## Ход работы") && t.body.contains("проверки зелёные"), "{}", t.body);
    // memory caught up as well
    assert!(app.tasks.iter().find(|t| t.id == "T-001").unwrap().body.contains("Ход работы"));
    assert_eq!(cur(&app), "T-001");
    // every other operation that only rewrites the card keeps it too: hide, sprint, reorder
    for (n, key) in ['x', 'p'].into_iter().enumerate() {
        append_notes(&root, "T-001");
        app.select_task("T-001");
        app.show_hidden = true;
        app.on_key(k(key));
        let t = disk_task(&app, "T-001");
        assert_eq!(t.body.matches("## Ход работы").count(), 2 + n, "{key}: {}", t.body);
    }
    append_notes(&root, "T-001");
    app.select_task("T-001");
    app.on_key(k('H'));
    app.on_key(k('K'));
    assert_eq!(disk_task(&app, "T-001").body.matches("## Ход работы").count(), 4);
    // trash and restore
    append_notes(&root, "T-002");
    trash(&mut app, "T-002");
    let t = disk_task(&app, "T-002");
    assert!(t.deleted && t.body.contains("проверки зелёные"));
    append_notes(&root, "T-002");
    app.on_key(k('B'));
    app.on_key(k('r'));
    let t = disk_task(&app, "T-002");
    assert!(!t.deleted && t.body.matches("## Ход работы").count() == 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn edit_form_keeps_disk_body_when_description_untouched() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    edit(&mut app, "T-001", &[("body", "Original text")]).unwrap();
    // open the form, then an agent appends notes while it is open
    app.select_task("T-001");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    assert_eq!(f.text("body"), "Original text");
    append_notes(&root, "T-001");
    set_text(&mut f, "title", "A renamed");
    app.mode = Mode::Form(f);
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Board));
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Файл изменён снаружи: описание не перезаписано", true));
    let t = disk_task(&app, "T-001");
    assert_eq!(t.title, "A renamed");
    assert!(t.body.starts_with("Original text") && t.body.contains("проверки зелёные"), "{}", t.body);
    assert_eq!(app.tasks[0].body, t.body);

    // the user edited the description: their text is saved, with a warning
    app.select_task("T-001");
    app.on_key(k('e'));
    let mut f = take_form(&mut app);
    append_notes(&root, "T-001");
    set_text(&mut f, "body", "Rewritten by the user");
    app.mode = Mode::Form(f);
    save_key(&mut app);
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Файл изменён снаружи: сохранён ваш текст описания", true));
    assert_eq!(disk_task(&app, "T-001").body, "Rewritten by the user");

    // nothing changed outside: no warning
    app.status = None;
    app.select_task("T-001");
    app.on_key(k('e'));
    let f = take_form(&mut app);
    app.mode = Mode::Form(f);
    save_key(&mut app);
    assert!(app.status.is_none(), "{:?}", app.status);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unreadable_disk_copy_blocks_the_write() {
    let (mut app, root) = fresh();
    add(&mut app, Kind::Task, "A", "", "");
    let p = root.join(".pumba/tasks/T-001.md");
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&p, "garbage written by a crashed editor").unwrap();
    app.select_task("T-001");
    app.on_key(k('L'));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "garbage written by a crashed editor");
    let st = app.status.as_ref().unwrap();
    assert!(st.1 && st.0.contains("T-001.md") && st.0.contains("не читается"), "{}", st.0);
    let _ = std::fs::remove_dir_all(root);
}

// ------------------------------------------------ init + installer

#[test]
fn init_installs_claude_pack_and_reports() {
    let (app, root) = fresh();
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Создано. Установлено: 7 файлов, пропущено: 0", false));
    for rel in [".claude/agents/pumba-worker.md", ".claude/agents/pumba-reviewer.md", ".claude/skills/pumba-task/SKILL.md"] {
        assert!(root.join(rel).is_file(), "{rel}");
    }
    assert!(root.join(".pumba/board.json").is_file() && root.join(".pumba/tasks").is_dir());
    let _ = std::fs::remove_dir_all(root);
}

fn init_app(tag: &str) -> (App, PathBuf) {
    let root = std::env::temp_dir().join(format!("pumba-init-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let app = App::new(root.clone(), false);
    (app, root)
}

#[test]
fn init_confirm_text_and_existing_files_are_kept() {
    let (mut app, root) = init_app("keep");
    match &app.mode {
        Mode::Confirm(m, Action::Init) => assert_eq!(
            m,
            &format!("Папка .pumba/ не найдена. Создать её в {}?", root.display())
        ),
        _ => panic!("no init confirm"),
    }
    let mine = root.join(".claude/agents/pumba-worker.md");
    std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
    std::fs::write(&mine, "my own worker").unwrap();
    app.on_key(k('y'));
    // nothing is created before the tool choice is confirmed
    assert!(matches!(app.mode, Mode::InitTools(_)) && !root.join(".pumba").exists());
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    assert_eq!(std::fs::read_to_string(&mine).unwrap(), "my own worker");
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Создано. Установлено: 6 файлов, пропущено: 1", false));
    // declining quits without creating anything
    let (mut app2, root2) = init_app("no");
    app2.on_key(k('n'));
    assert!(app2.quit && !root2.join(".pumba").exists() && !root2.join(".claude").exists());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn installer_failure_keeps_pumba_and_shows_red_status() {
    let (mut app, root) = init_app("fail");
    std::fs::write(root.join(".claude"), "a file where the directory should be").unwrap();
    app.on_key(k('y'));
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    assert!(root.join(".pumba/board.json").is_file());
    let st = app.status.as_ref().unwrap();
    assert!(st.1 && st.0.contains(".pumba/ создана") && st.0.contains("не удалась"), "{}", st.0);
    let _ = std::fs::remove_dir_all(root);
}

fn pick(app: &App) -> &ToolPick {
    match &app.mode {
        Mode::InitTools(p) => p,
        _ => panic!("no tool picker"),
    }
}

fn saved_tools(root: &std::path::Path) -> Vec<String> {
    Store::new(root).saved_tools()
}

#[test]
fn init_flow_preselects_toggles_and_installs() {
    let (mut app, root) = init_app("flow");
    app.on_key(k('y'));
    // no tool found in the project: Claude Code is preselected, the first row is focused
    let p = pick(&app);
    let ids: Vec<&str> = p.items.iter().map(|(t, _)| t.id).collect();
    assert_eq!(ids, ["claude", "codex", "cursor", "copilot", "opencode"]);
    assert!(p.creating && p.checked == [true, false, false, false, false] && p.cursor == 0);
    assert!(p.items.iter().all(|(_, found)| !found));
    // Space unchecks, `a` checks all, `ф` (Cyrillic a) unchecks all, `a` checks all again
    app.on_key(k(' '));
    assert_eq!(pick(&app).checked, [false; 5]);
    app.on_key(k('a'));
    assert_eq!(pick(&app).checked, [true; 5]);
    app.on_key(k('ф'));
    assert_eq!(pick(&app).checked, [false; 5]);
    app.on_key(k('a'));
    assert_eq!(pick(&app).checked, [true; 5]);
    // walk down uncheck the other four, walk back up
    for _ in 0..4 {
        press(&mut app, KeyCode::Down);
        app.on_key(k(' '));
    }
    assert_eq!((pick(&app).cursor, pick(&app).checked.clone()), (4, vec![true, false, false, false, false]));
    press(&mut app, KeyCode::Down); // clamped at the last row
    assert_eq!(pick(&app).cursor, 4);
    for _ in 0..5 {
        press(&mut app, KeyCode::Up);
    }
    assert_eq!(pick(&app).cursor, 0);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    for rel in [".claude/agents/pumba-worker.md", ".claude/skills/pumba-new-card/SKILL.md"] {
        assert!(root.join(rel).is_file(), "{rel}");
    }
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Создано. Установлено: 7 файлов, пропущено: 0", false));
    assert_eq!(saved_tools(&root), ["claude"]);
    assert_eq!(app.cfg.tools, ["claude"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn init_flow_unchecked_enter_and_esc_create_only_pumba() {
    let (mut app, root) = init_app("none");
    app.on_key(k('y'));
    app.on_key(k(' '));
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    assert!(root.join(".pumba/board.json").is_file() && !root.join(".claude").exists());
    assert!(app.status.as_ref().unwrap().0.starts_with("Создано. Агенты не установлены"));
    assert!(saved_tools(&root).is_empty());

    let (mut app2, root2) = init_app("esc");
    app2.on_key(k('y'));
    press(&mut app2, KeyCode::Esc);
    assert!(matches!(app2.mode, Mode::Board) && app2.ok && !app2.quit);
    assert!(root2.join(".pumba/board.json").is_file() && !root2.join(".claude").exists());
    assert!(saved_tools(&root2).is_empty());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn init_flow_marks_detected_tool() {
    let (mut app, root) = init_app("detected");
    std::fs::write(root.join("CLAUDE.md"), "x").unwrap();
    app.on_key(k('y'));
    let p = pick(&app);
    assert!(p.items[0].1 && p.checked == [true, false, false, false, false]);
    assert!(p.items[1..].iter().all(|(_, found)| !found));
    let _ = std::fs::remove_dir_all(root);

    // several detected tools are preselected together, the rest are marked only when found
    let (mut app, root) = init_app("detected2");
    std::fs::create_dir_all(root.join(".cursor")).unwrap();
    std::fs::create_dir_all(root.join(".opencode")).unwrap();
    std::fs::write(root.join("AGENTS.md"), "shared").unwrap(); // not a marker of any tool
    app.on_key(k('y'));
    let p = pick(&app);
    let found: Vec<bool> = p.items.iter().map(|(_, f)| *f).collect();
    assert_eq!(found, [false, false, true, false, true]);
    assert_eq!(p.checked, [false, false, true, false, true]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn init_with_duplicate_packs_installs_and_shows_orange_warning() {
    let (mut app, root) = init_app("warn");
    app.on_key(k('y'));
    // everything selected: Claude + Codex + Cursor + Copilot + OpenCode
    app.on_key(k('a'));
    assert_eq!(pick(&app).checked, [true; 5]);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board) && app.ok);
    for rel in [".codex/agents/pumba-worker.toml", ".cursor/rules/pumba.mdc", ".github/agents/pumba-planner.agent.md", ".opencode/commands/pumba-task.md", ".agents/skills/pumba-task/SKILL.md"] {
        assert!(root.join(rel).is_file(), "{rel}");
    }
    let st = app.status.as_ref().unwrap();
    // not an error, but marked as a warning: first message plus «и ещё 2», then the summary
    assert!(!st.1 && app.status_warn, "{:?}", app.status);
    assert!(st.0.starts_with("Внимание: Cursor читает") && st.0.contains("(и ещё 2)"), "{}", st.0);
    assert!(st.0.ends_with("Создано. Установлено: 39 файлов, пропущено: 0"), "{}", st.0);
    assert_eq!(saved_tools(&root), ["claude", "codex", "cursor", "copilot", "opencode"]);

    // a later plain message clears the warning mark
    app.on_key(k('I'));
    press(&mut app, KeyCode::Enter); // all five are prefilled from board.json; nothing new to write
    let st = app.status.as_ref().unwrap();
    assert!(app.status_warn && st.0.contains("(и ещё 2)") && st.0.ends_with("Установлено: 0 файлов, пропущено: 39"), "{}", st.0);
    app.on_key(k('I'));
    app.on_key(k('a')); // none
    press(&mut app, KeyCode::Enter);
    assert!(!app.status_warn && app.status.as_ref().unwrap().0.starts_with("Инструменты не выбраны"));

    // a single tool: plain green info, no warning
    app.on_key(k('I'));
    assert_eq!(pick(&app).checked, [true; 5]); // nothing saved: every detected tool is preselected
    app.on_key(k('a')); // none
    app.on_key(k(' ')); // Claude only
    press(&mut app, KeyCode::Enter);
    assert!(!app.status_warn, "{:?}", app.status);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn i_key_installs_for_existing_board_without_overwriting() {
    let (mut app, root) = fresh();
    // fresh() installed Claude; the user edits a file, then reinstalls with `I` (also Cyrillic Ш)
    let mine = root.join(".claude/agents/pumba-worker.md");
    std::fs::write(&mine, "my own worker").unwrap();
    std::fs::remove_file(root.join(".claude/agents/pumba-tester.md")).unwrap();
    app.on_key(KeyEvent::new(KeyCode::Char('Ш'), KeyModifiers::SHIFT));
    // prefilled from cfg.tools
    let p = pick(&app);
    assert!(!p.creating && p.checked == [true, false, false, false, false]);
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.mode, Mode::Board));
    assert_eq!(std::fs::read_to_string(&mine).unwrap(), "my own worker");
    assert!(root.join(".claude/agents/pumba-tester.md").is_file());
    let st = app.status.as_ref().unwrap();
    assert_eq!((st.0.as_str(), st.1), ("Установлено: 1 файлов, пропущено: 6", false));
    assert_eq!(saved_tools(&root), ["claude"]);

    // deselect everything: nothing installed, the empty choice is saved
    std::fs::remove_file(root.join(".claude/agents/pumba-tester.md")).unwrap();
    app.on_key(k('I'));
    app.on_key(k(' '));
    assert_eq!(pick(&app).checked, [false; 5]);
    press(&mut app, KeyCode::Enter);
    assert!(!root.join(".claude/agents/pumba-tester.md").exists());
    assert!(saved_tools(&root).is_empty() && app.cfg.tools.is_empty());
    assert!(app.status.as_ref().unwrap().0.starts_with("Инструменты не выбраны"));

    // Esc changes nothing
    app.on_key(k('I'));
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Board) && !root.join(".claude/agents/pumba-tester.md").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn i_key_prefills_from_saved_tools_and_survives_legacy_board() {
    let (mut app, root) = fresh();
    // a legacy board.json without `tools`: the picker falls back to the project default
    app.cfg.tools.clear();
    app.on_key(k('I'));
    assert_eq!(pick(&app).checked, [true, false, false, false, false]);
    press(&mut app, KeyCode::Esc);
    let _ = std::fs::remove_dir_all(root);
}

/// Open the new-column form and focus the executor field (name, before, rule, after, agent).
fn open_agent_field(app: &mut App) {
    app.on_key(k('c'));
    for _ in 0..4 {
        press(app, KeyCode::Tab);
    }
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.fields[f.focus].key, "agent"),
        _ => panic!("no form open"),
    }
}

#[test]
fn agent_field_suggests_subagents_end_to_end() {
    let (mut app, root) = fresh();
    app.home = None; // never read the real user folder
    // the installed Claude pack is found; project folders of other tools join, deduped
    std::fs::create_dir_all(root.join(".cursor/agents")).unwrap();
    std::fs::write(root.join(".cursor/agents/pumba-worker.md"), "---\ndescription: cursor copy\n---\n").unwrap();
    std::fs::write(root.join(".claude/agents/my-helper.md"), "---\nname: my-helper\ndescription: \"Делает мелочи: быстро\"\n---\nbody").unwrap();
    open_agent_field(&mut app);
    let v = picker(&app).expect("popup opens on focus");
    assert_eq!(v.results, ["pumba-committer", "pumba-planner", "pumba-reviewer", "pumba-tester", "pumba-worker", "my-helper"]);
    let w = v.agents.iter().find(|a| a.name == "pumba-worker").unwrap();
    assert_eq!(w.tag, "claude, cursor");
    assert_eq!(v.agents[5].desc, "Делает мелочи: быстро");
    // Esc closes only the popup, the field is still empty
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Form(_)) && picker(&app).is_none());
    // typing brings it back; Enter inserts the bare name (a single value)
    type_in(&mut app, "wor");
    assert_eq!(picker(&app).unwrap().results, ["pumba-worker"]);
    press(&mut app, KeyCode::Enter);
    match &app.mode {
        Mode::Form(f) => assert_eq!((f.text("agent").as_str(), f.fields[f.focus].key.as_str()), ("pumba-worker", "agent")),
        _ => panic!(),
    }
    press(&mut app, KeyCode::Tab); // popup is closed: moves on
    match &mut app.mode {
        Mode::Form(f) => {
            set_text(f, "name", "Работа");
        }
        _ => panic!(),
    }
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Board));
    // the new column sits right before the pinned last one
    let n = app.cfg.columns.len();
    assert_eq!((app.cfg.columns[n - 2].name.as_str(), app.cfg.columns[n - 2].agent.as_str()), ("Работа", "pumba-worker"));
    assert_eq!(app.cfg.columns[n - 1].id, "done");
    // a new form rescans: a file added meanwhile shows up; free text stays allowed
    std::fs::write(root.join(".claude/agents/late.md"), "---\ndescription: late\n---\n").unwrap();
    open_agent_field(&mut app);
    assert!(picker(&app).unwrap().results.contains(&"late".to_string()));
    type_in(&mut app, "my own thing");
    assert!(picker(&app).is_none());
    // other forms never show executor suggestions
    press(&mut app, KeyCode::Esc);
    app.on_key(k('n'));
    assert!(picker(&app).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn user_level_agents_are_listed_read_only() {
    let (mut app, root) = fresh();
    let home = root.join("fakehome");
    std::fs::create_dir_all(home.join(".claude/agents")).unwrap();
    std::fs::write(home.join(".claude/agents/global-one.md"), "---\ndescription: g\n---\n").unwrap();
    app.home = Some(home.clone());
    open_agent_field(&mut app);
    assert!(picker(&app).unwrap().results.contains(&"global-one".to_string()));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Board));
    assert_eq!(std::fs::read_dir(home.join(".claude/agents")).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn epic_select_filters_by_typing() {
    let (mut app, root) = fresh();
    for t in ["Авторизация", "Платежи", "Отчёты", "Миграция БД", "Поиск", "Уведомления", "Админка", "Экспорт", "Импорт", "Мобильный клиент", "Платёжный шлюз", "Документация"] {
        add(&mut app, Kind::Epic, t, "", "");
    }
    app.on_key(k('n'));
    type_in(&mut app, "Новая");
    for _ in 0..5 {
        press(&mut app, KeyCode::Tab); // body, model, effort, sprint, parent
    }
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.fields[f.focus].key, "parent"),
        _ => panic!(),
    }
    type_in(&mut app, "мобил");
    assert_eq!(picker(&app).unwrap().results, ["E-010"]);
    press(&mut app, KeyCode::Enter);
    assert!(picker(&app).is_none());
    // Left/Right still cycle; Up/Down still move focus without a query
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Down);
    match &app.mode {
        Mode::Form(f) => assert_eq!((f.sel("parent").as_str(), f.fields[f.focus].key.as_str()), ("E-011", "depends")),
        _ => panic!(),
    }
    press(&mut app, KeyCode::Up);
    type_in(&mut app, "док");
    press(&mut app, KeyCode::Tab);
    save_key(&mut app);
    assert!(matches!(app.mode, Mode::Board));
    assert_eq!(app.tasks.last().unwrap().parent.as_deref(), Some("E-012"));
    // the edit form of a card keeps its epic selected and lets the filter change it
    let id = app.tasks.last().unwrap().id.clone();
    app.select_task(&id);
    app.on_key(k('e'));
    match &app.mode {
        Mode::Form(f) => assert_eq!(f.sel("parent"), "E-012"),
        _ => panic!(),
    }
    let _ = std::fs::remove_dir_all(root);
}
