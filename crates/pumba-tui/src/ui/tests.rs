use super::board::CARD_H;
use super::card::agent_level;
use super::draw;
use super::util::*;
use crate::app::{Action, App, Mode};
use crate::editor::TextBuf;
use crate::form::{FieldKind, Form};
use crate::logo;
use crate::theme::*;
use ratatui::prelude::*;
use pumba_core::model::{Kind, Store, Task};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn task(id: &str, kind: Kind, title: &str, col: &str) -> Task {
    Task {
        id: id.into(),
        kind,
        title: title.into(),
        column: col.into(),
        parent: None,
        model: "default".into(),
        effort: "default".into(),
        sprint: None,
        hidden: false,
        order: 0,
        depends: Vec::new(),
        deleted: false,
        deleted_at: None,
        body: String::new(),
    }
}

fn dump(term: &Terminal<TestBackend>, label: &str) -> String {
    let buf = term.backend().buffer();
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    println!("===== {label} =====\n{s}");
    s
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn make_app() -> App {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pumba-ui-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::new(&dir);
    store.init().unwrap();
    let mut e = task("E-001", Kind::Epic, "Authentication overhaul with a really long epic title that must truncate", "in-progress");
    e.model = "opus".into();
    e.order = 1;
    let mut t1 = task("T-001", Kind::Task, "Login form", "in-progress");
    t1.parent = Some("E-001".into());
    t1.model = "sonnet".into();
    t1.effort = "high".into();
    t1.sprint = Some(1);
    t1.order = 2;
    t1.body = "Build the login form.\nIt must validate input and show errors inline, with a body long enough to need wrapping in the detail pane of the board.".into();
    let mut t2 = task("T-002", Kind::Task, "Hidden chore", "backlog");
    t2.hidden = true;
    t2.order = 3;
    let mut t3 = task("T-003", Kind::Task, "Ship it", "done");
    t3.parent = Some("E-001".into());
    t3.order = 4;
    let mut t4 = task("T-004", Kind::Task, "Unicode заголовок задачи ✓", "backlog");
    t4.order = 5;
    let mut t5 = task("T-005", Kind::Task, "Blocked by two", "backlog");
    t5.depends = vec!["T-001".into(), "T-004".into(), "T-003".into(), "X-999".into()];
    t5.order = 6;
    let mut t6 = task("T-006", Kind::Task, "Deps all done", "backlog");
    t6.depends = vec!["T-003".into()];
    t6.model = "opus".into();
    t6.sprint = Some(1);
    t6.parent = Some("E-001".into());
    t6.hidden = true;
    t6.order = 7;
    for t in [&e, &t1, &t2, &t3, &t4, &t5, &t6] {
        store.save_task(t).unwrap();
    }
    App::new(dir, true)
}

#[test]
fn smoke_render() {
    let mut app = make_app();
    for (w, h) in [(120u16, 35u16), (80, 24), (40, 12)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();

        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("board {w}x{h}"));
        assert!(s.contains("PUMBA"));

        app.show_hidden = true;
        app.sel_col = 1;
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("board hidden shown, col 1, {w}x{h}"));
        app.show_hidden = false;
        app.sel_col = 0;

        app.on_key(key(KeyCode::Char('n')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("new task form {w}x{h}"));
        for c in "A fairly long title that is typed into the form to test horizontal scrolling of the single line field".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::Tab));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("task form, later field focused {w}x{h}"));
        app.on_key(key(KeyCode::Esc));

        app.on_key(key(KeyCode::Char('P')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("rules form {w}x{h}"));
        app.on_key(key(KeyCode::Esc));

        app.on_key(key(KeyCode::Char('M')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("models form {w}x{h}"));
        if w >= 80 {
            assert!(s.contains("Модели и уровни") && s.contains("Уровни размышления"));
        }
        app.on_key(key(KeyCode::Tab));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("models form, second field {w}x{h}"));
        app.on_key(key(KeyCode::Esc));

        app.on_key(key(KeyCode::Char('?')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("help {w}x{h}"));
        app.on_key(key(KeyCode::Esc));

        app.on_key(key(KeyCode::Char('d')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        dump(&term, &format!("confirm {w}x{h}"));
        app.on_key(key(KeyCode::Char('n')));
        app.on_key(key(KeyCode::Esc));
    }
}

#[test]
fn init_confirm_logo() {
    let dir = std::env::temp_dir().join(format!("pumba-ui-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::new(dir, false);
    assert!(matches!(app.mode, Mode::Confirm(_, Action::Init)));
    for (w, h) in [(120u16, 35u16), (80, 24), (40, 12)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("init confirm {w}x{h}"));
        assert!(s.contains("Подтверждение"), "{w}x{h}");
        let has_art = s.contains('▀');
        if (w, h) == (120, 35) {
            assert!(has_art, "{w}x{h}");
            assert!(s.contains(logo::TITLE));
        }
        if h < (logo::H / 2) as u16 + 8 {
            assert!(!has_art, "{w}x{h}");
        }
    }
}

#[test]
fn init_tools_popup_render() {
    let dir = std::env::temp_dir().join(format!("pumba-ui-tools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    let mut app = App::new(dir.clone(), false);
    // the question first (with the new text), no tool list yet
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "init question 80x24");
    assert!(s.contains("Создать её в") && !s.contains("Claude Code"));
    assert!(s.contains("[y]") && s.contains("[n]"));
    app.on_key(key(KeyCode::Char('y')));
    assert!(matches!(app.mode, Mode::InitTools(_)));
    for (w, h) in [(120u16, 35u16), (80, 24), (40, 12), (30, 8)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("init tools {w}x{h}"));
        if w >= 80 {
            assert!(s.contains("Для каких инструментов установить агентов?"), "{w}x{h}");
            assert!(s.contains("[x] Claude Code") && s.contains("найден в проекте"), "{w}x{h}");
            assert!(s.contains("Пробел") && s.contains("Enter") && s.contains("только .pumba/"), "{w}x{h}");
            assert!(s.contains("▶"), "{w}x{h}");
        }
    }
    app.on_key(key(KeyCode::Char(' ')));
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "init tools unchecked").contains("[ ] Claude Code"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tools_popup_lists_five_tools_and_scrolls_when_short() {
    let mut app = make_app();
    app.on_key(key(KeyCode::Char('I')));
    let names = ["Claude Code", "OpenAI Codex", "Cursor", "GitHub Copilot", "OpenCode"];
    // 80x24: the whole list fits, the hints stay visible
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "tools popup, five rows 80x24");
    for n in names {
        assert!(s.contains(&format!("] {n}")), "{n}");
    }
    assert!(s.contains("Для каких инструментов") && s.contains("Пробел") && s.contains("Esc: закрыть"));
    // too short for five rows: the focused row is always scrolled into view
    for h in [10u16, 8, 7] {
        let mut app = make_app();
        app.on_key(key(KeyCode::Char('I')));
        for (i, n) in names.iter().enumerate() {
            let mut term = Terminal::new(TestBackend::new(80, h)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            let s = dump(&term, &format!("tools popup scroll 80x{h} row {i}"));
            let line = s.lines().find(|l| l.contains(n)).unwrap_or_else(|| panic!("80x{h}: «{n}» не виден"));
            assert!(line.contains('▶'), "80x{h}: фокус на «{n}»: {line}");
            app.on_key(key(KeyCode::Down));
        }
    }
}

#[test]
fn warning_status_is_orange_and_errors_stay_red() {
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let color_of = |term: &Terminal<TestBackend>, needle: &str| {
        let buf = term.backend().buffer();
        let row = (0..buf.area.height).find(|&y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().contains(needle)).expect(needle);
        let x = (0..buf.area.width).find(|&x| buf[(x, row)].symbol() == needle.chars().next().unwrap().to_string()).unwrap();
        buf[(x, row)].fg
    };
    app.status = Some(("Внимание: Cursor читает чужие копии (и ещё 1)".into(), false, std::time::Instant::now()));
    app.status_warn = true;
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(color_of(&term, "Внимание"), ORANGE);
    app.status_warn = false;
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(color_of(&term, "Внимание"), GREEN);
    app.status = Some(("Внимание: ошибка".into(), true, std::time::Instant::now()));
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(color_of(&term, "Внимание"), RED);
}

#[test]
fn tools_popup_on_existing_board_and_help_line() {
    let mut app = make_app();
    app.on_key(key(KeyCode::Char('I')));
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "tools popup, existing board 80x24");
    assert!(s.contains("Для каких инструментов") && s.contains("[x] Claude Code") && s.contains("Esc: закрыть"));
    assert!(!s.contains("найден в проекте"));
    app.on_key(key(KeyCode::Esc));
    app.mode = Mode::Help;
    let mut term = Terminal::new(TestBackend::new(120, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "help with I").contains("агенты и скиллы (инструменты)"));
}

#[test]
fn blocked_card_render() {
    let mut app = make_app();
    app.show_hidden = true;
    let ids = |app: &App| app.col_task_idx(0).iter().map(|&i| app.tasks[i].id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&app), ["T-002", "T-004", "T-005", "T-006"]);
    app.sel_col = 0;
    app.sel_idx = 2; // T-005
    for (w, h) in [(120u16, 35u16), (60, 24), (34, 20)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("blocked card {w}x{h}"));
        assert!(s.contains("⊘ T-001"), "{w}x{h}");
        if w >= 100 {
            assert!(s.contains("⊘ T-001,T-004"));
            assert!(s.contains("ЗАБЛОКИРОВАНА"));
            assert!(s.contains("зависит: T-001 (В работе), T-004 (Бэклог), T-003 (Готово), X-999 (?)"));
        }
    }
    // T-006 (done dependency only): card shows `✓ зав.`; cap is 3 cards, so it scrolls into view
    app.sel_idx = 3;
    let mut term = Terminal::new(TestBackend::new(120, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "done dep card").contains("✓ зав."));
    app.mode = Mode::Help;
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "help").contains("Зависит от"));
}

