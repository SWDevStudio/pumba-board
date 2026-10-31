//! Column screens: move and delete keys, the column form.
use super::App;
use super::forms::{opt, sel_field, text_field};
use crate::form::{Form, Purpose, Suggest};
use crate::theme;
use pumba_core::board::ColumnInput;
use pumba_install::agents;
use std::rc::Rc;

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

impl App {
    // ----------------------------------------------------- column actions

    pub(super) fn move_column(&mut self, d: isize) {
        let i = self.sel_col;
        // no column under the cursor: nothing to say
        if i >= self.cfg.columns.len() {
            return;
        }
        match self.board.move_column_by(i, d) {
            Ok(j) => {
                self.scrolls.swap(i, j);
                if self.save(&[], true) {
                    self.sel_col = j;
                }
            }
            Err(e) => self.err(e),
        }
    }

    pub(super) fn delete_column(&mut self, id: &str) {
        if !self.cfg.columns.iter().any(|c| c.id == id) {
            return;
        }
        let (pos, moved) = match self.board.remove_column(id) {
            Ok(r) => r,
            Err(e) => return self.err(e),
        };
        self.scrolls.resize(self.cfg.columns.len(), 0);
        if self.save(&moved, true) {
            self.sel_col = pos.saturating_sub(1).min(self.cfg.columns.len() - 1);
            self.sel_idx = 0;
        }
    }

    pub(super) fn open_column_form(&mut self, edit: Option<usize>) {
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

    pub(super) fn apply_column(&mut self, f: &Form, edit: Option<&str>) -> Result<(), String> {
        let input = ColumnInput {
            name: f.text("name"),
            before: f.text("before"),
            rule: f.text("rule"),
            after: f.text("after"),
            color: f.sel("color"),
            agent: f.text("agent"),
            next: f.sel("next"),
        };
        match self.board.apply_column(edit, &input)? {
            None => self.commit(&[], true),
            Some(at) => {
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
