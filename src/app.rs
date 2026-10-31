//! Application state and all board logic (no drawing).
use crate::cards::CardRef;
use crate::editor::TextBuf;
use crate::agents;
use crate::files::FileIndex;
use crate::form::{Field, FieldKind, Form, FormEvent, Opt, Purpose, Suggest};
use crate::install::{self, Tool, ToolInfo};
use crate::model::{Column, Config, Kind, Store, Task};
use crate::theme;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

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

/// State of the tool checklist popup.
pub struct ToolPick {
    /// Ready tools of the registry and whether each was found in the project.
    pub items: Vec<(ToolInfo, bool)>,
    pub checked: Vec<bool>,
    pub cursor: usize,
    /// True while `.pumba/` is still to be created (the init flow); false for an existing board.
    pub creating: bool,
}

impl ToolPick {
    pub fn chosen(&self) -> Vec<Tool> {
        self.items.iter().zip(&self.checked).filter(|(_, c)| **c).map(|((i, _), _)| i.tool).collect()
    }
}

pub struct App {
    pub store: Store,
    pub project: String,
    pub cfg: Config,
    pub tasks: Vec<Task>,
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
    pub sig: u128,
    pub quit: bool,
    /// Project file index for the `@` picker: built on the first `@` after a form opened.
    files: Option<Rc<FileIndex>>,
    /// User home whose `.claude/agents` is listed read-only by the executor suggestions (None: skip).
    pub home: Option<PathBuf>,
    /// False while board.json is missing or failed to load: nothing is written then.
    ok: bool,
    /// Card file names that failed to read or parse at the last load.
    pub unreadable: Vec<String>,
    /// How many times AGENTS.md was written by this app (tests watch it for write loops).
    pub agents_writes: u32,
    /// Raw text of every card file as this app last loaded or wrote it: the base for merging
    /// with edits made on disk behind our back.
    seen: HashMap<String, String>,
    /// Cards whose body was changed on disk since `seen`, noted by the last commit: (id, disk body kept).
    ext: Vec<(String, bool)>,
}

// ------------------------------------------------------------ form helpers

fn opt(value: &str, label: &str, hint: &str) -> Opt {
    Opt { value: value.into(), label: label.into(), hint: hint.into() }
}

fn text_field(key: &str, label: &str, val: &str, multiline: bool) -> Field {
    let suggest = if multiline { Suggest::Files } else { Suggest::None };
    Field { key: key.into(), label: label.into(), kind: FieldKind::Text(TextBuf::new(val, multiline)), side: false, suggest }
}

/// Select field preselecting `cur`; an unknown non-empty `cur` is appended so it is not lost.
fn sel_field(key: &str, label: &str, mut opts: Vec<Opt>, cur: &str) -> Field {
    let idx = match opts.iter().position(|o| o.value == cur) {
        Some(i) => i,
        None if !cur.is_empty() => {
            opts.push(opt(cur, cur, ""));
            opts.len() - 1
        }
        None => 0,
    };
    Field { key: key.into(), label: label.into(), kind: FieldKind::Select { opts, idx }, side: false, suggest: Suggest::None }
}

fn plain_opts(first: &str, rest: &[String]) -> Vec<Opt> {
    std::iter::once(first).chain(rest.iter().map(|s| s.as_str())).map(|s| opt(s, if s == "default" { "по умолчанию" } else { s }, "")).collect()
}

/// Visible label for a stored color id.
fn color_label(id: &str) -> &str {
    match id {
        "purple" => "фиолетовый",
        "orange" => "оранжевый",
        "green" => "зелёный",
        "red" => "красный",
        "pink" => "розовый",
        "tan" => "песочный",
        other => other,
    }
}

fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        let c = if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' };
        if c != '-' || (!s.is_empty() && !s.ends_with('-')) {
            s.push(c);
        }
    }
    let s = s.trim_end_matches('-');
    if s.is_empty() { "column".into() } else { s.into() }
}