#[test]
fn card_sections_render() {
    assert_eq!(wrap_title("short", 10, 2), ["short"]);
    assert_eq!(wrap_title("aaa bbb ccc", 7, 2), ["aaa bbb", "ccc"]);
    assert_eq!(wrap_title("aaa bbb ccc ddd eee", 7, 2), ["aaa bbb", "ccc dd…"]);
    assert_eq!(wrap_title("abcdefghij", 4, 2), ["abcd", "efg…"]);
    assert!(wrap_title("   ", 5, 2).is_empty());
    let mut t = task("T-1", Kind::Task, "x", "backlog");
    assert_eq!(agent_level(&t), (String::new(), String::new()));
    t.effort = "high".into();
    assert_eq!(agent_level(&t), ("авто".to_string(), "high".to_string()));
    t.model = "sonnet".into();
    t.effort = "default".into();
    assert_eq!(agent_level(&t), ("sonnet".to_string(), String::new()));

    let mut app = make_app();
    let mut t7 = task("T-007", Kind::Task, "Переработать механизм авторизации пользователей и согласовать формат токенов с внешним сервисом", "backlog");
    t7.effort = "high".into();
    t7.order = 8;
    let mut t8 = task("T-008", Kind::Task, "Короткая", "in-progress");
    t8.model = "sonnet".into();
    t8.effort = "medium".into();
    t8.order = 9;
    app.tasks.push(t7);
    app.tasks.push(t8);
    let mut term = Terminal::new(TestBackend::new(200, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "card sections 200x35");
    assert!(s.contains("T-008  sonnet*medium"));
    assert!(s.contains("T-007  авто*high"));
    assert!(s.contains("Переработать механизм авторизации"));
    assert!(s.contains('…'));
    assert!(s.contains("▰▱▱▱ 1/3"));
    assert!(s.contains("⊘ T-001,T-004"));
    assert!(!s.contains('▼'));
    app.show_hidden = true;
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "card sections, hidden shown").contains('▼'));
    app.show_hidden = false;
    term.draw(|f| draw(f, &mut app)).unwrap();
    // cards are CARD_H rows tall: consecutive top borders in the first column
    let tops: Vec<usize> = s.lines().enumerate().filter(|(_, l)| l.starts_with('╭')).map(|(i, _)| i).collect();
    assert!(tops.len() >= 2);
    assert!(tops.windows(2).all(|w| w[1] - w[0] == CARD_H as usize));
    // an id-only header for a card with default model and effort
    assert!(s.lines().any(|l| l.starts_with("│▪ T-004") && !l.contains("авто")));
}

#[test]
fn multiline_scroll_and_trunc() {
    let mut app = make_app();
    let mut tb = TextBuf::new(&"word ".repeat(120), true);
    tb.insert_str("\nlast line");
    let form = Form::new(
        "Custom",
        crate::form::Purpose::Rules,
        vec![crate::form::Field { key: "body".into(), label: "Body".into(), kind: FieldKind::Text(tb), side: false, suggest: crate::form::Suggest::None }],
    );
    app.mode = Mode::Form(Box::new(form));
    let mut term = Terminal::new(TestBackend::new(60, 14)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "multiline scrolled");
    assert!(s.contains("last line"));
    assert_eq!(trunc("héllo wörld", 6), "héllo…");
    assert_eq!(trunc("abc", 3), "abc");
}

