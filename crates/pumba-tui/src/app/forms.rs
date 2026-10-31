//! Card, rules and models forms: opening them prefilled, keys, and turning a saved form into
//! board input (`TaskInput`, `ModelsInput`, ...) applied by the core.
use super::{App, Mode};
use crate::cards::CardRef;
use crate::editor::TextBuf;
use crate::files::FileIndex;
use crate::form::{Field, FieldKind, Form, FormEvent, Opt, Purpose, Suggest};
use pumba_core::board::{ModelsInput, TaskInput};
use pumba_core::model::{Kind, Task};
use ratatui::crossterm::event::KeyEvent;
use std::rc::Rc;

// ------------------------------------------------------------ form helpers

pub(super) fn opt(value: &str, label: &str, hint: &str) -> Opt {
    Opt { value: value.into(), label: label.into(), hint: hint.into() }
}

pub(super) fn text_field(key: &str, label: &str, val: &str, multiline: bool) -> Field {
    let suggest = if multiline { Suggest::Files } else { Suggest::None };
    Field { key: key.into(), label: label.into(), kind: FieldKind::Text(TextBuf::new(val, multiline)), side: false, suggest }
}

/// Select field preselecting `cur`; an unknown non-empty `cur` is appended so it is not lost.
pub(super) fn sel_field(key: &str, label: &str, mut opts: Vec<Opt>, cur: &str) -> Field {
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

impl App {
    /// Show a form. The file index is dropped so the next `@` sees files created meanwhile.
    pub(super) fn open_form(&mut self, form: Form) {
        self.files = None;
        self.mode = Mode::Form(Box::new(form));
    }

    /// Give the form the file index as soon as its cursor sits in an `@` token (built lazily).
    pub(super) fn attach_index(&mut self, f: &mut Form) {
        if f.needs_index() {
            let root = self.root();
            let ix = self.files.get_or_insert_with(|| Rc::new(FileIndex::build(&root)));
            f.index = Some(ix.clone());
        }
    }

    // -------------------------------------------------------------- forms

    pub(super) fn open_task_form(&mut self, kind: Kind, edit: Option<usize>) {
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

    pub(super) fn open_rules_form(&mut self) {
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

    pub(super) fn open_models_form(&mut self) {
        // the defaults come first (Tab order); their options are the lists as they are now
        let fields = vec![
            sel_field("default_model", "Модель по умолчанию для новых карточек", plain_opts("default", &self.cfg.models), &self.cfg.default_model),
            sel_field("default_effort", "Уровень по умолчанию для новых карточек", plain_opts("default", &self.cfg.efforts), &self.cfg.default_effort),
            text_field("models", "Модели (по одной в строке)", &self.cfg.models.join("\n"), true),
            text_field("efforts", "Уровни размышления (по одному в строке)", &self.cfg.efforts.join("\n"), true),
        ];
        self.open_form(Form::new("Модели и уровни", Purpose::Models, fields));
    }

    pub(super) fn form_key(&mut self, mut f: Box<Form>, key: KeyEvent) {
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
    pub(super) fn apply_form(&mut self, f: &Form) -> Result<(), String> {
        match &f.purpose {
            Purpose::NewTask(kind) => self.apply_task(f, *kind, None),
            Purpose::EditTask(id) => self.apply_task(f, Kind::Task, Some(id)),
            Purpose::NewColumn => self.apply_column(f, None),
            Purpose::EditColumn(id) => self.apply_column(f, Some(id)),
            Purpose::Rules => {
                let values: Vec<(String, String)> = self.cfg.rules.iter().map(|r| (r.id.clone(), f.sel(&r.id))).collect();
                self.board.apply_rules(&values, &f.text("custom"));
                self.commit(&[], true)
            }
            Purpose::Models => {
                // a default that left the list in the same form falls back to `default`
                let note = self.board.apply_models(ModelsInput {
                    models: parse_list(&f.text("models")),
                    efforts: parse_list(&f.text("efforts")),
                    default_model: f.sel("default_model"),
                    default_effort: f.sel("default_effort"),
                });
                self.commit(&[], true)?;
                self.info(if note.is_empty() { "Списки сохранены".to_string() } else { format!("Списки сохранены. {}", note.join(". ")) });
                Ok(())
            }
        }
    }

    /// `edit` is Some(id) when editing; the kind is then taken from the stored task.
    pub(super) fn apply_task(&mut self, f: &Form, kind: Kind, edit: Option<&str>) -> Result<(), String> {
        let input = TaskInput {
            title: f.text("title"),
            body: f.text("body"),
            model: f.sel("model"),
            effort: f.sel("effort"),
            sprint: f.sel("sprint").parse().ok(),
            parent: Some(f.sel("parent")).filter(|p| !p.is_empty()),
            depends: f.text("depends"),
            blocks: f.text("blocks"),
        };
        // the reverse edges of «Блокирует» are written in the same commit as the card
        let (id, save) = self.board.apply_task(kind, edit, self.sel_col, &input)?;
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
}

/// One value per line: trimmed, empty lines and the reserved `default` dropped, duplicates removed in order.
pub(super) fn parse_list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in s.lines().map(str::trim) {
        if !l.is_empty() && l != "default" && !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    out
}
