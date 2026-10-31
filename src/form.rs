//! Generic modal form: text fields and option selectors. No drawing here.
use crate::agents::{self, AgentRef};
use crate::cards::{self, CardRef};
use crate::editor::TextBuf;
use crate::files::FileIndex;
use crate::model::Kind;
use ratatui::layout::Rect;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::rc::Rc;

/// Most results the `@` picker shows.
pub const PICKER_ROWS: usize = 8;

/// Identity of the `@` token under the cursor: (field, row, column of `@`, query).
type TokenKey = (usize, usize, usize, String);

/// What a text field suggests while typing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Suggest {
    #[default]
    None,
    /// `@` file references (multiline fields).
    Files,
    /// Card ids (the dependency field).
    Cards,
    /// Subagent names (the column's executor field): the whole field is one value.
    Agents,
}

/// Selects with more options than this get type-to-filter.
pub const FILTER_MIN_OPTS: usize = 8;
/// Most subagents the popup lists (it scrolls past the screen rows).
pub const AGENT_ROWS: usize = 40;

/// Display text of one card suggestion (the inserted value is the matching `results` entry).
#[derive(Debug, PartialEq, Eq)]
pub struct CardRow {
    pub epic: bool,
    pub id: String,
    pub label: String,
    pub column: String,
}

/// Display text of one subagent suggestion.
#[derive(Debug, PartialEq, Eq)]
pub struct AgentRow {
    pub name: String,
    pub desc: String,
    /// Tools holding the agent, joined: `claude, codex`.
    pub tag: String,
}

/// Display text of one filtered select option.
#[derive(Debug, PartialEq, Eq)]
pub struct OptRow {
    pub label: String,
    pub hint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    Files,
    Cards,
    Agents,
    /// Type-to-filter popup of a select field (may have no rows).
    Select,
}

/// What the UI draws for the suggestion popup.
#[derive(Debug, PartialEq, Eq)]
pub struct PickerView {
    pub kind: PickKind,
    /// Values to insert: file paths, card ids, agent names or select option values.
    pub results: Vec<String>,
    pub selected: usize,
    pub query: String,
    /// Display rows for card suggestions (parallel to `results`); empty otherwise.
    pub cards: Vec<CardRow>,
    /// Display rows for agent suggestions (parallel to `results`); empty otherwise.
    pub agents: Vec<AgentRow>,
    /// Display rows for select options (parallel to `results`); empty otherwise.
    pub opts: Vec<OptRow>,
}

pub struct Opt {
    pub value: String,
    pub label: String,
    pub hint: String,
}

pub enum FieldKind {
    Text(TextBuf),
    Select { opts: Vec<Opt>, idx: usize },
}

pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// Drawn in the right-hand column when the form is wide enough.
    pub side: bool,
    pub suggest: Suggest,
}

impl Field {
    /// Mark the field as a side-column field.
    pub fn side(mut self) -> Self {
        self.side = true;
        self
    }