#[test]
fn at_picker_popup_render() {
    for (w, h) in [(120u16, 35u16), (80, 24)] {
        let mut app = make_app();
        let root = match &app.mode {
            Mode::Board => app.store.dir.parent().unwrap().to_path_buf(),
            _ => unreachable!(),
        };
        std::fs::create_dir_all(root.join("src/widgets")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        for f in ["src/ui.rs", "src/model.rs", "src/widgets/button.rs", "README.md", "docs/guide.md"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('n')));
        app.on_key(key(KeyCode::Tab));
        for c in "Look at @s".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("@ picker below the cursor {w}x{h}"));
        assert!(s.contains("src/ui.rs") && s.contains("src/model.rs") && s.contains("▸ "), "{w}x{h}");
        if w >= 100 {
            assert!(s.contains("@ — файл проекта"));
        }
        let popup_row = s.lines().position(|l| l.contains("src/ui.rs")).unwrap();
        let cursor_row = s.lines().position(|l| l.contains("Look at @s")).unwrap();
        assert!(popup_row > cursor_row, "{w}x{h}");

        // Down moves the highlight
        app.on_key(key(KeyCode::Down));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s2 = dump(&term, &format!("@ picker, second row selected {w}x{h}"));
        let sel_line = s2.lines().find(|l| l.contains("▸ ")).unwrap();
        assert!(sel_line.contains("src/ui.rs"), "{w}x{h}");

        // cursor on the last visible row of the field: the popup flips above it
        for c in "  ".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        for _ in 0..(h as usize) {
            app.on_key(key(KeyCode::Enter));
        }
        app.on_key(key(KeyCode::Char('@')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s3 = dump(&term, &format!("@ picker flipped above {w}x{h}"));
        let popup_row = s3.lines().position(|l| l.contains("README.md")).unwrap();
        let at_row = s3.lines().position(|l| l.contains("│@")).unwrap();
        if w >= 100 {
            assert!(popup_row < at_row, "{w}x{h}");
        } else {
            // single-column fallback: the description is not at the bottom, so the popup stays below the cursor
            assert!(popup_row > at_row, "{w}x{h}");
        }
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn card_picker_popup_render() {
    for (w, h) in [(120u16, 35u16), (80, 24), (60, 20)] {
        let mut app = make_app();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('n')));
        app.on_key(key(KeyCode::BackTab)); // last field: «Блокирует»
        app.on_key(key(KeyCode::BackTab)); // dependencies
        for c in "T-0".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("card picker {w}x{h}"));
        assert!(s.contains("▸ T-001") && s.contains("Login form"), "{w}x{h}");
        assert!(s.contains("T-004") && s.contains("Unicode"), "{w}x{h}");
        if w >= 100 {
            assert!(s.contains("Введите id или название — подсказка карточек"));
            assert!(s.contains("Login form  · В работе"), "{w}x{h}");
        }
        let field_row = s.lines().position(|l| l.contains("T-0") && l.contains('│') && !l.contains("▸")).unwrap();
        let popup_row = row_of(&s, "▸ T-001");
        // room below the field (two columns, or 80x24 where «Блокирует» follows it); at 60x20 the field is at the bottom, so the popup flips above
        assert_eq!(popup_row > field_row, w >= 80, "{w}x{h}");

        // Cyrillic title search, epics get a diamond
        for _ in 0..3 {
            app.on_key(key(KeyCode::Backspace));
        }
        for c in "ЗАГОЛОВ".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s2 = dump(&term, &format!("card picker, cyrillic query {w}x{h}"));
        assert!(s2.contains("▸ T-004  Unicode заголовок"), "{w}x{h}");
        assert!(!s2.contains("T-001  Login"), "{w}x{h}");
        for _ in 0..7 {
            app.on_key(key(KeyCode::Backspace));
        }
        for c in "authent".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s3 = dump(&term, &format!("card picker, epic {w}x{h}"));
        assert!(s3.contains("◆ E-001"), "{w}x{h}");
        let _ = std::fs::remove_dir_all(app.store.dir.parent().unwrap_or(&app.store.dir));
    }
}