impl App {
    pub fn new(root: PathBuf, exists: bool) -> App {
        let project = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string());
        let store = Store::new(&root);
        let mut app = App {
            sig: store.signature(),
            store,
            project,
            cfg: Config::default(),
            tasks: vec![],
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
            ok: false,
            unreadable: Vec::new(),
            agents_writes: 0,
            seen: HashMap::new(),
            ext: Vec::new(),
        };
        if exists {
            app.reload();
        } else {
            app.mode = Mode::Confirm(app.init_msg(), Action::Init);
        }
        app
    }

    fn root(&self) -> PathBuf {
        self.store.dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.store.dir.clone())
    }

    fn init_msg(&self) -> String {
        format!("Папка .pumba/ не найдена. Создать её в {}?", self.root().display())
    }

    /// Checklist of ready tools. New board: found in the project (else Claude Code); existing
    /// board: the saved choice (same default when nothing was saved yet).
    fn tool_pick(&self, creating: bool) -> ToolPick {
        let root = self.root();
        let found = install::detect(&root);
        let saved = if creating { Vec::new() } else { install::from_ids(&self.cfg.tools) };
        let pre = if saved.is_empty() { install::preselect(&root) } else { saved };
        let items: Vec<(ToolInfo, bool)> = install::ready_tools().into_iter().map(|t| (t.clone(), found.contains(&t.tool))).collect();
        let checked = items.iter().map(|(t, _)| pre.contains(&t.tool)).collect();
        ToolPick { items, checked, cursor: 0, creating }
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

    /// Install result: the summary, or the first warning (and «и ещё N») followed by the summary.
    fn report_status(&mut self, summary: String, warnings: &[String]) {
        match install::warnings_line(warnings) {
            Some(w) => self.warn(format!("Внимание: {w} · {summary}")),
            None => self.info(summary),
        }
    }

    pub fn tick(&mut self) {
        let ttl = if self.status_warn { 12 } else { 4 };
        if self.status.as_ref().is_some_and(|s| s.2.elapsed() > Duration::from_secs(ttl)) {
            self.status = None;
            self.status_warn = false;
        }
        if matches!(self.mode, Mode::Board) && self.store.signature() != self.sig {
            self.reload();
        }
    }

    // ----------------------------------------------------------- queries

    pub fn effective_col(&self, t: &Task) -> usize {
        self.cfg.columns.iter().position(|c| c.id == t.column).unwrap_or(0)
    }

    fn visible(&self, t: &Task) -> bool {
        if t.deleted {
            return false;
        }
        if t.hidden && !self.show_hidden {
            return false;
        }
        if self.sprint_only {
            let cur = Some(self.cfg.sprint);
            return t.sprint == cur
                || (t.kind == Kind::Epic
                    && self.tasks.iter().any(|c| !c.deleted && c.parent.as_deref() == Some(t.id.as_str()) && c.sprint == cur));
        }
        true
    }

    pub fn col_task_idx(&self, ci: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.tasks.len())
            .filter(|&i| self.effective_col(&self.tasks[i]) == ci && self.visible(&self.tasks[i]))
            .collect();
        v.sort_by(|&a, &b| {
            let (a, b) = (&self.tasks[a], &self.tasks[b]);
            a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id))
        });
        v
    }

    pub fn selected_idx(&self) -> Option<usize> {
        self.col_task_idx(self.sel_col).get(self.sel_idx).copied()
    }

    /// Indices of trashed cards, newest deleted first (no timestamp sorts last, ties by id).
    pub fn trashed(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.tasks.len()).filter(|&i| self.tasks[i].deleted).collect();
        v.sort_by(|&a, &b| {
            let (a, b) = (&self.tasks[a], &self.tasks[b]);
            b.deleted_at.unwrap_or(0).cmp(&a.deleted_at.unwrap_or(0)).then_with(|| a.id.cmp(&b.id))
        });
        v
    }

    pub fn epic_progress(&self, epic_id: &str) -> (usize, usize) {
        let last = self.cfg.columns.len().saturating_sub(1);
        let kids = self.tasks.iter().filter(|t| !t.deleted && t.parent.as_deref() == Some(epic_id));
        kids.fold((0, 0), |(done, total), t| (done + (self.effective_col(t) == last) as usize, total + 1))
    }

    /// Dependencies of `t` that exist and are not in the last (done) column. Unknown and trashed ids are ignored.
    pub fn blockers(&self, t: &Task) -> Vec<&Task> {
        let last = self.cfg.columns.len().saturating_sub(1);
        t.depends
            .iter()
            .filter_map(|d| self.tasks.iter().find(|x| x.id == *d))
            .filter(|x| !x.deleted && self.effective_col(x) != last)
            .collect()
    }

    /// Non-trashed cards that list `id` in their `depends`, sorted by id. Derived on demand:
    /// the dependent card's `depends:` is the only stored side of the edge.
    pub fn dependents(&self, id: &str) -> Vec<&Task> {
        let mut v: Vec<&Task> = self.tasks.iter().filter(|t| !t.deleted && t.depends.iter().any(|d| d == id)).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
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

    fn next_order(&self, ci: usize) -> i64 {
        self.tasks.iter().filter(|t| self.effective_col(t) == ci).map(|t| t.order).max().map_or(0, |m| m + 1)
    }

    // ------------------------------------------------------- persistence

    /// Reload from disk, keeping the selected card. A failed load disables writing. When the loaded
    /// state differs from the previous (successfully loaded) one, AGENTS.md is regenerated so the
    /// index follows external edits; the signature only covers board.json and the cards, so that
    /// write does not trigger another reload.
    pub fn reload(&mut self) {
        let keep = self.selected_id();
        let was_ok = self.ok;
        match self.store.load_full() {
            Ok(l) => {
                let mut cfg = l.cfg;
                if cfg.columns.is_empty() {
                    cfg.columns = Config::default().columns;
                }
                let changed = was_ok && (cfg != self.cfg || l.tasks != self.tasks);
                self.cfg = cfg;
                self.tasks = l.tasks;
                self.seen = l.raws;
                self.unreadable = l.unreadable;
                self.ok = true;
                if changed {
                    match self.store.write_agents(&self.cfg, &self.tasks) {
                        Ok(()) => self.agents_writes += 1,
                        Err(e) => self.err(format!("Не удалось обновить AGENTS.md: {e}")),
                    }
                }
                match self.unreadable_note() {
                    Some(note) => self.err(note),
                    // the warning is gone with the files: do not leave it on screen
                    None if self.status.as_ref().is_some_and(|s| s.0.starts_with("Не прочитано")) => self.status = None,
                    None => {}
                }
            }
            Err(e) => {
                self.ok = false;
                self.unreadable.clear();
                self.err(format!("Не удалось загрузить .pumba: {e}"));
            }
        }
        self.sig = self.store.signature();
        self.scrolls.resize(self.cfg.columns.len(), 0);
        self.clamp();
        if let Some(id) = keep {
            self.select_task(&id);
        }
    }

    /// One-line warning about card files that could not be read: first 3 names plus the rest as a count.
    pub fn unreadable_note(&self) -> Option<String> {
        if self.unreadable.is_empty() {
            return None;
        }
        let names = self.unreadable.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        let rest = self.unreadable.len().saturating_sub(3);
        Some(if rest > 0 { format!("Не прочитано: {names} и ещё {rest}") } else { format!("Не прочитано: {names}") })
    }

    /// Before a card is rewritten from memory: when its file changed on disk since this app last
    /// loaded or wrote it, merge (see `Task::merge_disk`): fields this operation did not change take
    /// the disk value, in particular the body of a card that was only moved, reordered, hidden, ...
    fn merge_external(&mut self, save: &[String]) -> Result<(), String> {
        self.ext.clear();
        for id in save {
            let Some(i) = self.tasks.iter().position(|t| t.id == *id) else { continue };
            match (self.store.read_task_raw(id), self.seen.get(id)) {
                (Some(disk), Some(base)) if disk != *base => {
                    let (Some(dt), Some(bt)) = (Task::from_md(&disk, id), Task::from_md(base, id)) else {
                        return Err(format!("{id}.md изменён снаружи и не читается; ничего не записано"));
                    };
                    if dt.body != bt.body {
                        let ours_changed = self.tasks[i].body != bt.body;
                        self.ext.push((id.clone(), !ours_changed));
                    }
                    self.tasks[i].merge_disk(&bt, &dt);
                }
                (Some(_), None) => return Err(format!("{id}.md уже есть на диске (создан снаружи); обновите доску")),
                _ => {}
            }
        }
        Ok(())
    }

    /// Write changed tasks / board.json, then AGENTS.md. Task files are never removed.
    fn commit(&mut self, save: &[String], cfg: bool) -> Result<(), String> {
        if !self.ok {
            return Err("board.json не удалось загрузить; сначала исправьте его (ничего не записано)".into());
        }
        // something else touched the board since our last load: resync the rest of it after writing
        let external = self.store.signature() != self.sig;
        self.merge_external(save)?;
        // Remember every file about to be written so a failure part-way leaves the disk as it was.
        let mut paths: Vec<PathBuf> = save.iter().map(|id| self.store.dir.join("tasks").join(format!("{id}.md"))).collect();
        if cfg {
            paths.push(self.store.dir.join("board.json"));
        }
        paths.push(self.store.dir.join("AGENTS.md"));
        let before: Vec<(PathBuf, Option<Vec<u8>>)> =
            paths.into_iter().map(|p| { let old = std::fs::read(&p).ok(); (p, old) }).collect();
        let r = (|| -> crate::model::Res<()> {
            for t in self.tasks.iter().filter(|t| save.contains(&t.id)) {
                self.store.save_task(t)?;
            }
            if cfg {
                self.store.save_config(&self.cfg)?;
            }
            self.store.write_agents(&self.cfg, &self.tasks)?;
            Ok(())
        })();
        if r.is_err() {
            for (p, old) in &before {
                let _ = match old {
                    Some(bytes) => std::fs::write(p, bytes),
                    None => std::fs::remove_file(p),
                };
            }
        } else {
            self.agents_writes += 1;
            for t in self.tasks.iter().filter(|t| save.contains(&t.id)) {
                self.seen.insert(t.id.clone(), t.to_md());
            }
        }
        self.sig = self.store.signature();
        r.map_err(|e| format!("Ошибка записи: {e}"))?;
        if external {
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

    /// Show a form. The file index is dropped so the next `@` sees files created meanwhile.
    fn open_form(&mut self, form: Form) {
        self.files = None;
        self.mode = Mode::Form(Box::new(form));
    }

    /// Give the form the file index as soon as its cursor sits in an `@` token (built lazily).
    fn attach_index(&mut self, f: &mut Form) {
        if f.needs_index() {
            let root = self.store.dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.store.dir.clone());
            let ix = self.files.get_or_insert_with(|| Rc::new(FileIndex::build(&root)));
            f.index = Some(ix.clone());
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
                let Some(t) = self.tasks.iter_mut().find(|t| t.id == id && !t.deleted) else { return };
                t.deleted = true;
                t.deleted_at = Some(now_secs());
                if self.save(&[id.clone()], false) {
                    self.info(format!("В корзине: {id}. Восстановить: B"));
                }
                self.clamp();
            }
            Action::DeleteColumn(id) => self.delete_column(&id),
            Action::NextSprint => {
                let (old, last) = (self.cfg.sprint, self.cfg.columns.len().saturating_sub(1));
                self.cfg.sprint = old + 1;
                let mut moved = vec![];
                for i in 0..self.tasks.len() {
                    if !self.tasks[i].deleted && self.tasks[i].sprint == Some(old) && self.effective_col(&self.tasks[i]) != last {
                        self.tasks[i].sprint = Some(old + 1);
                        moved.push(self.tasks[i].id.clone());
                    }
                }
                if self.save(&moved, true) {
                    self.info(format!("Спринт {} начат", old + 1));
                }
            }
        }
    }

    // ------------------------------------------------------------- tools

    fn tools_key(&mut self, mut p: ToolPick, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(latin_key(c)),
            other => other,
        };
        let n = p.items.len();
        match code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            _ if ctrl => self.mode = Mode::InitTools(p),
            // new board: only `.pumba/`, no agents; existing board: just close
            KeyCode::Esc if p.creating => self.create_board(&[]),
            KeyCode::Esc => {}
            KeyCode::Enter => {
                let tools = p.chosen();
                if p.creating { self.create_board(&tools) } else { self.install_tools(&tools) }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                p.cursor = p.cursor.saturating_sub(1);
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                p.cursor = (p.cursor + 1).min(n.saturating_sub(1));
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Char(' ') => {
                if let Some(c) = p.checked.get_mut(p.cursor) {
                    *c = !*c;
                }
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all = p.checked.iter().all(|&c| c);
                p.checked.iter_mut().for_each(|c| *c = !all);
                self.mode = Mode::InitTools(p);
            }
            _ => self.mode = Mode::InitTools(p),
        }
    }

    /// Init flow: create `.pumba/`, remember the chosen tools, install their packs.
    fn create_board(&mut self, tools: &[Tool]) {
        if let Err(e) = self.store.init() {
            self.mode = Mode::Confirm(self.init_msg(), Action::Init);
            self.err(format!("Не удалось создать .pumba/: {e}"));
            return;
        }
        // .pumba/ stays created even when the installer fails
        let root = self.root();
        let saved = self.store.set_tools(&install::to_ids(tools));
        let report = if tools.is_empty() { Ok(install::InstallReport::default()) } else { install::install(&root, tools, false) };
        self.reload();
        if !self.ok {
            return;
        }
        match (report, saved) {
            (Ok(_), Ok(())) if tools.is_empty() => self.info("Создано. Агенты не установлены (установить: I)"),
            (Ok(r), Ok(())) => {
                self.report_status(format!("Создано. Установлено: {} файлов, пропущено: {}", r.installed.len(), r.skipped.len()), &r.warnings)
            }
            (Err(e), _) => self.err(format!("Папка .pumba/ создана, установка не удалась: {e}")),
            (_, Err(e)) => self.err(format!("Папка .pumba/ создана, выбор не записан в board.json: {e}")),
        }
    }

    /// `I` on an existing board: install for the chosen tools (existing files are kept) and save the choice.
    fn install_tools(&mut self, tools: &[Tool]) {
        let report = if tools.is_empty() { Ok(install::InstallReport::default()) } else { install::install(&self.root(), tools, false) };
        let r = match report {
            Ok(r) => r,
            Err(e) => return self.err(format!("Установка не удалась: {e}")),
        };
        let ids = install::to_ids(tools);
        if self.cfg.tools != ids {
            self.cfg.tools = ids;
            if !self.save(&[], true) {
                return;
            }
        }
        if tools.is_empty() {
            self.info("Инструменты не выбраны, ничего не установлено");
        } else {
            self.report_status(format!("Установлено: {} файлов, пропущено: {}", r.installed.len(), r.skipped.len()), &r.warnings);
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
        let t = &mut self.tasks[ti];
        t.deleted = false;
        t.deleted_at = None;
        let id = t.id.clone();
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
        let order = self.next_order(nc);
        let t = &mut self.tasks[ti];
        t.column = self.cfg.columns[nc].id.clone();
        t.order = order;
        let id = t.id.clone();
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
        let vis = self.col_task_idx(self.sel_col);
        let j = self.sel_idx as isize + d;
        if self.sel_idx >= vis.len() || j < 0 || j as usize >= vis.len() {
            return;
        }
        let mut changed = vec![];
        for (n, &i) in vis.iter().enumerate() {
            if self.tasks[i].order != n as i64 {
                self.tasks[i].order = n as i64;
                changed.push(i);
            }
        }
        let (a, b) = (vis[self.sel_idx], vis[j as usize]);
        let tmp = self.tasks[a].order;
        self.tasks[a].order = self.tasks[b].order;
        self.tasks[b].order = tmp;
        changed.extend([a, b]);
        changed.sort_unstable();
        changed.dedup();
        let ids: Vec<String> = changed.iter().map(|&i| self.tasks[i].id.clone()).collect();
        if self.save(&ids, false) {
            self.sel_idx = j as usize;
        }
    }

    fn toggle_hidden(&mut self) {
        if let Some(i) = self.selected_idx() {
            self.tasks[i].hidden = !self.tasks[i].hidden;
            let id = self.tasks[i].id.clone();
            self.save(&[id], false);
        }
    }

    fn toggle_sprint(&mut self) {
        if let Some(i) = self.selected_idx() {
            let cur = Some(self.cfg.sprint);
            let t = &mut self.tasks[i];
            t.sprint = if t.sprint == cur { None } else { cur };
            let id = t.id.clone();
            self.save(&[id], false);
        }
    }

    // ----------------------------------------------------- column actions

    /// The first (backlog) and the last (done) columns are pinned by position, not by id.
    fn is_pinned(&self, i: usize) -> bool {
        i == 0 || i + 1 == self.cfg.columns.len()
    }

    fn move_column(&mut self, d: isize) {
        let i = self.sel_col;
        let j = i as isize + d;
        let n = self.cfg.columns.len();
        // pinned columns stay put; the others move only inside 1..=n-2
        if self.is_pinned(i) || j < 1 || j as usize + 1 >= n {
            if let Some(c) = self.cfg.columns.get(i) {
                let name = c.name.clone();
                self.err(format!("Колонки «{name}» нельзя двигать: первая и последняя закреплены"));
            }
            return;
        }
        let j = j as usize;
        self.cfg.columns.swap(i, j);
        self.scrolls.swap(i, j);
        if self.save(&[], true) {
            self.sel_col = j;
        }
    }

    fn delete_column(&mut self, id: &str) {
        let Some(pos) = self.cfg.columns.iter().position(|c| c.id == id) else { return };
        if self.cfg.columns.len() <= 1 {
            return self.err("Нельзя удалить единственную колонку");
        }
        if self.is_pinned(pos) {
            let name = self.cfg.columns[pos].name.clone();
            return self.err(format!("Колонку «{name}» нельзя удалить: первая и последняя закреплены"));
        }
        // Cards of the deleted column (or of unknown columns, if it was first) go to the first survivor.
        let target = self.cfg.columns.iter().position(|c| c.id != id).unwrap_or(0);
        let target_id = self.cfg.columns[target].id.clone();
        let mut order = self.next_order(target);
        let mut moved = vec![];
        for i in 0..self.tasks.len() {
            if self.effective_col(&self.tasks[i]) == pos {
                self.tasks[i].column = target_id.clone();
                self.tasks[i].order = order;
                order += 1;
                moved.push(self.tasks[i].id.clone());
            }
        }
        self.cfg.columns.remove(pos);
        // no column may keep pointing at the deleted one
        for c in &mut self.cfg.columns {
            if c.next == id {
                c.next.clear();
            }
        }
        self.scrolls.resize(self.cfg.columns.len(), 0);
        if self.save(&moved, true) {
            self.sel_col = pos.saturating_sub(1).min(self.cfg.columns.len() - 1);
            self.sel_idx = 0;
        }
    }

    // -------------------------------------------------------------- forms

    fn open_task_form(&mut self, kind: Kind, edit: Option<usize>) {
        let cfg = &self.cfg;
        let cur = edit.map(|i| &self.tasks[i]);
        let (parent, sprint) = match cur {
            Some(t) => (t.parent.clone().unwrap_or_default(), t.sprint),
            None => {
                let sel = self.selected_idx().map(|i| &self.tasks[i]);
                let p = sel.map(|t| if t.kind == Kind::Epic { Some(t.id.clone()) } else { t.parent.clone() });
                (p.flatten().unwrap_or_default(), None)
            }
        };
        let (n, next) = (cfg.sprint, cfg.sprint + 1);
        let sprint_opts = vec![
            opt("", "нет", ""),
            opt(&n.to_string(), &format!("Спринт {n} (текущий)"), ""),
            opt(&next.to_string(), &format!("Спринт {next} (следующий)"), ""),
        ];
        // new cards start from the project defaults; an existing card keeps its own values
        let model = cur.map_or(cfg.default_model.as_str(), |t| t.model.as_str());
        let effort = cur.map_or(cfg.default_effort.as_str(), |t| t.effort.as_str());
        // Order is the visual and Tab order: left column (title, body), then the side column.
        let mut fields = vec![
            text_field("title", "Название", cur.map_or("", |t| t.title.as_str()), false),
            text_field("body", "Описание для агента", cur.map_or("", |t| t.body.as_str()), true),
            sel_field("model", "Модель", plain_opts("default", &cfg.models), model).side(),
            sel_field("effort", "Уровень", plain_opts("default", &cfg.efforts), effort).side(),
            sel_field("sprint", "Спринт", sprint_opts, &sprint.map(|s| s.to_string()).unwrap_or_default()).side(),
        ];
        if kind == Kind::Task {
            let mut epics: Vec<&Task> = self.tasks.iter().filter(|t| t.kind == Kind::Epic && !t.deleted).collect();
            epics.sort_by(|a, b| a.id.cmp(&b.id));
            let mut opts = vec![opt("", "нет", "")];
            opts.extend(epics.iter().map(|e| opt(&e.id, &format!("{} {}", e.id, e.title), "")));
            fields.push(sel_field("parent", "Эпик", opts, &parent).side());
        }
        let depends = cur.map_or(String::new(), |t| t.depends.join(", "));
        fields.push(text_field("depends", "Зависит от (id через запятую)", &depends, false).side().suggest(Suggest::Cards));
        // reverse edges, derived from the other cards' `depends:` (trashed cards are not shown)
        let blocks = cur.map_or(String::new(), |t| {
            self.dependents(&t.id).iter().map(|d| d.id.as_str()).collect::<Vec<_>>().join(", ")
        });
        fields.push(text_field("blocks", "Блокирует (id через запятую)", &blocks, false).side().suggest(Suggest::Cards));
        let (title, purpose) = match cur {
            Some(t) => (format!("Правка {}", t.id), Purpose::EditTask(t.id.clone())),
            None if kind == Kind::Epic => ("Новый эпик".to_string(), Purpose::NewTask(kind)),
            None => ("Новая задача".to_string(), Purpose::NewTask(kind)),
        };
        // suggestions for the dependency field: every card except the one being edited
        let me = cur.map(|t| t.id.as_str());
        let cards: Vec<CardRef> = self
            .tasks
            .iter()
            .filter(|t| !t.deleted && Some(t.id.as_str()) != me)
            .map(|t| {
                let col = self.cfg.columns.get(self.effective_col(t)).map_or("", |c| c.name.as_str());
                CardRef::new(&t.id, t.kind, &t.title, col, &t.body)
            })
            .collect();
        let mut form = Form::new(title, purpose, fields);
        form.cards = Rc::new(cards);
        self.open_form(form);
    }

    fn open_column_form(&mut self, edit: Option<usize>) {
        let c = edit.map(|i| self.cfg.columns[i].clone());
        let (name, color, agent) = c.as_ref().map_or(("", "", ""), |c| (&c.name[..], &c.color[..], &c.agent[..]));
        let (before, rule, after) = c.as_ref().map_or(("", "", ""), |c| (&c.before[..], &c.rule[..], &c.after[..]));
        let colors = theme::COLOR_NAMES.iter().map(|n| opt(n, color_label(n), "")).collect();
        // «next» offers every other column; a stored pointer to a missing column shows as «нет»
        let me = c.as_ref().map(|c| c.id.as_str());
        let mut next_opts = vec![opt("", "нет", "")];
        next_opts.extend(self.cfg.columns.iter().filter(|o| Some(o.id.as_str()) != me).map(|o| opt(&o.id, &o.name, "")));
        let next = c.as_ref().map_or("", |c| c.next.as_str());
        let next = if next_opts.iter().any(|o| o.value == next) { next } else { "" };
        // Order is the visual and Tab order: the rule parts in reading order (left column), then
        // the side column. Old boards have only `rule` («Работа»).
        let fields = vec![
            text_field("name", "Название", name, false),
            text_field("before", "Перед работой (проверки, стоп)", before, true),
            text_field("rule", "Работа (что делает исполнитель)", rule, true),
            text_field("after", "После работы (проверка, куда)", after, true),
            text_field("agent", "Исполнитель (пусто — нет)", agent, false).side().suggest(Suggest::Agents),
            sel_field("next", "После успеха", next_opts, next).side(),
            sel_field("color", "Цвет", colors, color).side(),
        ];
        let (title, purpose) = match &c {
            Some(c) => (format!("Правка колонки {}", c.name), Purpose::EditColumn(c.id.clone())),
            None => ("Новая колонка".to_string(), Purpose::NewColumn),
        };
        let mut form = Form::new(title, purpose, fields);
        // rescanned on every open: agent files may have been added meanwhile
        form.agents = Rc::new(agents::scan(&self.root(), self.home.as_deref()));
        self.open_form(form);
    }

    fn open_rules_form(&mut self) {
        let mut fields: Vec<Field> = self
            .cfg
            .rules
            .iter()
            .map(|r| {
                let opts = r.options.iter().map(|o| opt(&o.id, &o.label, &o.text)).collect();
                sel_field(&r.id, &r.title, opts, &r.value)
            })
            .collect();
        fields.push(text_field("custom", "Свои правила (Markdown)", &self.cfg.custom_rules, true));
        self.open_form(Form::new("Правила проекта", Purpose::Rules, fields));
    }

    fn open_models_form(&mut self) {
        // the defaults come first (Tab order); their options are the lists as they are now
        let fields = vec![
            sel_field("default_model", "Модель по умолчанию для новых карточек", plain_opts("default", &self.cfg.models), &self.cfg.default_model),
            sel_field("default_effort", "Уровень по умолчанию для новых карточек", plain_opts("default", &self.cfg.efforts), &self.cfg.default_effort),
            text_field("models", "Модели (по одной в строке)", &self.cfg.models.join("\n"), true),
            text_field("efforts", "Уровни размышления (по одному в строке)", &self.cfg.efforts.join("\n"), true),
        ];
        self.open_form(Form::new("Модели и уровни", Purpose::Models, fields));
    }

    fn form_key(&mut self, mut f: Box<Form>, key: KeyEvent) {
        let ev = f.handle(key);
        self.attach_index(&mut f);
        match ev {
            FormEvent::None => self.mode = Mode::Form(f),
            FormEvent::Cancel => {}
            FormEvent::Save => {
                let snap = (self.cfg.clone(), self.tasks.clone());
                if let Err(e) = self.apply_form(&f) {
                    (self.cfg, self.tasks) = snap;
                    self.err(e);
                    self.mode = Mode::Form(f);
                }
            }
        }
    }

    /// Validate a form and apply it to the board. The caller restores state on Err.
    fn apply_form(&mut self, f: &Form) -> Result<(), String> {
        match &f.purpose {
            Purpose::NewTask(kind) => self.apply_task(f, *kind, None),
            Purpose::EditTask(id) => self.apply_task(f, Kind::Task, Some(id)),
            Purpose::NewColumn => self.apply_column(f, None),
            Purpose::EditColumn(id) => self.apply_column(f, Some(id)),
            Purpose::Rules => {
                for r in &mut self.cfg.rules {
                    let v = f.sel(&r.id);
                    if !v.is_empty() {
                        r.value = v;
                    }
                }
                self.cfg.custom_rules = f.text("custom").trim_end().to_string();
                self.commit(&[], true)
            }
            Purpose::Models => {
                self.cfg.models = parse_list(&f.text("models"));
                self.cfg.efforts = parse_list(&f.text("efforts"));
                // a default that left the list in the same form falls back to `default`
                let (dm, de) = (f.sel("default_model"), f.sel("default_effort"));
                let mut note = Vec::new();
                self.cfg.default_model = if dm == "default" || self.cfg.models.contains(&dm) {
                    dm
                } else {
                    note.push("Модель по умолчанию сброшена");
                    "default".into()
                };
                self.cfg.default_effort = if de == "default" || self.cfg.efforts.contains(&de) {
                    de
                } else {
                    note.push("Уровень по умолчанию сброшен");
                    "default".into()
                };
                self.commit(&[], true)?;
                self.info(if note.is_empty() { "Списки сохранены".to_string() } else { format!("Списки сохранены. {}", note.join(". ")) });
                Ok(())
            }
        }
    }

    fn new_id(&self, kind: Kind) -> String {
        let p = kind.prefix();
        let max = self.tasks.iter().filter_map(|t| t.id.strip_prefix(p)?.parse::<u32>().ok()).max().unwrap_or(0);
        // an id whose file already exists (a card created outside and not loaded yet) is skipped
        let mut n = max + 1;
        while self.store.task_exists(&format!("{p}{n:03}")) {
            n += 1;
        }
        format!("{p}{n:03}")
    }

    /// `edit` is Some(id) when editing; the kind is then taken from the stored task.
    fn apply_task(&mut self, f: &Form, kind: Kind, edit: Option<&str>) -> Result<(), String> {
        let title = f.text("title").trim().to_string();
        if title.is_empty() {
            return Err("Нужно указать название".into());
        }
        let (model, effort) = (f.sel("model"), f.sel("effort"));
        let sprint = f.sel("sprint").parse().ok();
        let parent = Some(f.sel("parent")).filter(|p| !p.is_empty());
        // same normalisation as `Task::from_md`, so an untouched description equals what was loaded
        let body = f.text("body").trim_start_matches('\n').trim_end().to_string();
        let depends = self.parse_depends(&f.text("depends"), edit)?;
        // the id a new card is going to get; it exists in no card yet, so only the checks below see it
        let me = match edit {
            Some(id) => id.to_string(),
            None => self.new_id(kind),
        };
        let blocks = self.parse_blocks(&f.text("blocks"), edit, &depends)?;
        // reverse edges: B.depends += me for every newly blocked B, -= me for every dropped one.
        // Trashed dependents never show in the field, so they are neither added nor removed.
        let all_dependents: Vec<&str> =
            self.tasks.iter().filter(|t| t.depends.iter().any(|d| *d == me)).map(|t| t.id.as_str()).collect();
        let added: Vec<String> = blocks.iter().filter(|b| !all_dependents.contains(&b.as_str())).cloned().collect();
        let removed: Vec<String> =
            self.dependents(&me).iter().map(|t| t.id.clone()).filter(|id| !blocks.contains(id)).collect();
        self.check_cycles(&me, &depends, &added, &removed)?;
        let id = match edit {
            Some(id) => {
                let t = self.tasks.iter_mut().find(|t| t.id == id).ok_or("Задачи больше нет")?;
                t.title = title;
                t.model = model;
                t.effort = effort;
                t.sprint = sprint;
                if t.kind == Kind::Task {
                    t.parent = parent;
                }
                t.body = body;
                t.depends = depends;
                id.to_string()
            }
            None => {
                let ci = self.sel_col;
                let t = Task {
                    id: me.clone(),
                    kind,
                    title,
                    column: self.cfg.columns[ci].id.clone(),
                    parent: if kind == Kind::Task { parent } else { None },
                    model,
                    effort,
                    sprint,
                    hidden: false,
                    order: self.next_order(ci),
                    depends,
                    deleted: false,
                    deleted_at: None,
                    body,
                };
                let id = t.id.clone();
                self.tasks.push(t);
                id
            }
        };
        // the card exists now: apply the reverse edges to the dependents, then write everything in one commit
        let mut save = vec![id.clone()];
        for b in added.iter().chain(removed.iter()) {
            let Some(t) = self.tasks.iter_mut().find(|t| t.id == *b) else { continue };
            if added.contains(b) {
                if !t.depends.contains(&id) {
                    t.depends.push(id.clone());
                }
            } else {
                t.depends.retain(|d| *d != id);
            }
            save.push(b.clone());
        }
        self.commit(&save, false)?;
        self.select_task(&id);
        if let Some(kept_disk) = self.ext.iter().find(|(i, _)| *i == id).map(|e| e.1) {
            self.err(if kept_disk {
                "Файл изменён снаружи: описание не перезаписано"
            } else {
                "Файл изменён снаружи: сохранён ваш текст описания"
            });
        }
        Ok(())
    }

    /// Parse and validate a dependency list (ids split by commas/spaces, duplicates dropped silently).
    /// Every id must exist and not be the card itself (cycles are checked later over both fields).
    fn parse_depends(&self, raw: &str, edit: Option<&str>) -> Result<Vec<String>, String> {
        let kept: Vec<String> = edit
            .and_then(|id| self.tasks.iter().find(|t| t.id == id))
            .map(|t| t.depends.clone())
            .unwrap_or_default();
        let mut deps: Vec<String> = Vec::new();
        for d in raw.split(|c: char| c == ',' || c.is_whitespace()).filter(|d| !d.is_empty()) {
            if !deps.iter().any(|x| x == d) {
                deps.push(d.to_string());
            }
        }
        for d in &deps {
            if edit == Some(d.as_str()) {
                return Err(format!("{d} не может зависеть от самой себя"));
            }
            match self.tasks.iter().find(|t| t.id == *d) {
                None => return Err(format!("Зависимость {d}: такой карточки нет")),
                // a trashed card cannot be newly picked; an entry that is already stored stays as is
                Some(t) if t.deleted && !kept.contains(d) => return Err(format!("Зависимость {d}: карточка в корзине")),
                Some(_) => {}
            }
        }
        Ok(deps)
    }

    /// Parse and validate the «Блокирует» list: cards that will depend on this one. Same rules as
    /// `parse_depends`, plus: an id cannot also sit in `depends` of the same form. For a new card
    /// (`edit` is None) there is no id yet, so the self check cannot trigger.
    fn parse_blocks(&self, raw: &str, edit: Option<&str>, depends: &[String]) -> Result<Vec<String>, String> {
        let mut blocks: Vec<String> = Vec::new();
        for d in raw.split(|c: char| c == ',' || c.is_whitespace()).filter(|d| !d.is_empty()) {
            if !blocks.iter().any(|x| x == d) {
                blocks.push(d.to_string());
            }
        }
        for b in &blocks {
            if edit == Some(b.as_str()) {
                return Err(format!("{b} не может блокировать саму себя"));
            }
            if depends.contains(b) {
                return Err(format!("{b}: нельзя одновременно зависеть от карточки и блокировать её"));
            }
            match self.tasks.iter().find(|t| t.id == *b) {
                None => return Err(format!("Блокирует {b}: такой карточки нет")),
                // a trashed card cannot be newly picked; one that already depends on this card stays as is
                Some(t) if t.deleted && !edit.is_some_and(|me| t.depends.iter().any(|d| d == me)) => {
                    return Err(format!("Блокирует {b}: карточка в корзине"))
                }
                Some(_) => {}
            }
        }
        Ok(blocks)
    }

    /// Reject the combination of this card's new `depends` and the reverse edges (`added` dependents
    /// get `me`, `removed` ones lose it). Every new edge touches `me`, so a new cycle exists iff `me`
    /// is reachable from one of its new dependencies in the resulting graph.
    fn check_cycles(&self, me: &str, deps: &[String], added: &[String], removed: &[String]) -> Result<(), String> {
        let next = |id: &str| -> Vec<String> {
            if id == me {
                return deps.to_vec();
            }
            let mut d = self.tasks.iter().find(|t| t.id == id).map(|t| t.depends.clone()).unwrap_or_default();
            if added.iter().any(|a| a == id) {
                d.push(me.to_string());
            }
            if removed.iter().any(|r| r == id) {
                d.retain(|x| x != me);
            }
            d
        };
        for d in deps {
            let mut seen: Vec<String> = Vec::new();
            let mut path = vec![me.to_string()];
            if let Some(p) = find_path(d, me, &next, &mut seen, &mut path) {
                return Err(format!("Цикл зависимостей: {}", p.join(" → ")));
            }
        }
        Ok(())
    }

    fn apply_column(&mut self, f: &Form, edit: Option<&str>) -> Result<(), String> {
        let name = f.text("name").trim().to_string();
        if name.is_empty() {
            return Err("Нужно указать название".into());
        }
        let color = f.sel("color");
        let part = |key: &str| f.text(key).trim_end().to_string();
        let (before, rule, after) = (part("before"), part("rule"), part("after"));
        let agent = f.text("agent").trim().to_string();
        let next = f.sel("next");
        if !next.is_empty() {
            if edit == Some(next.as_str()) {
                return Err("Колонка не может вести сама в себя".into());
            }
            if !self.cfg.columns.iter().any(|c| c.id == next) {
                return Err(format!("Следующая колонка «{next}»: такой колонки нет"));
            }
        }
        match edit {
            Some(id) => {
                let c = self.cfg.columns.iter_mut().find(|c| c.id == id).ok_or("Колонки больше нет")?;
                (c.name, c.color, c.before, c.rule, c.after, c.agent, c.next) = (name, color, before, rule, after, agent, next);
                self.commit(&[], true)
            }
            None => {
                let base = slug(&name);
                let mut id = base.clone();
                let mut n = 2;
                while self.cfg.columns.iter().any(|c| c.id == id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                // right before the pinned last column (a single-column board just appends)
                let n = self.cfg.columns.len();
                let at = if n >= 2 { n - 1 } else { n };
                self.cfg.columns.insert(at, Column { id, name, before, rule, after, color, agent, next });
                self.commit(&[], true)?;
                self.scrolls.insert(at.min(self.scrolls.len()), 0);
                self.scrolls.resize(self.cfg.columns.len(), 0);
                self.sel_col = at;
                self.sel_idx = 0;
                Ok(())
            }
        }
    }
}

/// DFS along `next` edges from `cur` looking for `target`; returns the full path when found.
fn find_path(
    cur: &str,
    target: &str,
    next: &dyn Fn(&str) -> Vec<String>,
    seen: &mut Vec<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    path.push(cur.to_string());
    if cur == target {
        return Some(path.clone());
    }
    if !seen.iter().any(|s| s == cur) {
        seen.push(cur.to_string());
        for n in next(cur) {
            if let Some(p) = find_path(&n, target, next, seen, path) {
                return Some(p);
            }
        }
    }
    path.pop();
    None
}

/// One value per line: trimmed, empty lines and the reserved `default` dropped, duplicates removed in order.
fn parse_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in s.lines().map(str::trim) {
        if !l.is_empty() && l != "default" && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    out
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
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

#[cfg(test)]
mod tests {
    use super::*;
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
        app.cfg.columns.push(app.cfg.columns[0].clone());
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
    fn ids_are_not_reused_after_trash() {
        let (mut app, root) = fresh();
        add(&mut app, Kind::Task, "A", "", "");
        add(&mut app, Kind::Task, "B", "", "");
        add(&mut app, Kind::Epic, "E", "", "");
        trash(&mut app, "T-002");
        trash(&mut app, "E-001");
        assert_eq!(add(&mut app, Kind::Task, "C", "", ""), "T-003");
        assert_eq!(add(&mut app, Kind::Epic, "F", "", ""), "E-002");
        assert!(raw(&root, "T-002").contains("deleted: true"));
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
    fn trashed_cards_follow_column_changes() {
        let (mut app, root) = fresh();
        app.sel_col = 1;
        add(&mut app, Kind::Task, "A", "", "");
        trash(&mut app, "T-001");
        app.sel_col = 1;
        app.on_key(k('X')); // delete "in-progress"
        app.on_key(k('y'));
        let t = app.tasks.iter().find(|t| t.id == "T-001").unwrap();
        assert!(t.deleted);
        assert_eq!(t.column, "backlog");
        assert_eq!(app.store.load().unwrap().1[0].column, "backlog");
        // swapping columns never touches card files
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert_eq!(app.tasks.iter().find(|t| t.id == "T-001").unwrap().column, "backlog");
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
    fn dependents_are_sorted_and_skip_trash() {
        let (mut app, root) = fresh();
        for n in ["A", "B", "C", "D"] {
            add(&mut app, Kind::Task, n, "", "");
        }
        set_depends(&mut app, "T-004", "T-001").unwrap();
        set_depends(&mut app, "T-002", "T-001, T-003").unwrap();
        let ids = |app: &App, id: &str| app.dependents(id).iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&app, "T-001"), ["T-002", "T-004"]);
        assert_eq!(ids(&app, "T-003"), ["T-002"]);
        assert!(ids(&app, "T-002").is_empty() && ids(&app, "NOPE").is_empty());
        trash(&mut app, "T-002");
        assert_eq!(ids(&app, "T-001"), ["T-004"]);
        assert!(ids(&app, "T-003").is_empty());
        let _ = std::fs::remove_dir_all(root);
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

    #[test]
    fn deleting_a_column_clears_pointers_to_it() {
        let (mut app, root) = fresh();
        assert_eq!(col(&app, "in-progress").next, "review");
        app.sel_col = app.cfg.columns.iter().position(|c| c.id == "review").unwrap();
        app.on_key(k('X'));
        app.on_key(k('y'));
        assert!(app.cfg.columns.iter().all(|c| c.id != "review"));
        // in-progress pointed at review: cleared; review's own pointer left with it; the rest is intact
        assert_eq!(col(&app, "in-progress").next, "");
        assert_eq!(col(&app, "in-progress").agent, "pumba-worker");
        for c in &app.cfg.columns {
            assert!(c.next.is_empty() || app.cfg.columns.iter().any(|o| o.id == c.next), "{}", c.id);
        }
        let disk = app.store.load().unwrap().0;
        assert_eq!(disk, app.cfg);
        let md = std::fs::read_to_string(root.join(".pumba/AGENTS.md")).unwrap();
        let ip = md.find("### В работе (`in-progress`)").unwrap();
        let section = &md[ip..ip + md[ip..].find("\n### ").unwrap()];
        assert!(section.contains("**Работа** (исполнитель: `pumba-worker`): "), "{section}");
        assert!(section.contains("в «Ждёт разработчика» (`on-dev`) и остановись. После успеха: остановиться и отчитаться; карточку переносит человек."), "{section}");
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
    fn card_move_keeps_a_column_changed_outside_for_untouched_fields() {
        let (mut app, root) = fresh();
        add(&mut app, Kind::Task, "A", "", "");
        // an agent moves the card to review while we only hide it
        external_write(&root, "T-001", |t| t.replace("column: backlog", "column: review"));
        app.select_task("T-001");
        app.on_key(k('x'));
        let t = disk_task(&app, "T-001");
        assert!(t.hidden);
        assert_eq!(t.column, "review");
        // the resync after the write put the card in its real column
        assert_eq!(app.effective_col(app.tasks.iter().find(|t| t.id == "T-001").unwrap()), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn external_change_to_other_cards_is_resynced_after_our_write() {
        let (mut app, root) = fresh();
        add(&mut app, Kind::Task, "A", "", "");
        add(&mut app, Kind::Task, "B", "", "");
        external_write(&root, "T-002", |t| t.replace("column: backlog", "column: done"));
        app.select_task("T-001");
        app.on_key(k('L')); // the next write notices the board changed and reloads
        assert_eq!(app.tasks.iter().find(|t| t.id == "T-002").unwrap().column, "done");
        assert_eq!(app.tasks.iter().find(|t| t.id == "T-001").unwrap().column, "in-progress");
        assert!(agents_file(&root).contains("| T-002 | task | done |"));
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

    #[test]
    fn new_card_id_skips_a_file_created_outside() {
        let (mut app, root) = fresh();
        add(&mut app, Kind::Task, "A", "", "");
        // an agent creates T-002 right now; the app has not noticed yet
        std::fs::write(root.join(".pumba/tasks/T-002.md"), "---\ntitle: Agent card\ncolumn: backlog\n---\n\nmine\n").unwrap();
        let id = add(&mut app, Kind::Task, "B", "", "");
        assert_eq!(id, "T-003");
        assert!(raw(&root, "T-002").contains("Agent card"));
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
}
