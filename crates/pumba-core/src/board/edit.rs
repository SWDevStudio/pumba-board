//! Changes to the board in memory. None of them writes: each returns what has to be written
//! (card ids, and whether board.json changed is documented per method); the caller then calls
//! `Board::commit` (or `write` + `reload`). Validation errors come before any change, so an Err
//! leaves the board as it was.
use super::{Board, View};
use crate::model::{Column, Kind, Task};

/// Card fields as the user entered them; `Board::apply_task` trims and validates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskInput {
    pub title: String,
    pub body: String,
    pub model: String,
    pub effort: String,
    pub sprint: Option<u32>,
    /// Epic of a task (ignored for epics); None or empty means none.
    pub parent: Option<String>,
    /// «Зависит от»: ids split by commas or spaces.
    pub depends: String,
    /// «Блокирует»: ids of the cards that are to depend on this one, same format.
    pub blocks: String,
}

/// Column fields as the user entered them; `Board::apply_column` trims and validates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnInput {
    pub name: String,
    /// «Перед работой».
    pub before: String,
    /// «Работа».
    pub rule: String,
    /// «После работы».
    pub after: String,
    pub color: String,
    /// Executor subagent (empty = none).
    pub agent: String,
    /// Column id a card moves to after success (empty = none).
    pub next: String,
}

/// Model and effort lists with the defaults for new cards (lists already parsed).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelsInput {
    pub models: Vec<String>,
    pub efforts: Vec<String>,
    pub default_model: String,
    pub default_effort: String,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Column id from a name: ASCII letters and digits lowercased, the rest collapsed into `-`.
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

impl Board {
    // ------------------------------------------------------ card actions

    /// Put the card at the end of column `ci`. False (nothing changed) for an unknown card or
    /// column. Write: the card.
    pub fn move_card_to(&mut self, id: &str, ci: usize) -> bool {
        let Some(ti) = self.find(id) else { return false };
        if ci >= self.cfg.columns.len() {
            return false;
        }
        let order = self.next_order(ci);
        let t = &mut self.tasks[ti];
        t.column = self.cfg.columns[ci].id.clone();
        t.order = order;
        true
    }

    /// Swap the card at position `pos` among the visible cards of column `ci` with the one `d`
    /// positions away; the column is renumbered 0.. first. None (nothing changed) when either
    /// position is outside the column. Returns the cards to write.
    pub fn reorder_card(&mut self, ci: usize, view: View, pos: usize, d: isize) -> Option<Vec<String>> {
        let vis = self.column_tasks(ci, view);
        let j = pos as isize + d;
        if pos >= vis.len() || j < 0 || j as usize >= vis.len() {
            return None;
        }
        let mut changed = vec![];
        for (n, &i) in vis.iter().enumerate() {
            if self.tasks[i].order != n as i64 {
                self.tasks[i].order = n as i64;
                changed.push(i);
            }
        }
        let (a, b) = (vis[pos], vis[j as usize]);
        let tmp = self.tasks[a].order;
        self.tasks[a].order = self.tasks[b].order;
        self.tasks[b].order = tmp;
        changed.extend([a, b]);
        changed.sort_unstable();
        changed.dedup();
        Some(changed.iter().map(|&i| self.tasks[i].id.clone()).collect())
    }

    /// Hide or show the card. False for an unknown card. Write: the card.
    pub fn toggle_card_hidden(&mut self, id: &str) -> bool {
        let Some(ti) = self.find(id) else { return false };
        self.tasks[ti].hidden = !self.tasks[ti].hidden;
        true
    }

    /// Put the card into the current sprint or take it out. False for an unknown card. Write: the card.
    pub fn toggle_card_sprint(&mut self, id: &str) -> bool {
        let Some(ti) = self.find(id) else { return false };
        let cur = Some(self.cfg.sprint);
        let t = &mut self.tasks[ti];
        t.sprint = if t.sprint == cur { None } else { cur };
        true
    }