#[test]
fn agent_picker_popup_render() {
    for (w, h) in [(120u16, 35u16), (80, 24)] {
        let mut app = make_app();
        app.home = None;
        let root = app.store.dir.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(root.join(".claude/agents")).unwrap();
        std::fs::create_dir_all(root.join(".codex/agents")).unwrap();
        std::fs::write(
            root.join(".claude/agents/pumba-worker.md"),
            "---\nname: pumba-worker\ndescription: \"Исполнитель карточки доски pumba. Вызывать как исполнителя колонки «В работе»\"\n---\n",
        )
        .unwrap();
        std::fs::write(root.join(".codex/agents/pumba-worker.toml"), "name = \"pumba_worker\"\ndescription = \"codex\"\n").unwrap();
        std::fs::write(root.join(".claude/agents/pumba-planner.md"), "---\ndescription: Планировщик\n---\n").unwrap();
        std::fs::write(root.join(".claude/agents/reviewer-x.md"), "---\ndescription: Мой ревьюер\n---\n").unwrap();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('c')));
        for _ in 0..4 {
            app.on_key(key(KeyCode::Tab));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("agent popup, empty field {w}x{h}"));
        assert!(s.contains("▸ pumba-planner") && s.contains("pumba-worker") && s.contains("reviewer-x"), "{w}x{h}");
        assert!(s.contains("· claude, codex"), "{w}x{h}");
        assert!(s.contains("Подсказка: ↑↓ Enter — выбрать субагента"), "{w}x{h}");
        let field_row = row_of(&s, "Исполнитель (пусто — нет)");
        let popup_row = row_of(&s, "▸ pumba-planner");
        assert!(popup_row > field_row, "{w}x{h}");
        assert!(s.lines().all(|l| l.chars().count() <= w as usize));

        // narrowing and moving the highlight
        for c in "wor".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s2 = dump(&term, &format!("agent popup, query «wor» {w}x{h}"));
        assert!(s2.contains("▸ pumba-worker") && !s2.contains("pumba-planner"), "{w}x{h}");
        app.on_key(key(KeyCode::Enter));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s3 = dump(&term, &format!("agent popup, after pick {w}x{h}"));
        assert!(!s3.contains("▸ "), "{w}x{h}");
        assert!(s3.contains("pumba-worker"), "{w}x{h}");
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn select_filter_popup_render() {
    for (w, h) in [(120u16, 35u16), (80, 24)] {
        let mut app = make_app();
        let titles = ["Авторизация", "Платежи", "Отчёты", "Миграция БД", "Поиск", "Уведомления", "Админка", "Экспорт", "Импорт", "Мобильный клиент", "Платёжный шлюз", "Документация"];
        for (i, t) in titles.iter().enumerate() {
            let mut e = task(&format!("E-{:03}", i + 2), Kind::Epic, t, "backlog");
            e.order = 20 + i as i64;
            app.tasks.push(e);
        }
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('n')));
        for _ in 0..5 {
            app.on_key(key(KeyCode::Tab));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s0 = dump(&term, &format!("epic select focused {w}x{h}"));
        assert!(s0.contains("Печатайте — фильтр списка"), "{w}x{h}");
        for c in "пла".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("epic filter popup {w}x{h}"));
        assert!(s.contains("▸ E-003 Платежи") && s.contains("E-012 Платёжный шлюз"), "{w}x{h}");
        assert!(s.contains(" пла▏ "), "{w}x{h}");
        assert!(!s.contains("E-002 Авторизация"), "{w}x{h}");
        let field_row = row_of(&s, "Эпик");
        assert!(row_of(&s, "▸ E-003 Платежи") > field_row, "{w}x{h}");
        assert!(s.lines().all(|l| l.chars().count() <= w as usize));
        // move the highlight, then a query without a match
        app.on_key(key(KeyCode::Down));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s2 = dump(&term, &format!("epic filter popup, second row {w}x{h}"));
        assert!(s2.contains("▸ E-012 Платёжный шлюз"), "{w}x{h}");
        for c in "яя".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s3 = dump(&term, &format!("epic filter popup, no match {w}x{h}"));
        assert!(s3.contains("нет совпадений") && s3.contains("плаяя▏"), "{w}x{h}");
        // pick: the value shows in the field, popup gone
        for _ in 0..2 {
            app.on_key(key(KeyCode::Backspace));
        }
        app.on_key(key(KeyCode::Enter));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s4 = dump(&term, &format!("epic filter popup, after pick {w}x{h}"));
        assert!(!s4.contains("▸ E-") && !s4.contains("нет совпадений"), "{w}x{h}");
        assert!(s4.contains("◀ E-003 Платежи ▶") || s4.contains("◀ E-003 Платеж"), "{w}x{h}");
        let _ = std::fs::remove_dir_all(app.store.dir.parent().unwrap_or(&app.store.dir));
    }
}

fn row_of(s: &str, pat: &str) -> usize {
    s.lines().position(|l| l.contains(pat)).unwrap_or_else(|| panic!("no row with {pat}"))
}

fn col_of(s: &str, row: usize, pat: &str) -> usize {
    let l = s.lines().nth(row).unwrap();
    l[..l.find(pat).unwrap()].chars().count()
}

