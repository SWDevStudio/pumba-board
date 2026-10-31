//! Screen state of the TUI on top of `pumba_core::board::Board`: mode, selection, scrolls,
//! statuses, key handling. Board data and operations live in the core (reached through `Deref`);
//! forms, column screens and the tool picker are in the submodules.
use crate::files::FileIndex;
use crate::form::Form;
use pumba_core::board::{Board, View};
use pumba_core::model::Kind;
use pumba_install::agents;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

mod columns;
mod forms;
mod tools;
#[cfg(test)]
mod tests;

pub use tools::ToolPick;

pub enum Action {
    Init,
    /// Soft delete: move the card to the trash (the file stays on disk).
    TrashTask(String),
    DeleteColumn(String),
    NextSprint,
}

pub enum Mode {
    Board,
    Form(Box<Form>),
    Confirm(String, Action),
    Help,
    /// Trash view; the value is the selected row (newest deleted first).
    Trash(usize),
    /// Tool picker for the agent pack (after the init question, or `I` on an existing board).
    InitTools(ToolPick),
}

/// Screen state on top of the board. Board data and queries (`app.cfg`, `app.tasks`,
/// `app.blockers(..)`, ...) are reached through `Deref` to `Board`.
pub struct App {
    pub board: Board,
    pub project: String,
    pub sel_col: usize,
    pub sel_idx: usize,
    pub col_scroll: usize,
    pub scrolls: Vec<usize>,
    pub show_hidden: bool,
    pub sprint_only: bool,
    pub mode: Mode,
    pub status: Option<(String, bool, Instant)>,
    /// The status is a warning (orange, stays longer), not an error: set by `warn`, cleared by `info`/`err`.
    pub status_warn: bool,
    pub quit: bool,
    /// Project file index for the `@` picker: built on the first `@` after a form opened.
    files: Option<Rc<FileIndex>>,
    /// User home whose `.claude/agents` is listed read-only by the executor suggestions (None: skip).
    pub home: Option<PathBuf>,
}

impl Deref for App {
    type Target = Board;
    fn deref(&self) -> &Board {
        &self.board
    }
}

impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Board {
        &mut self.board
    }
}