    pub fn suggest(mut self, s: Suggest) -> Self {
        self.suggest = s;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Purpose {
    NewTask(Kind),
    EditTask(String),
    NewColumn,
    EditColumn(String),
    Rules,
    Models,
}

pub struct Form {
    pub title: String,
    pub fields: Vec<Field>,
    pub focus: usize,
    pub purpose: Purpose,
    /// Project file index for the `@` picker; attached by the app once an `@` is typed.
    pub index: Option<Rc<FileIndex>>,
    /// Cards offered by `Suggest::Cards` fields (the card being edited is not in the list).
    pub cards: Rc<Vec<CardRef>>,
    /// Subagents offered by `Suggest::Agents` fields (scanned when the form opens).
    pub agents: Rc<Vec<AgentRef>>,
    /// Set by the UI each frame: true while the form is drawn in two columns.
    pub two_col: bool,
    /// Set by the UI each frame: screen rect of every field (same order as `fields`).
    pub rects: Vec<Rect>,
    pick_sel: usize,
    pick_seen: Option<TokenKey>,
    pick_dismissed: Option<TokenKey>,
    /// Up/Down was used in the current popup: only then does Tab pick from an empty-query popup.
    pick_touched: bool,
    /// Type-to-filter query of the focused select (empty: closed) and the field it belongs to.
    sel_q: String,
    sel_focus: usize,
    sel_pick: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FormEvent {
    None,
    Save,
    Cancel,
}

impl Form {
    pub fn new(title: impl Into<String>, purpose: Purpose, fields: Vec<Field>) -> Self {
        Form { title: title.into(), fields, focus: 0, purpose, index: None, cards: Rc::new(Vec::new()), agents: Rc::new(Vec::new()), two_col: false, rects: Vec::new(), pick_sel: 0, pick_seen: None, pick_dismissed: None, pick_touched: false, sel_q: String::new(), sel_focus: 0, sel_pick: 0 }
    }

    pub fn text(&self, key: &str) -> String {
        match self.fields.iter().find(|f| f.key == key).map(|f| &f.kind) {
            Some(FieldKind::Text(tb)) => tb.text(),
            _ => String::new(),
        }
    }

    pub fn sel(&self, key: &str) -> String {
        match self.fields.iter().find(|f| f.key == key).map(|f| &f.kind) {
            Some(FieldKind::Select { opts, idx }) => opts.get(*idx).map(|o| o.value.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Move focus by `d`, wrapping around.
    fn step(&mut self, d: isize) {
        let n = self.fields.len() as isize;
        if n > 0 {
            self.focus = (self.focus as isize + d).rem_euclid(n) as usize;
        }
    }

    /// Jump to the other column (two-column layout only): the field there at the closest
    /// vertical position. Returns false when nothing moved.
    fn jump_col(&mut self) -> bool {
        if !self.two_col {
            return false;
        }
        let Some(cur) = self.fields.get(self.focus) else { return false };
        let side = cur.side;
        let y = self.rects.get(self.focus).map_or(0, |r| r.y) as i32;
        let target = (0..self.fields.len())
            .filter(|&i| self.fields[i].side != side)
            .min_by_key(|&i| (self.rects.get(i).map_or(0, |r| r.y) as i32 - y).abs());
        match target {
            Some(i) => {
                self.focus = i;
                true
            }
            None => false,
        }
    }

    /// `@` file references work in multiline text fields, except in the models form.
    pub fn picker_enabled(&self) -> bool {
        self.purpose != Purpose::Models
    }

    /// What the focused field suggests; `Files` only in multiline fields outside the models form.
    pub fn focused_suggest(&self) -> Suggest {
        match self.fields.get(self.focus) {
            Some(Field { kind: FieldKind::Text(tb), suggest, .. }) => match suggest {
                Suggest::Files if self.picker_enabled() && tb.multiline => Suggest::Files,
                Suggest::Cards => Suggest::Cards,
                Suggest::Agents => Suggest::Agents,
                _ => Suggest::None,
            },
            _ => Suggest::None,
        }
    }

    /// True when the focused field accepts `@` references.
    pub fn focused_mentions(&self) -> bool {
        self.focused_suggest() == Suggest::Files
    }

    fn token_key(&self) -> Option<TokenKey> {
        let Some(Field { kind: FieldKind::Text(tb), .. }) = self.fields.get(self.focus) else { return None };
        let (start, query) = match self.focused_suggest() {
            Suggest::Files => tb.mention_token()?,
            Suggest::Cards => tb.list_token()?,
            // the whole field is the token
            Suggest::Agents => (0, tb.text()),
            Suggest::None => return None,
        };
        Some((self.focus, tb.row, start, query))
    }

    /// True when the cursor sits in an `@` token but no file index is attached yet.
    pub fn needs_index(&self) -> bool {
        self.index.is_none() && self.focused_mentions() && self.token_key().is_some()
    }

    /// Ids already listed in the focused field, outside the token being typed (lowercase).
    fn listed_ids(&self, token_start: usize) -> Vec<String> {
        let Some(Field { kind: FieldKind::Text(tb), .. }) = self.fields.get(self.focus) else { return Vec::new() };
        let line = &tb.lines[tb.row];
        let end = tb.col.min(line.len());
        let rest: String = line[..token_start.min(end)].iter().chain(line[end..].iter()).collect();
        let mut ids: Vec<String> =
            rest.split(|c: char| c == ',' || c.is_whitespace()).filter(|d| !d.is_empty()).map(str::to_lowercase).collect();
        // a card cannot sit in two card-list fields at once (depends / blocks): skip the other fields' ids too
        for (i, f) in self.fields.iter().enumerate() {
            if let (true, Suggest::Cards, FieldKind::Text(o)) = (i != self.focus, f.suggest, &f.kind) {
                let text = o.text();
                ids.extend(text.split(|c: char| c == ',' || c.is_whitespace()).filter(|d| !d.is_empty()).map(str::to_lowercase));
            }
        }
        ids
    }

    /// The type-to-filter popup of the focused select; None while the query is empty.
    fn select_view(&self) -> Option<PickerView> {
        if self.sel_q.is_empty() || self.sel_focus != self.focus {
            return None;
        }
        let Some(Field { kind: FieldKind::Select { opts, .. }, .. }) = self.fields.get(self.focus) else { return None };
        let hits = self.select_hits();
        let results = hits.iter().map(|&i| opts[i].value.clone()).collect::<Vec<_>>();
        let rows = hits.iter().map(|&i| OptRow { label: opts[i].label.clone(), hint: opts[i].hint.clone() }).collect();
        let selected = self.sel_pick.min(results.len().saturating_sub(1));
        Some(PickerView { kind: PickKind::Select, results, selected, query: self.sel_q.clone(), cards: Vec::new(), agents: Vec::new(), opts: rows })
    }

    /// True when the focused field is a select long enough for type-to-filter.
    pub fn focused_filterable(&self) -> bool {
        matches!(self.fields.get(self.focus), Some(Field { kind: FieldKind::Select { opts, .. }, .. }) if opts.len() > FILTER_MIN_OPTS)
    }

    /// Indices of the focused select's options matching the query (case-insensitive substring
    /// over label and hint).
    fn select_hits(&self) -> Vec<usize> {
        let Some(Field { kind: FieldKind::Select { opts, .. }, .. }) = self.fields.get(self.focus) else { return Vec::new() };
        let q = self.sel_q.to_lowercase();
        (0..opts.len()).filter(|&i| opts[i].label.to_lowercase().contains(&q) || opts[i].hint.to_lowercase().contains(&q)).collect()
    }

    /// The popup as the UI should draw it; None when inactive, dismissed or without results
    /// (a select filter without a match still shows, so the query stays visible).
    pub fn picker_view(&self) -> Option<PickerView> {
        if let Some(v) = self.select_view() {
            return Some(v);
        }
        let key = self.token_key()?;
        if self.pick_dismissed.as_ref() == Some(&key) {
            return None;
        }
        let (kind, results, cards, agent_rows) = match self.focused_suggest() {
            Suggest::Files => (PickKind::Files, self.index.as_ref()?.search(&key.3, PICKER_ROWS), Vec::new(), Vec::new()),
            Suggest::Cards => {
                let hits = cards::search(&self.cards, &key.3, &self.listed_ids(key.2), PICKER_ROWS);
                let rows = hits
                    .iter()
                    .map(|c| CardRow { epic: c.kind == Kind::Epic, id: c.id.clone(), label: c.title.clone(), column: c.column.clone() })
                    .collect();
                (PickKind::Cards, hits.iter().map(|c| c.id.clone()).collect(), rows, Vec::new())
            }
            Suggest::Agents => {
                let hits = agents::search(&self.agents, &key.3, AGENT_ROWS);
                // the value is already a complete name: nothing left to suggest
                if hits.len() == 1 && hits[0].name == key.3.trim() {
                    return None;
                }
                let rows = hits.iter().map(|a| AgentRow { name: a.name.clone(), desc: a.desc.clone(), tag: a.tag() }).collect();
                (PickKind::Agents, hits.iter().map(|a| a.name.clone()).collect(), Vec::new(), rows)
            }
            Suggest::None => return None,
        };
        if results.is_empty() {
            return None;
        }
        let selected = self.pick_sel.min(results.len() - 1);
        Some(PickerView { kind, results, selected, query: key.3, cards, agents: agent_rows, opts: Vec::new() })
    }

    /// Reset the selection when the token changes; forget a dismissal once its token is gone or edited.
    fn sync_picker(&mut self) {
        let key = self.token_key();
        if key != self.pick_seen {
            self.pick_sel = 0;
            self.pick_touched = false;
            self.pick_seen = key.clone();
        }
        if self.sel_focus != self.focus {
            self.sel_q.clear();
            self.sel_pick = 0;
        }
        if self.pick_dismissed != key {
            self.pick_dismissed = None;
        }
    }

    /// Picker keys, tried before the editor. Some(event) when the key was consumed.
    fn picker_key(&mut self, key: KeyEvent) -> Option<FormEvent> {
        let view = self.picker_view()?;
        if view.kind == PickKind::Select {
            return None; // `select_key` owns it
        }
        let n = view.results.len();
        let kind = view.kind;
        match key.code {
            KeyCode::Esc => self.pick_dismissed = self.token_key(),
            KeyCode::Up => {
                self.pick_touched = true;
                self.pick_sel = (view.selected + n - 1) % n;
            }
            KeyCode::Down => {
                self.pick_touched = true;
                self.pick_sel = (view.selected + 1) % n;
            }
            // an untouched popup of an empty agent field must not turn Tab-through into a pick
            KeyCode::Tab if kind == PickKind::Agents && view.query.trim().is_empty() && !self.pick_touched => return None,
            KeyCode::Enter | KeyCode::Tab => {
                let value = &view.results[view.selected];
                let start = self.token_key()?.2;
                if let Some(Field { kind: FieldKind::Text(tb), .. }) = self.fields.get_mut(self.focus) {
                    match kind {
                        PickKind::Cards => tb.replace_list_token(start, value),
                        PickKind::Agents => {
                            tb.lines = vec![value.chars().collect()];
                            tb.row = 0;
                            tb.col = tb.lines[0].len();
                        }
                        _ => {
                            let text = if value.ends_with('/') { format!("@{value}") } else { format!("@{value} ") };
                            tb.replace_mention(start, &text);
                        }
                    }
                }
                if kind == PickKind::Agents {
                    self.pick_dismissed = self.token_key();
                }
            }
            _ => return None,
        }
        Some(FormEvent::None)
    }

    /// Type-to-filter keys of a long select, tried before the picker. Some when consumed.
    fn select_key(&mut self, key: KeyEvent) -> Option<FormEvent> {
        if !self.focused_filterable() {
            return None;
        }
        if self.sel_focus != self.focus {
            self.sel_q.clear();
            self.sel_pick = 0;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let typed = ctrl == alt; // plain or Shift; AltGr arrives as Ctrl+Alt
        let open = !self.sel_q.is_empty();
        match key.code {
            // Space keeps cycling until a query is started
            KeyCode::Char(c) if typed && (open || c != ' ') => {
                self.sel_focus = self.focus;
                self.sel_q.push(c);
                self.sel_pick = 0;
            }
            KeyCode::Backspace if open => {
                self.sel_q.pop();
                self.sel_pick = 0;
            }
            KeyCode::Esc if open => self.sel_q.clear(),
            KeyCode::Up | KeyCode::Down if open => {
                let n = self.select_hits().len();
                if n > 0 {
                    let cur = self.sel_pick.min(n - 1);
                    self.sel_pick = if key.code == KeyCode::Up { (cur + n - 1) % n } else { (cur + 1) % n };
                }
            }
            KeyCode::Enter | KeyCode::Tab if open => {
                let hits = self.select_hits();
                let at = self.sel_pick.min(hits.len().saturating_sub(1));
                self.sel_q.clear();
                self.sel_pick = 0;
                // no match: the key navigates as usual
                let &pick = hits.get(at)?;
                if let Some(Field { kind: FieldKind::Select { idx, .. }, .. }) = self.fields.get_mut(self.focus) {
                    *idx = pick;
                }
            }
            KeyCode::Left | KeyCode::Right if open => {
                self.sel_q.clear();
                return None; // then cycles as usual
            }
            _ => return None,
        }
        Some(FormEvent::None)
    }

    pub fn handle(&mut self, key: KeyEvent) -> FormEvent {
        self.sync_picker(); // focus may have moved (mouse) since the last key
        let ev = match self.select_key(key).or_else(|| self.picker_key(key)) {
            Some(ev) => ev,
            None => self.handle_key(key),
        };
        self.sync_picker();
        ev
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return FormEvent::Cancel,
            KeyCode::Char('c') if ctrl => return FormEvent::Cancel,
            KeyCode::Char('s') if ctrl => return FormEvent::Save,
            KeyCode::Tab => {
                self.step(1);
                return FormEvent::None;
            }
            KeyCode::BackTab => {
                self.step(-1);
                return FormEvent::None;
            }
            KeyCode::Left | KeyCode::Right if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.jump_col();
                return FormEvent::None;
            }
            KeyCode::Left | KeyCode::Right if self.two_col => {
                // plain arrows change column only when they would otherwise do nothing
                let at_edge = match self.fields.get(self.focus).map(|f| &f.kind) {
                    Some(FieldKind::Text(tb)) if key.code == KeyCode::Left => tb.row == 0 && tb.col == 0,
                    Some(FieldKind::Text(tb)) => tb.row + 1 == tb.lines.len() && tb.col >= tb.lines[tb.row].len(),
                    _ => false,
                };
                if at_edge {
                    self.jump_col();
                    return FormEvent::None;
                }
            }
            _ => {}
        }
        let Some(field) = self.fields.get_mut(self.focus) else { return FormEvent::None };
        let mut nav = 0isize;
        match &mut field.kind {
            FieldKind::Text(tb) => {
                if !tb.handle(key) {
                    match key.code {
                        KeyCode::Up => nav = -1,
                        KeyCode::Down | KeyCode::Enter => nav = 1,
                        _ => {}
                    }
                }
            }
            FieldKind::Select { opts, idx } => {
                let n = opts.len();
                match key.code {
                    KeyCode::Left if n > 0 => *idx = (*idx + n - 1) % n,
                    KeyCode::Right | KeyCode::Char(' ') if n > 0 => *idx = (*idx + 1) % n,
                    KeyCode::Up => nav = -1,
                    KeyCode::Down | KeyCode::Enter => nav = 1,
                    _ => {}
                }
            }
        }
        if nav != 0 {
            self.step(nav);
        }
        FormEvent::None
    }

    pub fn paste(&mut self, s: &str) {
        if let Some(Field { kind: FieldKind::Text(tb), .. }) = self.fields.get_mut(self.focus) {
            tb.insert_str(s);
        }
        self.sync_picker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn sample() -> Form {
        let opts = ["a", "b", "c"].iter().map(|v| Opt { value: v.to_string(), label: v.to_string(), hint: String::new() }).collect();
        Form::new("t", Purpose::NewColumn, vec![
            Field { key: "name".into(), label: "Name".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: false, suggest: Suggest::None },
            Field { key: "pick".into(), label: "Pick".into(), kind: FieldKind::Select { opts, idx: 0 }, side: false, suggest: Suggest::None },
        ])
    }

    #[test]
    fn navigation_and_select() {
        let mut f = sample();
        f.handle(key(KeyCode::Char('x')));
        assert_eq!(f.text("name"), "x");
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.focus, 1);
        f.handle(key(KeyCode::Left));
        assert_eq!(f.sel("pick"), "c");
        f.handle(key(KeyCode::Right));
        assert_eq!(f.sel("pick"), "a");
        f.handle(key(KeyCode::Char(' ')));
        assert_eq!(f.sel("pick"), "b");
        f.handle(key(KeyCode::Tab));
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::BackTab));
        assert_eq!(f.focus, 1);
        f.paste("zz");
        assert_eq!(f.text("name"), "x");
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::Cancel);
        assert_eq!(f.handle(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)), FormEvent::Save);
        assert_eq!(f.text("nope"), "");
        assert_eq!(f.sel("nope"), "");
    }

    fn picker_form(purpose: Purpose) -> Form {
        let mut f = Form::new("t", purpose, vec![
            Field { key: "name".into(), label: "Name".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: false, suggest: Suggest::None },
            Field { key: "body".into(), label: "Body".into(), kind: FieldKind::Text(TextBuf::new("", true)), side: false, suggest: Suggest::Files },
        ]);
        f.index = Some(Rc::new(FileIndex {
            root: std::path::PathBuf::new(),
            paths: ["src/", "README.md", "src/ui.rs", "src/model.rs"].iter().map(|s| s.to_string()).collect(),
        }));
        f.focus = 1;
        f
    }

    fn type_str(f: &mut Form, s: &str) {
        for c in s.chars() {
            f.handle(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn picker_select_inserts_path() {
        let mut f = picker_form(Purpose::NewColumn);
        type_str(&mut f, "see ");
        assert!(f.picker_view().is_none());
        f.handle(key(KeyCode::Char('@')));
        let v = f.picker_view().unwrap();
        assert_eq!(v.results, ["src/", "README.md", "src/ui.rs", "src/model.rs"]);
        assert_eq!(v.selected, 0);
        type_str(&mut f, "ui");
        let v = f.picker_view().unwrap();
        assert_eq!((v.results[0].as_str(), v.selected, v.query.as_str()), ("src/ui.rs", 0, "ui"));
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!(f.text("body"), "see @src/ui.rs ");
        assert!(f.picker_view().is_none());
        // Tab also selects; directories get no trailing space and keep the picker open for drilling down
        type_str(&mut f, "@src");
        assert_eq!(f.picker_view().unwrap().results[0], "src/");
        assert_eq!(f.handle(key(KeyCode::Tab)), FormEvent::None);
        assert_eq!(f.focus, 1);
        assert_eq!(f.text("body"), "see @src/ui.rs @src/");
        let v = f.picker_view().unwrap();
        assert_eq!(v.results, ["src/", "src/ui.rs", "src/model.rs"]);
    }

    #[test]
    fn picker_navigation_wraps() {
        let mut f = picker_form(Purpose::NewColumn);
        f.handle(key(KeyCode::Char('@')));
        f.handle(key(KeyCode::Up));
        assert_eq!(f.picker_view().unwrap().selected, 3);
        f.handle(key(KeyCode::Down));
        assert_eq!(f.picker_view().unwrap().selected, 0);
        f.handle(key(KeyCode::Down));
        f.handle(key(KeyCode::Down));
        assert_eq!(f.picker_view().unwrap().selected, 2);
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.text("body"), "@src/ui.rs ");
        // typing resets the selection
        f.handle(key(KeyCode::Char('@')));
        f.handle(key(KeyCode::Down));
        f.handle(key(KeyCode::Char('s')));
        assert_eq!(f.picker_view().unwrap().selected, 0);
    }

    #[test]
    fn picker_esc_closes_only_popup() {
        let mut f = picker_form(Purpose::NewColumn);
        type_str(&mut f, "@r");
        assert!(f.picker_view().is_some());
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::None);
        assert!(f.picker_view().is_none());
        assert_eq!(f.text("body"), "@r");
        // second Esc cancels the form
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::Cancel);
        // editing the token reopens it
        f.handle(key(KeyCode::Char('e')));
        assert!(f.picker_view().is_some());
        // Backspace over the `@` closes it
        for _ in 0..3 {
            f.handle(key(KeyCode::Backspace));
        }
        assert!(f.picker_view().is_none());
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::Cancel);
    }