#[test]
fn task_form_two_columns() {
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(120, 35)).unwrap();
    app.on_key(key(KeyCode::Char('n')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "two-column task form 120x35");
    // right column starts on the title row; the left column border is on the same row
    let r = row_of(&s, "Модель");
    assert_eq!(r, row_of(&s, "Название"));
    let l = s.lines().nth(r).unwrap();
    assert!(l.contains("┌") || l.contains("╭") || l.contains("┏"), "{l}");
    assert!(col_of(&s, r, "Название") < col_of(&s, r, "Модель"));
    let rows_after = |p: &str| s.lines().enumerate().skip(r).find(|(_, l)| l.contains(p)).unwrap().0;
    let (ry, ru, rs, re, rd, rb) = (r, rows_after("Уровень"), rows_after("Спринт"), rows_after("Эпик"), rows_after("Зависит от"), rows_after("Блокирует (id через запятую)"));
    assert!(ry < ru && ru < rs && rs < re && re < rd && rd < rb);
    let col = col_of(&s, r, "Модель");
    assert_eq!(col_of(&s, ru, "Уровень"), col);
    // description starts under the title and runs next to the side fields
    let body = row_of(&s, "Описание для агента");
    assert!(body > r && body <= r + 3);
    assert!(col_of(&s, body, "Описание") < col);
    assert!(s.contains("Список моделей: M на доске"));
    // select value sits right after the (narrow) label column
    let model_line = s.lines().nth(r).unwrap();
    assert!(model_line.contains("Модель   ◀ по умолчанию ▶"), "{model_line}");
    // cursor follows the focused field's real rect
    let cx = |t: &Terminal<TestBackend>| t.backend().cursor_position().x as usize;
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(cx(&term) < col - 2, "title cursor in left column");
    app.on_key(key(KeyCode::Tab));
    for c in "hello".chars() {
        app.on_key(key(KeyCode::Char(c)));
    }
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(cx(&term) < col - 2, "body cursor in left column");
    app.on_key(key(KeyCode::BackTab));
    app.on_key(key(KeyCode::BackTab)); // wraps to blocks (right column, single-line text)
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s2 = dump(&term, "depends focused 120x35");
    assert!(cx(&term) >= col - 2, "depends cursor in right column");
    assert!(s2.contains("hello"));
    // epic form has no parent select
    app.on_key(key(KeyCode::Esc));
    app.on_key(key(KeyCode::Char('N')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s3 = dump(&term, "epic form 120x35");
    assert!(s3.contains("Модель") && !s3.contains("Эпик "));
}

#[test]
fn task_form_narrow_falls_back() {
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    app.on_key(key(KeyCode::Char('n')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "single-column fallback 80x24");
    let t = row_of(&s, "Название");
    assert_ne!(row_of(&s, "Модель"), t);
    assert!(row_of(&s, "Описание для агента") > t);
    // side fields follow the others
    assert!(row_of(&s, "Модель") > row_of(&s, "Описание для агента"));
    assert_eq!(col_of(&s, row_of(&s, "Модель"), "Модель") > 0, true);
}

#[test]
fn task_form_order_and_wrap() {
    let mut app = make_app();
    app.on_key(key(KeyCode::Char('n')));
    let Mode::Form(f) = &mut app.mode else { panic!() };
    let keys: Vec<&str> = f.fields.iter().map(|x| x.key.as_str()).collect();
    assert_eq!(keys, ["title", "body", "model", "effort", "sprint", "parent", "depends", "blocks"]);
    assert!(f.fields.iter().all(|x| x.side == (x.key != "title" && x.key != "body")));
    f.handle(key(KeyCode::Up));
    assert_eq!(f.fields[f.focus].key, "blocks");
    f.handle(key(KeyCode::Down));
    assert_eq!(f.focus, 0);
    f.handle(key(KeyCode::BackTab));
    assert_eq!(f.focus, f.fields.len() - 1);
    f.handle(key(KeyCode::Enter));
    assert_eq!(f.focus, 0);
    assert_eq!(f.sel("model"), "default");
    assert_eq!(f.text("body"), "");
}

#[test]
fn rules_form_wraps() {
    let mut app = make_app();
    app.on_key(key(KeyCode::Char('P')));
    let Mode::Form(f) = &mut app.mode else { panic!() };
    let last = f.fields.len() - 1;
    f.handle(key(KeyCode::Up));
    assert_eq!(f.focus, last);
    assert_eq!(f.fields[last].key, "custom");
    f.handle(key(KeyCode::Down)); // multiline, empty: Down is not consumed
    assert_eq!(f.focus, 0);
}

#[test]
fn column_jump_keys() {
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(120, 35)).unwrap();
    app.on_key(key(KeyCode::Char('n')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    let ctrl = |c| KeyEvent::new(c, KeyModifiers::CONTROL);
    let alt = |c| KeyEvent::new(c, KeyModifiers::ALT);
    let foc = |app: &App| match &app.mode {
        Mode::Form(f) => (f.fields[f.focus].key.clone(), f.fields[f.focus].side),
        _ => panic!(),
    };
    // Ctrl+Right from the title lands in the right column, at the closest row (model)
    app.on_key(ctrl(KeyCode::Right));
    assert_eq!(foc(&app), ("model".to_string(), true));
    // Ctrl+Left from a select lands in the left column; nothing was inserted anywhere
    app.on_key(ctrl(KeyCode::Left));
    assert_eq!(foc(&app), ("title".to_string(), false));
    // Alt works too, from any field
    app.on_key(alt(KeyCode::Right));
    assert!(foc(&app).1);
    app.on_key(key(KeyCode::Down));
    app.on_key(key(KeyCode::Down));
    app.on_key(alt(KeyCode::Left));
    assert!(!foc(&app).1);
    // plain Right at the end of the title jumps; mid-text it only moves the cursor
    if let Mode::Form(f) = &mut app.mode {
        f.focus = 0;
    }
    assert_eq!(foc(&app).0, "title");
    for c in "abc".chars() {
        app.on_key(key(KeyCode::Char(c)));
    }
    app.on_key(key(KeyCode::Left));
    assert_eq!(foc(&app).0, "title");
    app.on_key(key(KeyCode::Right));
    assert_eq!(foc(&app).0, "title");
    app.on_key(key(KeyCode::Right)); // at the end now
    assert_eq!(foc(&app), ("model".to_string(), true));
    // plain Left on a select keeps cycling the value
    app.on_key(key(KeyCode::Left));
    let Mode::Form(f) = &app.mode else { panic!() };
    assert_eq!(f.fields[f.focus].key, "model");
    assert_ne!(f.sel("model"), "default"); // wrapped to the last model
    assert_eq!(f.text("title"), "abc");
    // Left at the very start of a text field jumps back from the right column's text field
    let Mode::Form(f) = &mut app.mode else { panic!() };
    f.focus = 6; // depends, empty: Left at 0 jumps to the left column
    app.on_key(key(KeyCode::Left));
    assert!(!foc(&app).1);
    // narrow terminal: nothing special, Ctrl+Right does not move focus
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    app.on_key(key(KeyCode::Char('n')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    app.on_key(ctrl(KeyCode::Right));
    assert_eq!(foc(&app), ("title".to_string(), false));
}

/// (x, y, width, height) of the popup whose top border row holds `title`.
fn popup_box(s: &str, title: &str) -> (usize, usize, usize, usize) {
    let y = row_of(s, title);
    let chars: Vec<char> = s.lines().nth(y).unwrap().chars().collect();
    let tchars: Vec<char> = format!("╭ {title}").chars().collect();
    let x = (0..chars.len()).find(|&i| chars[i..].starts_with(&tchars)).expect("top-left corner");
    let x2 = (x..chars.len()).find(|&i| chars[i] == '╮').expect("top-right corner");
    let at = |l: &str, i: usize| l.chars().nth(i);
    let h = s
        .lines()
        .skip(y)
        .position(|l| at(l, x) == Some('╰') && at(l, x2) == Some('╯'))
        .expect("bottom border")
        + 1;
    (x, y, x2 - x + 1, h)
}

#[test]
fn form_popup_fills_90_percent() {
    let mut app = make_app();
    for (w, h) in [(160u16, 50u16), (120, 35), (80, 24)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('n')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("new task form {w}x{h}"));
        let (x, y, pw, ph) = popup_box(&s, "Новая задача");
        assert_eq!(pw, (w as usize) * 9 / 10, "{w}x{h} width");
        assert_eq!(ph, (h as usize) * 9 / 10, "{w}x{h} height");
        // centered: equal margins (within one cell of rounding)
        assert!(x.abs_diff(w as usize - x - pw) <= 1 && y.abs_diff(h as usize - y - ph) <= 1, "{w}x{h} centered");
        if w >= 120 {
            // right column is at most 44 wide: the description gets the rest
            let r = row_of(&s, "Модель");
            let side = col_of(&s, r, "Модель");
            let right_edge = x + pw - 2;
            assert!(right_edge - side <= 44, "{w}x{h}: right column {}", right_edge - side);
            if w == 160 {
                assert!(right_edge - side >= 40, "right column not clamped narrower than 44");
            }
        }
        app.on_key(key(KeyCode::Esc));
        // the other forms use the same width rule
        for c in ['P', 'M', 'c'] {
            app.on_key(key(KeyCode::Char(c)));
            term.draw(|f| draw(f, &mut app)).unwrap();
            dump(&term, &format!("form {c} {w}x{h}"));
            app.on_key(key(KeyCode::Esc));
        }
    }
}

#[test]
fn trash_view_render() {
    let mut app = make_app();
    // empty trash
    app.on_key(key(KeyCode::Char('B')));
    let mut term = Terminal::new(TestBackend::new(120, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "trash empty 120x35");
    assert!(s.contains("Корзина (0)") && s.contains("Корзина пуста") && s.contains("UTC"));
    app.on_key(key(KeyCode::Esc));
    assert!(!dump(&term, "board no chip").contains("корзина:"));

    for id in ["T-004", "T-001", "T-003"] {
        let ti = app.tasks.iter().position(|t| t.id == id).unwrap();
        app.sel_col = app.effective_col(&app.tasks[ti]);
        app.sel_idx = app.col_task_idx(app.sel_col).iter().position(|&i| i == ti).unwrap();
        app.on_key(key(KeyCode::Char('d')));
        app.on_key(key(KeyCode::Char('y')));
    }
    let at = |app: &mut App, id: &str, v: u64| app.tasks.iter_mut().find(|t| t.id == id).unwrap().deleted_at = Some(v);
    at(&mut app, "T-004", 1_793_479_260); // 31.10 20:41
    at(&mut app, "T-001", 1_793_479_260 + 3_600);
    at(&mut app, "T-003", 1_793_479_260 - 86_400);
    for (w, h) in [(120u16, 35u16), (80, 24), (40, 12)] {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("board with trash chip {w}x{h}"));
        if w >= 80 {
            assert!(s.contains("корзина: 3"), "{w}x{h}");
        }
        app.on_key(key(KeyCode::Char('B')));
        app.on_key(key(KeyCode::Down));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("trash view {w}x{h}"));
        assert!(s.contains("Корзина (3)"), "{w}x{h}");
        if w >= 80 {
            // newest first: T-001 (21:41), T-004 (20:41), T-003 (previous day)
            let (a, b, c) = (row_of(&s, "T-001"), row_of(&s, "T-004"), row_of(&s, "T-003"));
            assert!(a < b && b < c, "{w}x{h}");
            assert!(s.contains("удалена: 31.10 21:41") && s.contains("удалена: 31.10 20:41") && s.contains("удалена: 30.10 20:41"));
            assert!(s.lines().nth(b).unwrap().contains('▶'), "selection on the second row");
            assert!(s.contains("· Бэклог"));
        }
        app.on_key(key(KeyCode::Esc));
    }
}

#[test]
fn blocks_marker_on_cards() {
    let mut app = make_app();
    let mut term = Terminal::new(TestBackend::new(200, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "blocks marker 200x35");
    // T-003 (done) is waited on by T-005 and T-006 (both in the backlog); T-001/T-004 by T-005 only
    assert!(s.contains("→ T-005,T-006"));
    assert!(s.matches("→ T-005").count() >= 3);
    // T-005 itself blocks nothing: no marker on its row group, `⊘` stays
    assert!(s.contains("⊘ T-001,T-004"));
    // a dependent in the last column is not waiting any more
    app.tasks.iter_mut().find(|t| t.id == "T-005").unwrap().column = "done".into();
    app.tasks.iter_mut().find(|t| t.id == "T-006").unwrap().column = "done".into();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "blocks marker, dependents done");
    assert!(!s.contains("→ T-00"), "no marker once the dependents are done");

    // many dependents: the ids do not fit, the count is shown instead
    for i in 0..8 {
        let mut t = task(&format!("T-1{i:02}"), Kind::Task, "Waits", "backlog");
        t.depends = vec!["T-001".into()];
        t.order = 20 + i;
        app.tasks.push(t);
    }
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "blocks marker, count form");
    assert!(s.contains("→ 8"), "8 waiting dependents of T-001 (T-005 is done)");
    assert!(!s.contains("→ T-100"));

    // placement: the marker sits before `скрыта`; epics get it too
    let mut app = make_app();
    app.show_hidden = true;
    let ti = app.tasks.iter().position(|t| t.id == "T-003").unwrap();
    app.tasks[ti].hidden = true;
    app.tasks[ti].sprint = Some(1);
    app.tasks[ti].parent = Some("E-001".into());
    for w in [200u16, 80] {
        let mut term = Terminal::new(TestBackend::new(w, 35)).unwrap();
        app.sel_col = 4;
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("marker and hidden {w}"));
        assert!(s.contains("→ T-005,T-006  скрыта"), "{w}: marker sits before `скрыта`");
    }
    // narrow column: the ids no longer fit and shrink to a count before anything is dropped
    let mut term = Terminal::new(TestBackend::new(60, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "marker count, 60 wide");
    assert!(s.contains("→ 2") && !s.contains("→ T-005,T-006"));
    app.tasks.iter_mut().find(|t| t.id == "E-001").unwrap().column = "backlog".into();
    app.tasks.push({
        let mut t = task("T-200", Kind::Task, "After epic", "backlog");
        t.depends = vec!["E-001".into()];
        t.order = 30;
        t
    });
    let mut term = Terminal::new(TestBackend::new(200, 35)).unwrap();
    app.sel_col = 0;
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "epic with dependent").contains("→ T-200"));
}

#[test]
fn detail_pane_lists_blocked_cards() {
    let mut app = make_app();
    let select = |app: &mut App, id: &str| {
        let ti = app.tasks.iter().position(|t| t.id == id).unwrap();
        app.sel_col = app.effective_col(&app.tasks[ti]);
        app.sel_idx = app.col_task_idx(app.sel_col).iter().position(|&i| i == ti).unwrap();
    };
    let mut term = Terminal::new(TestBackend::new(230, 40)).unwrap();
    select(&mut app, "T-001");
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "detail T-001");
    assert!(s.contains("блокирует: T-005 (Бэклог)"), "{s}");
    assert!(!s.contains("зависит:"));
    select(&mut app, "T-003");
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "detail T-003");
    assert!(s.contains("блокирует: T-005 (Бэклог), T-006 (Бэклог)"), "{s}");
    // depends and blocks both shown; a trashed dependent is not listed
    let mut t9 = task("T-009", Kind::Task, "Gone", "backlog");
    t9.depends = vec!["T-004".into()];
    t9.deleted = true;
    app.tasks.push(t9);
    app.tasks.iter_mut().find(|t| t.id == "T-004").unwrap().depends = vec!["T-001".into()];
    select(&mut app, "T-004");
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "detail T-004 (both)");
    assert!(s.contains("зависит: T-001 (В работе)") && s.contains("блокирует: T-005 (Бэклог)"), "{s}");
    assert!(!s.contains("T-009"));
    // a card nothing waits on has no such part
    select(&mut app, "T-005");
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(!dump(&term, "detail T-005").contains("блокирует:"));
    // help has the new line
    app.mode = Mode::Help;
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "help");
    assert!(s.contains("блокирует") && s.contains("поле «Блокирует» в форме правки"));
    assert!(s.contains("ЙЦУКЕН"), "help still fits: last left row visible");
    assert!(s.contains("Бэклог и Готово закреплены"), "{s}");
}