impl App {
    pub fn new(root: PathBuf, exists: bool) -> App {
        let project = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());
        let mut app = App {
            board: Board::new(&root),
            project,
            sel_col: 0,
            sel_idx: 0,
            col_scroll: 0,
            scrolls: vec![0; 3],
            show_hidden: false,
            sprint_only: false,
            mode: Mode::Board,
            status: None,
            status_warn: false,
            quit: false,
            files: None,
            home: agents::home_dir(),
        };
        if exists {
            app.reload();
        } else {
            app.mode = Mode::Confirm(app.init_msg(), Action::Init);
        }
        app
    }

    fn init_msg(&self) -> String {
        format!("Папка .pumba/ не найдена. Создать её в {}?", self.root().display())
    }

    // ------------------------------------------------------------ status

    fn info(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), false, Instant::now()));
        self.status_warn = false;
    }

    fn err(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), true, Instant::now()));
        self.status_warn = false;
    }

    /// Success with a caveat (e.g. duplicate packs): not red, shown longer than a plain info.
    fn warn(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), false, Instant::now()));
        self.status_warn = true;
    }

    pub fn tick(&mut self) {
        let ttl = if self.status_warn { 12 } else { 4 };
        if self.status.as_ref().is_some_and(|s| s.2.elapsed() > Duration::from_secs(ttl)) {
            self.status = None;
            self.status_warn = false;
        }
        if matches!(self.mode, Mode::Board) && self.changed_on_disk() {
            self.reload();
        }
    }

    // ----------------------------------------------------------- queries

    /// The board filters toggled on screen.
    pub fn view(&self) -> View {
        View { show_hidden: self.show_hidden, sprint_only: self.sprint_only }
    }

    /// Visible cards of column `ci` under the screen filters (see `Board::column_tasks`).
    pub fn col_task_idx(&self, ci: usize) -> Vec<usize> {
        self.board.column_tasks(ci, self.view())
    }

    pub fn selected_idx(&self) -> Option<usize> {
        self.col_task_idx(self.sel_col).get(self.sel_idx).copied()
    }

    fn clamp(&mut self) {
        self.sel_col = self.sel_col.min(self.cfg.columns.len().saturating_sub(1));
        let n = self.col_task_idx(self.sel_col).len();
        self.sel_idx = self.sel_idx.min(n.saturating_sub(1));
    }

    fn select_task(&mut self, id: &str) {
        if let Some(ti) = self.tasks.iter().position(|t| t.id == id) {
            let ci = self.effective_col(&self.tasks[ti]);
            if let Some(p) = self.col_task_idx(ci).iter().position(|&i| i == ti) {
                self.sel_col = ci;
                self.sel_idx = p;
            }
        }
        self.clamp();
    }

    fn selected_id(&self) -> Option<String> {
        self.selected_idx().map(|i| self.tasks[i].id.clone())
    }

    // ------------------------------------------------------- persistence

    /// Reload from disk (see `Board::reload`), keeping the selected card; problems go to the status.
    pub fn reload(&mut self) {
        let keep = self.selected_id();
        match self.board.reload() {
            Ok(warn) => {
                if let Some(e) = warn {
                    self.err(e);
                }
                match self.unreadable_note() {
                    Some(note) => self.err(note),
                    // the warning is gone with the files: do not leave it on screen
                    None if self.status.as_ref().is_some_and(|s| s.0.starts_with("Не прочитано")) => self.status = None,
                    None => {}
                }
            }
            Err(e) => self.err(e),
        }
        self.scrolls.resize(self.cfg.columns.len(), 0);
        self.clamp();
        if let Some(id) = keep {
            self.select_task(&id);
        }
    }

    /// `Board::write`, then a screen reload (keeping the selection) when the board was changed
    /// on disk behind our back.
    fn commit(&mut self, save: &[String], cfg: bool) -> Result<(), String> {
        if self.board.write(save, cfg)? {
            self.reload();
        }
        Ok(())
    }

    /// Commit for board actions: on failure resync from disk and show the error.
    fn save(&mut self, save: &[String], cfg: bool) -> bool {
        match self.commit(save, cfg) {
            Ok(()) => true,
            Err(e) => {
                if self.ok {
                    self.reload();
                }
                self.err(e);
                false
            }
        }
    }

    // -------------------------------------------------------------- input

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        match std::mem::replace(&mut self.mode, Mode::Board) {
            Mode::Board => self.board_key(key),
            Mode::Form(f) => self.form_key(f, key),
            Mode::Confirm(m, a) => self.confirm_key(m, a, key),
            Mode::Help => {}
            Mode::Trash(sel) => self.trash_key(sel, key),
            Mode::InitTools(p) => self.tools_key(p, key),
        }
    }

    pub fn on_paste(&mut self, s: String) {
        if let Mode::Form(mut f) = std::mem::replace(&mut self.mode, Mode::Board) {
            f.paste(&s);
            self.attach_index(&mut f);
            self.mode = Mode::Form(f);
        }
    }

    fn step_col(&mut self, d: isize) {
        let max = self.cfg.columns.len() as isize - 1;
        self.sel_col = (self.sel_col as isize + d).clamp(0, max.max(0)) as usize;
    }

    fn step_card(&mut self, d: isize) {
        self.sel_idx = (self.sel_idx as isize + d).max(0) as usize;
    }

    fn board_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(latin_key(c)),
            other => other,
        };
        match code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Left if ctrl || alt => self.move_column(-1),
            KeyCode::Right if ctrl || alt => self.move_column(1),
            _ if ctrl => {}
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('H') => self.move_card(-1),
            KeyCode::Char('L') => self.move_card(1),
            KeyCode::Left if shift => self.move_card(-1),
            KeyCode::Right if shift => self.move_card(1),
            KeyCode::Char('h') | KeyCode::Left => self.step_col(-1),
            KeyCode::Char('l') | KeyCode::Right => self.step_col(1),
            KeyCode::Char('j') | KeyCode::Down => self.step_card(1),
            KeyCode::Char('k') | KeyCode::Up => self.step_card(-1),
            KeyCode::Char('J') => self.reorder(1),
            KeyCode::Char('K') => self.reorder(-1),
            KeyCode::Char('n') => self.open_task_form(Kind::Task, None),
            KeyCode::Char('N') => self.open_task_form(Kind::Epic, None),
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some(i) = self.selected_idx() {
                    self.open_task_form(self.tasks[i].kind, Some(i));
                }
            }
            KeyCode::Char('d') => {
                if let Some(i) = self.selected_idx() {
                    let t = &self.tasks[i];
                    let msg = format!(
                        "Переместить «{}» в корзину? Файл останется в .pumba/tasks/, восстановить можно в корзине (B).",
                        t.title
                    );
                    self.mode = Mode::Confirm(msg, Action::TrashTask(t.id.clone()));
                }
            }
            KeyCode::Char('x') => self.toggle_hidden(),
            KeyCode::Char('v') => {
                self.show_hidden = !self.show_hidden;
                self.info(if self.show_hidden { "Скрытые карточки показаны" } else { "Скрытые карточки спрятаны" });
            }
            KeyCode::Char('s') => {
                self.sprint_only = !self.sprint_only;
                let s = self.cfg.sprint;
                self.info(if self.sprint_only { format!("Только спринт {s}") } else { "Все спринты".into() });
            }
            KeyCode::Char('p') => self.toggle_sprint(),
            KeyCode::Char(']') => {
                let (s, n) = (self.cfg.sprint, self.cfg.sprint + 1);
                let msg = format!("Начать спринт {n}? Незавершённые карточки спринта {s} перейдут в него.");
                self.mode = Mode::Confirm(msg, Action::NextSprint);
            }
            KeyCode::Char('c') => self.open_column_form(None),
            KeyCode::Char('r') => self.open_column_form(Some(self.sel_col)),
            KeyCode::Char('X') => {
                if self.cfg.columns.len() <= 1 {
                    self.err("Нельзя удалить единственную колонку");
                } else if self.is_pinned(self.sel_col) {
                    let name = self.cfg.columns[self.sel_col].name.clone();
                    self.err(format!("Колонку «{name}» нельзя удалить: первая и последняя закреплены"));
                } else {
                    let c = &self.cfg.columns[self.sel_col];
                    let to = self.cfg.columns.iter().find(|o| o.id != c.id).map(|o| o.name.clone()).unwrap_or_default();
                    let msg = format!("Удалить колонку \"{}\"? Её карточки переедут в \"{to}\".", c.name);
                    self.mode = Mode::Confirm(msg, Action::DeleteColumn(c.id.clone()));
                }
            }
            KeyCode::Char('<') => self.move_column(-1),
            KeyCode::Char('>') => self.move_column(1),
            KeyCode::Char('P') => self.open_rules_form(),
            KeyCode::Char('M') => self.open_models_form(),
            KeyCode::Char('B') => self.mode = Mode::Trash(0),
            KeyCode::Char('I') => self.mode = Mode::InitTools(self.tool_pick(false)),
            KeyCode::Char('?') => self.mode = Mode::Help,
            _ => {}
        }
        self.clamp();
    }

    fn confirm_key(&mut self, msg: String, act: Action, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.run_action(act),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.cancel_action(&act),
            KeyCode::Char('c') if ctrl => self.cancel_action(&act),
            _ => self.mode = Mode::Confirm(msg, act),
        }
    }

    fn cancel_action(&mut self, act: &Action) {
        if matches!(act, Action::Init) {
            self.quit = true;
        }
    }

    fn run_action(&mut self, act: Action) {
        match act {
            // the board is created after the tool choice (see `tools_key`)
            Action::Init => self.mode = Mode::InitTools(self.tool_pick(true)),
            Action::TrashTask(id) => {
                if !self.board.trash_card(&id) {
                    return;
                }
                if self.save(&[id.clone()], false) {
                    self.info(format!("В корзине: {id}. Восстановить: B"));
                }
                self.clamp();
            }
            Action::DeleteColumn(id) => self.delete_column(&id),
            Action::NextSprint => {
                let new = self.cfg.sprint + 1;
                let moved = self.board.next_sprint();
                if self.save(&moved, true) {
                    self.info(format!("Спринт {new} начат"));
                }
            }
        }
    }

    // ------------------------------------------------------------- trash

    fn trash_key(&mut self, sel: usize, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(latin_key(c)),
            other => other,
        };
        let n = self.trashed().len();
        let sel = sel.min(n.saturating_sub(1));
        match code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            _ if ctrl => self.mode = Mode::Trash(sel),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('B') => {}
            KeyCode::Down | KeyCode::Char('j') => self.mode = Mode::Trash((sel + 1).min(n.saturating_sub(1))),
            KeyCode::Up | KeyCode::Char('k') => self.mode = Mode::Trash(sel.saturating_sub(1)),
            KeyCode::Enter | KeyCode::Char('r') => {
                self.mode = Mode::Trash(sel);
                self.restore(sel);
            }
            _ => self.mode = Mode::Trash(sel),
        }
    }

    /// Take the `sel`-th trashed card out of the trash; the view stays open with the selection in range.
    fn restore(&mut self, sel: usize) {
        let Some(&ti) = self.trashed().get(sel) else { return };
        let id = self.tasks[ti].id.clone();
        self.board.restore_card(&id);
        if self.save(&[id.clone()], false) {
            self.info(format!("Восстановлена: {id}"));
            self.select_task(&id);
        }
        let n = self.trashed().len();
        self.mode = Mode::Trash(sel.min(n.saturating_sub(1)));
    }

    // ------------------------------------------------------ card actions

    fn move_card(&mut self, d: isize) {
        let (Some(ti), nc) = (self.selected_idx(), self.sel_col as isize + d) else { return };
        if nc < 0 || nc as usize >= self.cfg.columns.len() {
            return;
        }
        let nc = nc as usize;
        let id = self.tasks[ti].id.clone();
        self.board.move_card_to(&id, nc);
        if self.save(&[id.clone()], false) {
            self.select_task(&id);
            // after a save that resynced with the disk the indices may have moved: look the card up by id
            let names: Vec<String> = match self.tasks.iter().find(|t| t.id == id) {
                Some(t) if nc != 0 => self.blockers(t).iter().map(|b| b.id.clone()).collect(),
                _ => Vec::new(),
            };
            if !names.is_empty() {
                self.err(format!("Заблокирована: {}", names.join(", ")));
            }
        }
    }

    fn reorder(&mut self, d: isize) {
        let (view, ci, pos) = (self.view(), self.sel_col, self.sel_idx);
        let Some(ids) = self.board.reorder_card(ci, view, pos, d) else { return };
        if self.save(&ids, false) {
            self.sel_idx = (pos as isize + d) as usize;
        }
    }

    fn toggle_hidden(&mut self) {
        if let Some(id) = self.selected_id() {
            self.board.toggle_card_hidden(&id);
            self.save(&[id], false);
        }
    }

    fn toggle_sprint(&mut self) {
        if let Some(id) = self.selected_id() {
            self.board.toggle_card_sprint(&id);
            self.save(&[id], false);
        }
    }
}