    /// Soft delete: mark the card trashed now (the file stays). False for an unknown or already
    /// trashed card. Write: the card.
    pub fn trash_card(&mut self, id: &str) -> bool {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == id && !t.deleted) else { return false };
        t.deleted = true;
        t.deleted_at = Some(now_secs());
        true
    }

    /// Take the card out of the trash. False for an unknown card. Write: the card.
    pub fn restore_card(&mut self, id: &str) -> bool {
        let Some(ti) = self.find(id) else { return false };
        let t = &mut self.tasks[ti];
        t.deleted = false;
        t.deleted_at = None;
        true
    }

    /// Start the next sprint: unfinished (not in the last column) live cards of the current one
    /// move into it. Write: the returned cards and board.json.
    pub fn next_sprint(&mut self) -> Vec<String> {
        let (old, last) = (self.cfg.sprint, self.cfg.columns.len().saturating_sub(1));
        self.cfg.sprint = old + 1;
        let mut moved = vec![];
        for i in 0..self.tasks.len() {
            if !self.tasks[i].deleted && self.tasks[i].sprint == Some(old) && self.effective_col(&self.tasks[i]) != last {
                self.tasks[i].sprint = Some(old + 1);
                moved.push(self.tasks[i].id.clone());
            }
        }
        moved
    }

    // ----------------------------------------------------- column actions

    /// Swap column `i` with its neighbour `d` steps away; returns the new index. Pinned columns
    /// stay put, the others move only inside 1..=n-2. Write: board.json (cards are untouched).
    pub fn move_column_by(&mut self, i: usize, d: isize) -> Result<usize, String> {
        let n = self.cfg.columns.len();
        let Some(c) = self.cfg.columns.get(i) else { return Err("Колонки больше нет".into()) };
        let j = i as isize + d;
        // pinned columns stay put; the others move only inside 1..=n-2
        if self.is_pinned(i) || j < 1 || j as usize + 1 >= n {
            return Err(format!("Колонки «{}» нельзя двигать: первая и последняя закреплены", c.name));
        }
        let j = j as usize;
        self.cfg.columns.swap(i, j);
        Ok(j)
    }

    /// Delete a middle column; its cards (trashed ones too) go to the end of the first other
    /// column, and `next` pointers to it are cleared. Returns (former index, moved cards).
    /// Write: the moved cards and board.json.
    pub fn remove_column(&mut self, id: &str) -> Result<(usize, Vec<String>), String> {
        let Some(pos) = self.cfg.columns.iter().position(|c| c.id == id) else { return Err("Колонки больше нет".into()) };
        if self.cfg.columns.len() <= 1 {
            return Err("Нельзя удалить единственную колонку".into());
        }
        if self.is_pinned(pos) {
            let name = &self.cfg.columns[pos].name;
            return Err(format!("Колонку «{name}» нельзя удалить: первая и последняя закреплены"));
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
        Ok((pos, moved))
    }

    // -------------------------------------------------------------- apply

    /// Create (`edit` None: a `kind` card at the end of column `ci`) or edit (`edit` Some(id); the
    /// kind is then the stored one) a card, including the reverse edges of «Блокирует». Returns
    /// the card id and every card to write.
    pub fn apply_task(&mut self, kind: Kind, edit: Option<&str>, ci: usize, input: &TaskInput) -> Result<(String, Vec<String>), String> {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("Нужно указать название".into());
        }
        let (model, effort) = (input.model.clone(), input.effort.clone());
        let sprint = input.sprint;
        let parent = input.parent.clone().filter(|p| !p.is_empty());
        // same normalisation as `Task::from_md`, so an untouched description equals what was loaded
        let body = input.body.trim_start_matches('\n').trim_end().to_string();
        let depends = self.parse_depends(&input.depends, edit)?;
        // the id a new card is going to get; it exists in no card yet, so only the checks below see it
        let me = match edit {
            Some(id) => id.to_string(),
            None => self.new_id(kind),
        };
        let blocks = self.parse_blocks(&input.blocks, edit, &depends)?;
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
                let column = self.cfg.columns.get(ci).ok_or("Колонки больше нет")?.id.clone();
                let t = Task {
                    id: me.clone(),
                    kind,
                    title,
                    column,
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
        // the card exists now: apply the reverse edges to the dependents (written in the same commit)
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
        Ok((id, save))
    }

    /// Create (`edit` None: right before the pinned last column, a single-column board just
    /// appends; the id comes from the name) or edit (`edit` Some(id); the id never changes) a
    /// column. Returns the index of a new column. Write: board.json.
    pub fn apply_column(&mut self, edit: Option<&str>, input: &ColumnInput) -> Result<Option<usize>, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err("Нужно указать название".into());
        }
        let color = input.color.clone();
        let part = |s: &str| s.trim_end().to_string();
        let (before, rule, after) = (part(&input.before), part(&input.rule), part(&input.after));
        let agent = input.agent.trim().to_string();
        let next = input.next.clone();
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
                Ok(None)
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
                Ok(Some(at))
            }
        }
    }

    /// Set project rules: `values` are (rule id, option id) pairs, an empty option keeps the rule
    /// as it is; `custom` is the free Markdown text. Write: board.json.
    pub fn apply_rules(&mut self, values: &[(String, String)], custom: &str) {
        for r in &mut self.cfg.rules {
            if let Some((_, v)) = values.iter().find(|(id, v)| *id == r.id && !v.is_empty()) {
                r.value = v.clone();
            }
        }
        self.cfg.custom_rules = custom.trim_end().to_string();
    }

    /// Set the model and effort lists and the defaults for new cards. A default that is not in its
    /// list falls back to `default`; the returned notes name what was reset. Write: board.json.
    pub fn apply_models(&mut self, input: ModelsInput) -> Vec<&'static str> {
        let ModelsInput { models, efforts, default_model: dm, default_effort: de } = input;
        self.cfg.models = models;
        self.cfg.efforts = efforts;
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
        note
    }
}