#[test]
fn blocks_field_in_form_render() {
    for (w, h) in [(160u16, 50u16), (120, 35), (80, 24)] {
        let mut app = make_app();
        let ti = app.tasks.iter().position(|t| t.id == "T-001").unwrap();
        app.sel_col = app.effective_col(&app.tasks[ti]);
        app.sel_idx = app.col_task_idx(app.sel_col).iter().position(|&i| i == ti).unwrap();
        app.on_key(key(KeyCode::Char('e')));
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("edit T-001, blocks prefilled {w}x{h}"));
        let (rd, rb) = (row_of(&s, "Зависит от"), row_of(&s, "Блокирует (id через запятую)"));
        assert!(rb > rd, "{w}x{h}");
        // blocks (T-005 waits on T-001) is prefilled on the row under its title
        assert!(s.lines().nth(rb + 1).unwrap().contains("T-005"), "{w}x{h}");
        if w >= 100 {
            // same column as depends, directly below it
            assert_eq!(col_of(&s, rd, "Зависит от"), col_of(&s, rb, "Блокирует"), "{w}x{h}");
            assert_eq!(rb - rd, 3, "{w}x{h}");
            // and still next to the left column, fully inside the popup
            assert!(col_of(&s, rb, "Блокирует") > col_of(&s, row_of(&s, "Описание для агента"), "Описание"));
        }
        // focus the blocks field and type: popup excludes the card itself, T-005 (listed) and the depends ids
        app.on_key(key(KeyCode::BackTab));
        for c in ", T-00".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("blocks popup {w}x{h}"));
        assert!(s.contains("▸ T-002") || s.contains("▸ T-003") || s.contains("▸ T-004"), "{w}x{h}");
        assert!(!s.contains("▸ T-001") && !s.contains("▸ T-005"), "{w}x{h}");
    }
}