/// Maps a Cyrillic (ЙЦУКЕН) character to the Latin hotkey on the same physical key,
/// so board hotkeys work on a Russian layout. Case is kept; other chars pass through.
fn latin_key(c: char) -> char {
    match c {
        'р' => 'h',
        'о' => 'j',
        'л' => 'k',
        'д' => 'l',
        'Р' => 'H',
        'О' => 'J',
        'Л' => 'K',
        'Д' => 'L',
        'й' => 'q',
        'Й' => 'Q',
        'т' => 'n',
        'Т' => 'N',
        'у' => 'e',
        'У' => 'E',
        'в' => 'd',
        'В' => 'D',
        'ч' => 'x',
        'Ч' => 'X',
        'м' => 'v',
        'М' => 'V',
        'ы' => 's',
        'Ы' => 'S',
        'з' => 'p',
        'З' => 'P',
        'с' => 'c',
        'С' => 'C',
        'к' => 'r',
        'К' => 'R',
        'и' => 'b',
        'И' => 'B',
        'Б' => '<',
        'Ю' => '>',
        'ъ' => ']',
        'ь' => 'm',
        'Ь' => 'M',
        'ш' => 'i',
        'Ш' => 'I',
        'ф' => 'a',
        'Ф' => 'A',
        other => other,
    }
}