    #[test]
    fn picker_scope() {
        // not in single-line fields
        let mut f = picker_form(Purpose::NewColumn);
        f.focus = 0;
        f.handle(key(KeyCode::Char('@')));
        assert!(f.picker_view().is_none() && !f.focused_mentions());
        // not in the models form
        let mut f = picker_form(Purpose::Models);
        f.handle(key(KeyCode::Char('@')));
        assert!(f.picker_view().is_none() && !f.needs_index());
        assert_eq!(f.text("body"), "@");
        // mid-word `@` and no matches do not open it
        let mut f = picker_form(Purpose::NewColumn);
        type_str(&mut f, "a@");
        assert!(f.picker_view().is_none());
        type_str(&mut f, " @zzz");
        assert!(f.picker_view().is_none());
        // without an index it asks for one
        let mut f = picker_form(Purpose::NewColumn);
        f.index = None;
        f.handle(key(KeyCode::Char('@')));
        assert!(f.needs_index() && f.picker_view().is_none());
        // Enter without a popup is the normal newline
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.text("body"), "@\n");
    }

    #[test]
    fn focus_wraps_both_ways() {
        let mut f = sample();
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::Up)); // single-line text: Up not consumed
        assert_eq!(f.focus, 1);
        f.handle(key(KeyCode::Down)); // select: Down on the last field
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::BackTab));
        assert_eq!(f.focus, 1);
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::Tab));
        assert_eq!(f.focus, 1);
        // multiline: Up on the top row of the first field wraps, Down on the bottom row of the last wraps
        let mut f = picker_form(Purpose::NewColumn);
        f.focus = 1;
        type_str(&mut f, "ab");
        f.handle(key(KeyCode::Up));
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::Up));
        assert_eq!(f.focus, 1);
        f.handle(key(KeyCode::Down));
        assert_eq!(f.focus, 0);
    }

    #[test]
    fn picker_keys_win_over_wrap() {
        let mut f = picker_form(Purpose::NewColumn);
        f.handle(key(KeyCode::Char('@')));
        assert!(f.picker_view().is_some());
        f.handle(key(KeyCode::Down)); // moves the picker selection, not the focus
        assert_eq!(f.focus, 1);
        assert_eq!(f.picker_view().unwrap().selected, 1);
    }

    fn cards_form() -> Form {
        let mut f = Form::new("t", Purpose::NewTask(Kind::Task), vec![
            Field { key: "title".into(), label: "T".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: false, suggest: Suggest::None },
            Field { key: "depends".into(), label: "D".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: true, suggest: Suggest::Cards },
        ]);
        f.cards = Rc::new(vec![
            CardRef::new("E-001", Kind::Epic, "Авторизация", "В работе", ""),
            CardRef::new("T-001", Kind::Task, "Форма входа", "В работе", "токены"),
            CardRef::new("T-002", Kind::Task, "Refactor parser", "Бэклог", ""),
            CardRef::new("T-005", Kind::Task, "Ship it", "Готово", "parser release"),
        ]);
        f.focus = 1;
        f
    }

    #[test]
    fn cards_popup_matching_and_insert() {
        let mut f = cards_form();
        // empty token: no popup, Enter just moves focus on
        assert!(f.picker_view().is_none());
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!(f.focus, 0);
        f.focus = 1;
        // id prefix beats body match; rows carry display text
        type_str(&mut f, "T-0");
        let v = f.picker_view().unwrap();
        assert_eq!(v.results, ["T-001", "T-002", "T-005"]);
        assert_eq!((v.cards[1].label.as_str(), v.cards[1].column.as_str(), v.cards[1].epic), ("Refactor parser", "Бэклог", false));
        for _ in 0..3 {
            f.handle(key(KeyCode::Backspace));
        }
        type_str(&mut f, "parser");
        assert_eq!(f.picker_view().unwrap().results, ["T-002", "T-005"]); // title word beats body
        f.handle(key(KeyCode::Down));
        f.handle(key(KeyCode::Up));
        assert_eq!(f.picker_view().unwrap().selected, 0);
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!((f.text("depends").as_str(), f.focus), ("T-002, ", 1));
        assert!(f.picker_view().is_none());
        // second token: already-listed T-002 is excluded; Down + Tab picks the second hit
        type_str(&mut f, "T-0");
        assert_eq!(f.picker_view().unwrap().results, ["T-001", "T-005"]);
        f.handle(key(KeyCode::Down));
        assert_eq!(f.handle(key(KeyCode::Tab)), FormEvent::None);
        assert_eq!(f.text("depends"), "T-002, T-005, ");
        assert_eq!(f.focus, 1);
        // Cyrillic title search; epic is flagged
        type_str(&mut f, "АВТОР");
        let v = f.picker_view().unwrap();
        assert_eq!((v.results[0].as_str(), v.cards[0].epic), ("E-001", true));
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.text("depends"), "T-002, T-005, E-001, ");
        // T-005 is already listed: its column name finds nothing
        type_str(&mut f, "готово");
        assert!(f.picker_view().is_none());
    }

    #[test]
    fn cards_fields_skip_each_others_ids() {
        let mut f = cards_form();
        f.fields.push(Field { key: "blocks".into(), label: "B".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: true, suggest: Suggest::Cards });
        // depends has T-002; blocks must not offer it, and a fresh depends token must not offer blocks' T-005
        f.focus = 1;
        type_str(&mut f, "T-002, ");
        f.focus = 2;
        type_str(&mut f, "T-005, T-0");
        assert_eq!(f.picker_view().unwrap().results, ["T-001"]);
        f.focus = 1;
        f.handle(key(KeyCode::End));
        type_str(&mut f, "T-0");
        assert_eq!(f.picker_view().unwrap().results, ["T-001"]);
    }

    #[test]
    fn cards_popup_esc_and_scope() {
        let mut f = cards_form();
        type_str(&mut f, "T-00");
        assert!(f.picker_view().is_some());
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::None);
        assert!(f.picker_view().is_none());
        assert_eq!(f.text("depends"), "T-00");
        // typing reopens it; Backspace refines
        f.handle(key(KeyCode::Char('1')));
        assert_eq!(f.picker_view().unwrap().results, ["T-001"]);
        f.handle(key(KeyCode::Backspace));
        assert_eq!(f.picker_view().unwrap().results.len(), 3);
        // no match: no popup, Enter moves focus
        type_str(&mut f, "9");
        assert!(f.picker_view().is_none());
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!(f.focus, 0);
        // a plain text field never suggests
        f.handle(key(KeyCode::Char('T')));
        assert!(f.picker_view().is_none() && !f.needs_index());
        f.focus = 1;
        assert!(!f.focused_mentions());
        // a separator right before the token starts a fresh token
        let mut f = cards_form();
        type_str(&mut f, "T-001,ship");
        assert_eq!(f.picker_view().unwrap().results, ["T-005"]);
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.text("depends"), "T-001,T-005, ");
    }

    // ---------------------------------------------------------------- agents

    fn agents_form(with_agents: bool) -> Form {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("pumba-form-agents-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        if with_agents {
            std::fs::create_dir_all(root.join(".claude/agents")).unwrap();
            std::fs::create_dir_all(root.join(".codex/agents")).unwrap();
            std::fs::write(root.join(".claude/agents/pumba-worker.md"), "---\ndescription: Исполнитель карточки\n---\n").unwrap();
            std::fs::write(root.join(".claude/agents/pumba-planner.md"), "---\ndescription: Планировщик\n---\n").unwrap();
            std::fs::write(root.join(".claude/agents/alpha.md"), "---\ndescription: mine\n---\n").unwrap();
            std::fs::write(root.join(".codex/agents/pumba-worker.toml"), "name = \"pumba_worker\"\ndescription = \"codex\"\n").unwrap();
        }
        let opts = ["a", "b"].iter().map(|v| Opt { value: v.to_string(), label: v.to_string(), hint: String::new() }).collect();
        let mut f = Form::new("t", Purpose::NewColumn, vec![
            Field { key: "name".into(), label: "N".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: false, suggest: Suggest::None },
            Field { key: "agent".into(), label: "A".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: true, suggest: Suggest::Agents },
            Field { key: "next".into(), label: "X".into(), kind: FieldKind::Select { opts, idx: 0 }, side: true, suggest: Suggest::None },
        ]);
        f.agents = Rc::new(crate::agents::scan(&root, None));
        let _ = std::fs::remove_dir_all(&root);
        f.focus = 1;
        f
    }

    #[test]
    fn agents_popup_opens_on_focus_and_lists_all() {
        let mut f = agents_form(true);
        let v = f.picker_view().expect("opens on focus with an empty field");
        assert_eq!((v.kind, v.selected), (PickKind::Agents, 0));
        // pack first, then the rest; display rows carry description and tools
        assert_eq!(v.results, ["pumba-planner", "pumba-worker", "alpha"]);
        assert_eq!((v.agents[1].desc.as_str(), v.agents[1].tag.as_str()), ("Исполнитель карточки", "claude, codex"));
        assert!(f.focused_suggest() == Suggest::Agents && !f.focused_mentions() && !f.needs_index());
        // focus elsewhere: no popup; coming back opens it again
        f.handle(key(KeyCode::BackTab));
        assert_eq!(f.focus, 0);
        assert!(f.picker_view().is_none());
        f.handle(key(KeyCode::Tab));
        assert!(f.picker_view().is_some());
    }

    #[test]
    fn agents_select_inserts_name_only() {
        let mut f = agents_form(true);
        f.handle(key(KeyCode::Down));
        assert_eq!(f.picker_view().unwrap().selected, 1);
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!((f.text("agent").as_str(), f.focus), ("pumba-worker", 1));
        assert!(f.picker_view().is_none(), "popup closes after a pick");
        // the next Enter is plain navigation
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.focus, 2);
        // typing narrows by name, a pick replaces the whole value (no commas, no leftovers)
        f.focus = 1;
        f.handle(key(KeyCode::End));
        for _ in 0..12 {
            f.handle(key(KeyCode::Backspace));
        }
        type_str(&mut f, "AL");
        let v = f.picker_view().unwrap();
        assert_eq!((v.results.as_slice(), v.query.as_str()), (["alpha".to_string()].as_slice(), "AL"));
        f.handle(key(KeyCode::Tab));
        assert_eq!((f.text("agent").as_str(), f.focus), ("alpha", 1));
        // a pick over free text replaces it
        let mut f = agents_form(true);
        type_str(&mut f, "plan");
        assert_eq!(f.picker_view().unwrap().results, ["pumba-planner"]);
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.text("agent"), "pumba-planner");
        // description words match too; unknown text shows nothing and stays as typed
        let mut f = agents_form(true);
        type_str(&mut f, "планиров");
        assert_eq!(f.picker_view().unwrap().results, ["pumba-planner"]);
        let mut f = agents_form(true);
        type_str(&mut f, "my-own-agent");
        assert!(f.picker_view().is_none());
        f.handle(key(KeyCode::Enter));
        assert_eq!((f.text("agent").as_str(), f.focus), ("my-own-agent", 2));
    }

    #[test]
    fn agents_esc_closes_only_popup_and_tab_through_is_safe() {
        let mut f = agents_form(true);
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::None);
        assert!(f.picker_view().is_none());
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::Cancel);
        // typing reopens it
        f.handle(key(KeyCode::Char('u')));
        assert_eq!(f.picker_view().unwrap().results, ["pumba-planner", "pumba-worker"]);
        // Tab on an untouched empty popup just moves on and picks nothing
        let mut f = agents_form(true);
        assert_eq!(f.handle(key(KeyCode::Tab)), FormEvent::None);
        assert_eq!((f.text("agent").as_str(), f.focus), ("", 2));
        // once the user steered the popup with Up/Down, Tab picks
        let mut f = agents_form(true);
        f.handle(key(KeyCode::Up));
        assert_eq!(f.picker_view().unwrap().selected, 2);
        f.handle(key(KeyCode::Tab));
        assert_eq!((f.text("agent").as_str(), f.focus), ("alpha", 1));
        // a complete name that is the only match needs no popup: Tab moves on
        assert!(f.picker_view().is_none());
        f.handle(key(KeyCode::Tab));
        assert_eq!(f.focus, 2);
    }

    #[test]
    fn agents_none_found_means_no_popup() {
        let mut f = agents_form(false);
        assert!(f.picker_view().is_none());
        type_str(&mut f, "x");
        assert!(f.picker_view().is_none());
        f.handle(key(KeyCode::Down)); // plain navigation
        assert_eq!(f.focus, 2);
    }

    // ------------------------------------------------------- select filter

    fn epic_form(n: usize) -> Form {
        let mut opts = vec![Opt { value: String::new(), label: "нет".into(), hint: String::new() }];
        let titles = ["Авторизация", "Платежи", "Отчёты", "Миграция БД", "Поиск", "Уведомления", "Админка", "Экспорт", "Импорт", "Мобильный клиент", "Платёжный шлюз", "Документация"];
        for i in 0..n {
            opts.push(Opt { value: format!("E-{:03}", i + 1), label: format!("E-{:03} {}", i + 1, titles[i]), hint: if i == 4 { "полнотекстовый".into() } else { String::new() } });
        }
        Form::new("t", Purpose::NewTask(Kind::Task), vec![
            Field { key: "title".into(), label: "T".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: false, suggest: Suggest::None },
            Field { key: "parent".into(), label: "Эпик".into(), kind: FieldKind::Select { opts, idx: 0 }, side: true, suggest: Suggest::None },
            Field { key: "other".into(), label: "O".into(), kind: FieldKind::Text(TextBuf::new("", false)), side: true, suggest: Suggest::None },
        ])
    }

    fn focus_epic(f: &mut Form) {
        f.handle(key(KeyCode::Tab));
        assert_eq!(f.focus, 1);
    }

    #[test]
    fn select_type_to_filter_pick() {
        let mut f = epic_form(12);
        focus_epic(&mut f);
        assert!(f.focused_filterable() && f.picker_view().is_none());
        // typing opens the popup; case-insensitive substring over the label
        type_str(&mut f, "ПЛАТ");
        let v = f.picker_view().unwrap();
        assert_eq!(v.kind, PickKind::Select);
        assert_eq!(v.results, ["E-002", "E-011"]);
        assert_eq!((v.opts[0].label.as_str(), v.query.as_str()), ("E-002 Платежи", "ПЛАТ"));
        // Down/Up choose in the popup (wrapping), focus stays
        f.handle(key(KeyCode::Down));
        assert_eq!((f.picker_view().unwrap().selected, f.focus), (1, 1));
        f.handle(key(KeyCode::Down));
        assert_eq!(f.picker_view().unwrap().selected, 0);
        f.handle(key(KeyCode::Up));
        assert_eq!(f.picker_view().unwrap().selected, 1);
        // Backspace edits the query
        f.handle(key(KeyCode::Backspace));
        f.handle(key(KeyCode::Backspace));
        assert_eq!(f.picker_view().unwrap().query, "ПЛ");
        type_str(&mut f, "ат");
        assert_eq!(f.picker_view().unwrap().results, ["E-002", "E-011"]);
        // Enter picks and closes; focus stays, value is set
        assert_eq!(f.handle(key(KeyCode::Enter)), FormEvent::None);
        assert_eq!((f.sel("parent").as_str(), f.focus), ("E-002", 1));
        assert!(f.picker_view().is_none());
        // Enter now is plain navigation
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.focus, 2);
        // Tab picks too; the hint is searched; the id matches
        f.focus = 1;
        type_str(&mut f, "полнотекст");
        assert_eq!(f.picker_view().unwrap().results, ["E-005"]);
        f.handle(key(KeyCode::Tab));
        assert_eq!((f.sel("parent").as_str(), f.focus), ("E-005", 1));
        type_str(&mut f, "e-012");
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.sel("parent"), "E-012");
        // «нет» is reachable by its label
        type_str(&mut f, "нет");
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.sel("parent"), "");
    }

    #[test]
    fn select_filter_esc_cycling_and_navigation() {
        let mut f = epic_form(12);
        focus_epic(&mut f);
        // without a query Space and arrows cycle exactly as before; Up/Down move focus
        f.handle(key(KeyCode::Right));
        assert_eq!(f.sel("parent"), "E-001");
        f.handle(key(KeyCode::Char(' ')));
        assert_eq!(f.sel("parent"), "E-002");
        f.handle(key(KeyCode::Left));
        assert_eq!(f.sel("parent"), "E-001");
        f.handle(key(KeyCode::Up));
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::Down));
        assert_eq!(f.focus, 1);
        // Esc closes only the popup (value untouched), the next Esc cancels the form
        type_str(&mut f, "мигр");
        assert!(f.picker_view().is_some());
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::None);
        assert!(f.picker_view().is_none());
        assert_eq!(f.sel("parent"), "E-001");
        assert_eq!(f.handle(key(KeyCode::Esc)), FormEvent::Cancel);
        // Space is part of the query once one is started
        type_str(&mut f, "мобильный к");
        assert_eq!(f.picker_view().unwrap().results, ["E-010"]);
        // no match: popup stays (shows the query), Enter navigates on and drops the query
        type_str(&mut f, "zz");
        let v = f.picker_view().unwrap();
        assert!(v.results.is_empty() && v.query == "мобильный кzz");
        f.handle(key(KeyCode::Down)); // nothing to choose: no panic, no focus change
        assert_eq!(f.focus, 1);
        f.handle(key(KeyCode::Enter));
        assert_eq!(f.focus, 2);
        assert!(f.picker_view().is_none());
        // Left/Right drop the query and cycle
        f.focus = 1;
        type_str(&mut f, "эксп");
        f.handle(key(KeyCode::Right));
        assert!(f.picker_view().is_none());
        assert_eq!(f.sel("parent"), "E-002");
        // moving focus away drops the query
        type_str(&mut f, "поиск");
        f.handle(key(KeyCode::BackTab));
        assert_eq!(f.focus, 0);
        f.handle(key(KeyCode::Tab));
        assert!(f.picker_view().is_none());
    }

    #[test]
    fn select_filter_keeps_modifier_keys() {
        let mut f = epic_form(12);
        focus_epic(&mut f);
        // Ctrl+S saves, Ctrl+C cancels: never typed into the query
        assert_eq!(f.handle(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)), FormEvent::Save);
        assert_eq!(f.handle(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)), FormEvent::Cancel);
        assert!(f.picker_view().is_none());
        // Alt/Ctrl arrows still jump columns (two-column layout), with or without a query
        f.two_col = true;
        f.rects = vec![Rect::new(0, 0, 10, 3), Rect::new(20, 0, 10, 1), Rect::new(20, 1, 10, 3)];
        type_str(&mut f, "пла");
        f.handle(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        assert_eq!(f.focus, 0);
        f.handle(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));
        assert_eq!(f.focus, 1);
        assert!(f.picker_view().is_none(), "query is dropped when focus moved");
        // BackTab is plain navigation
        type_str(&mut f, "пла");
        f.handle(key(KeyCode::BackTab));
        assert_eq!(f.focus, 0);
        // short selects never filter: letters do nothing, Space still cycles
        let mut g = sample();
        g.focus = 1;
        g.handle(key(KeyCode::Char('b')));
        assert!(g.picker_view().is_none() && !g.focused_filterable());
        g.handle(key(KeyCode::Char(' ')));
        assert_eq!(g.sel("pick"), "b");
        // «нет» + 7 epics = 8 options: still short; one more and it filters
        let mut e7 = epic_form(7);
        e7.focus = 1;
        assert!(!e7.focused_filterable());
        let mut e8 = epic_form(8);
        e8.focus = 1;
        assert!(e8.focused_filterable());
    }
}