#[test]
fn models_form_with_defaults_render() {
    for (w, h) in [(160u16, 50u16), (120, 35), (80, 24)] {
        let mut app = make_app();
        app.cfg.default_model = "sonnet".into();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        app.on_key(key(KeyCode::Char('M')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("models form with defaults {w}x{h}"));
        assert!(s.contains("Модель по умолчанию для новых карточек"), "{w}x{h}");
        assert!(s.contains("Уровень по умолчанию для новых карточек"), "{w}x{h}");
        assert!(s.contains("Модели (по одной в строке)") && s.contains("Уровни размышления"), "{w}x{h}");
        // selects first, one line each; the value follows the label on the same row
        let (rm, re) = (row_of(&s, "Модель по умолчанию"), row_of(&s, "Уровень по умолчанию"));
        assert_eq!(re, rm + 1, "{w}x{h}");
        assert!(s.lines().nth(rm).unwrap().contains("◀ sonnet ▶"), "{w}x{h}");
        assert!(s.lines().nth(re).unwrap().contains("◀ по умолчанию ▶"), "{w}x{h}");
        assert!(row_of(&s, "Модели (по одной") > re, "{w}x{h}");
        // the selection cycles through the lists as typed in the text fields at open time
        app.on_key(key(KeyCode::Right));
        term.draw(|f| draw(f, &mut app)).unwrap();
        assert!(dump(&term, "models form, model cycled").lines().nth(rm).unwrap().contains("◀ haiku ▶"));
    }
}

/// Foreground color of the first cell of `pat` on any row of the drawn buffer.
fn fg_of(term: &Terminal<TestBackend>, pat: &str) -> Option<Color> {
    let buf = term.backend().buffer();
    let first = pat.chars().next().unwrap().to_string();
    for y in 0..buf.area.height {
        let row: Vec<String> = (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
        let line = row.concat();
        if let Some(byte) = line.find(pat) {
            let col = line[..byte].chars().count();
            if row[col] == first {
                return Some(buf[(col as u16, y)].fg);
            }
        }
    }
    None
}

#[test]
fn unknown_column_is_flagged_on_card_and_detail() {
    let mut app = make_app();
    let mut t = task("T-050", Kind::Task, "Lost card", "gone");
    t.order = -1; // first card of the first column
    app.tasks.push(t);
    assert_eq!(app.effective_col(app.tasks.last().unwrap()), 0);
    app.sel_col = 0;
    app.sel_idx = 0;
    assert_eq!(app.tasks[app.selected_idx().unwrap()].id, "T-050");
    let mut term = Terminal::new(TestBackend::new(200, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "unknown column 200x35");
    // card line 4 flag, red
    assert!(s.contains("?колонка"));
    assert_eq!(fg_of(&term, "?колонка"), Some(RED));
    // detail pane meta, red
    assert!(s.contains("колонка: gone (нет такой колонки)"));
    assert_eq!(fg_of(&term, "gone (нет такой колонки)"), Some(RED));
    // cards in known columns carry neither
    let mut ok = make_app();
    let mut term2 = Terminal::new(TestBackend::new(200, 35)).unwrap();
    term2.draw(|f| draw(f, &mut ok)).unwrap();
    let s2 = dump(&term2, "known columns 200x35");
    assert!(!s2.contains("?колонка") && !s2.contains("нет такой колонки"));
    // an empty `column:` is flagged too
    app.tasks.last_mut().unwrap().column = String::new();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "empty column").contains("колонка: <пусто> (нет такой колонки)"));

    // narrow: `скрыта` and the parent go first, the flag stays (it sits before `скрыта`)
    let last = app.tasks.last_mut().unwrap();
    last.column = "gone".into();
    last.hidden = true;
    last.sprint = Some(1);
    last.parent = Some("E-001".into());
    app.show_hidden = true;
    let mut term = Terminal::new(TestBackend::new(30, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "unknown column, narrow");
    let row = s.lines().find(|l| l.contains("?колонка")).expect("flag survives");
    assert!(!row.contains("скрыта"), "{row}");
    let mut term = Terminal::new(TestBackend::new(200, 35)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert!(dump(&term, "unknown column, hidden").contains("?колонка  скрыта"));
}

#[test]
fn unreadable_cards_stay_in_the_footer() {
    let mut app = make_app();
    std::fs::write(app.store.dir.join("tasks/T-007.md"), "broken").unwrap();
    app.reload();
    app.status = None; // the transient status expired; the warning stays while the files are broken
    let mut term = Terminal::new(TestBackend::new(100, 24)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "footer unreadable");
    assert!(s.contains("Не прочитано: T-007.md"));
    assert_eq!(fg_of(&term, "Не прочитано"), Some(RED));
    assert!(!s.contains("h/l кол."));
    std::fs::remove_file(app.store.dir.join("tasks/T-007.md")).unwrap();
    app.reload();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "footer after fix");
    assert!(!s.contains("Не прочитано") && s.contains("h/l кол."));
}

#[test]
fn column_form_fits_80x24() {
    let mut app = make_app();
    for (open, size) in [('c', (80u16, 24u16)), ('r', (80, 24)), ('r', (120, 35)), ('c', (40, 12))] {
        app.on_key(key(KeyCode::Char('l'))); // select in-progress for the edit form
        app.sel_col = 1;
        app.on_key(key(KeyCode::Char(open)));
        let mut term = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let s = dump(&term, &format!("column form {open} {}x{}", size.0, size.1));
        if size.0 >= 80 {
            let labels = [
                "Название",
                "Перед работой (проверки, стоп)",
                "Работа (что делает исполнитель)",
                "После работы (проверка, куда)",
                "Исполнитель (пусто — нет)",
                "После успеха",
                "Цвет",
            ];
            for label in labels {
                assert!(s.contains(label), "{label} at {size:?}");
            }
            // two columns: the name and the side fields start on the same row
            let r = row_of(&s, "Название");
            assert_eq!(row_of(&s, "Исполнитель (пусто"), r, "{size:?}");
            let side = col_of(&s, r, "Исполнитель (пусто");
            assert!(col_of(&s, r, "Название") < side);
            let (sn, sc) = (row_of(&s, "После успеха"), row_of(&s, "Цвет"));
            assert!(r < sn && sn < sc && col_of(&s, sn, "После успеха") >= side - 2 && col_of(&s, sc, "Цвет") >= side - 2);
            // the three parts stacked in reading order in the left column, each with 3+ text rows
            let (b, w, a) = (row_of(&s, "Перед работой ("), row_of(&s, "Работа (что"), row_of(&s, "После работы ("));
            assert!(b > r && w > b && a > w, "{size:?}");
            assert!(col_of(&s, w, "Работа (что") < side && col_of(&s, a, "После работы (") < side);
            let title = if open == 'c' { "Новая колонка".to_string() } else { "Правка колонки В работе".to_string() };
            let (_, py, _, ph) = popup_box(&s, &title);
            let body_bottom = py + ph - 4; // two footer rows and the popup border below
            let rows = [w - b - 2, a - w - 2, body_bottom - a - 1];
            assert!(rows.iter().all(|&n| n >= 3), "text rows {rows:?} at {size:?}");
            if size.1 >= 35 {
                // the spare height is shared evenly
                assert!(rows.iter().max().unwrap() - rows.iter().min().unwrap() <= 1, "{rows:?}");
                assert!(rows[0] >= 5, "{rows:?}");
            }
        }
        if open == 'r' && size.0 >= 80 {
            assert!(s.contains("pumba-worker") && s.contains("На ревью"), "prefilled agent and next");
            assert!(s.contains("Эта задача В РАБОТЕ") && s.contains("1) `pumba-planner`") && s.contains("Успех: исполнитель"), "prefilled parts");
        }
        app.on_key(key(KeyCode::Esc));
    }
}

#[test]
fn column_form_focus_order_and_column_jumps() {
    let mut app = make_app();
    let ctrl = |c| KeyEvent::new(c, KeyModifiers::CONTROL);
    let foc = |app: &App| match &app.mode {
        Mode::Form(f) => (f.fields[f.focus].key.clone(), f.fields[f.focus].side),
        _ => panic!(),
    };
    for size in [(120u16, 35u16), (80, 24)] {
        let mut term = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        app.on_key(key(KeyCode::Char('c')));
        term.draw(|f| draw(f, &mut app)).unwrap();
        // Tab walks name → before → rule → after → agent → next → color and wraps
        let mut order = vec![foc(&app).0];
        for _ in 0..7 {
            app.on_key(key(KeyCode::Tab));
            order.push(foc(&app).0);
        }
        assert_eq!(order, ["name", "before", "rule", "after", "agent", "next", "color", "name"]);
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(foc(&app).0, "color");
        app.on_key(key(KeyCode::Tab));
        // two columns at both sizes: Ctrl+Right from the name lands on the agent, Ctrl+Left goes back
        term.draw(|f| draw(f, &mut app)).unwrap();
        app.on_key(ctrl(KeyCode::Right));
        assert_eq!(foc(&app), ("agent".to_string(), true), "{size:?}");
        app.on_key(ctrl(KeyCode::Left));
        assert_eq!(foc(&app), ("name".to_string(), false), "{size:?}");
        // from the lowest side field the jump lands on a rule part, not on the name
        if let Mode::Form(f) = &mut app.mode {
            f.focus = 6;
        }
        app.on_key(ctrl(KeyCode::Left));
        assert!(!foc(&app).1 && foc(&app).0 != "name", "{size:?}: {:?}", foc(&app));
        app.on_key(key(KeyCode::Esc));
    }
    // very small terminal: one column, Ctrl+Right does not move focus; nothing panics
    let mut term = Terminal::new(TestBackend::new(40, 12)).unwrap();
    app.on_key(key(KeyCode::Char('c')));
    term.draw(|f| draw(f, &mut app)).unwrap();
    dump(&term, "column form single column 40x12");
    app.on_key(ctrl(KeyCode::Right));
    assert_eq!(foc(&app), ("name".to_string(), false));
}

#[test]
fn rule_panel_shows_parts_in_order() {
    let mut app = make_app();
    app.sel_col = 1; // in-progress: the guard comes first
    let mut term = Terminal::new(TestBackend::new(160, 50)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "rule panel 160x50");
    assert!(row_of(&s, "Перед работой: Эта задача В РАБОТЕ") < row_of(&s, "Работа: 1)"));
    // short parts: all three in order; an empty part is left out
    let c = &mut app.cfg.columns[1];
    (c.before, c.rule, c.after) = ("check first".into(), "do it\nline two".into(), "verify".into());
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "rule panel short parts 160x50");
    let (b, w, w2, a) = (row_of(&s, "Перед работой: check first"), row_of(&s, "Работа: do it"), row_of(&s, "line two"), row_of(&s, "После работы: verify"));
    assert!(b < w && w < w2 && w2 < a, "{b} {w} {w2} {a}");
    app.cfg.columns[1].rule.clear();
    term.draw(|f| draw(f, &mut app)).unwrap();
    let s = dump(&term, "rule panel without work 160x50");
    assert!(s.contains("Перед работой: check first") && s.contains("После работы: verify") && !s.contains("Работа: "));
}
