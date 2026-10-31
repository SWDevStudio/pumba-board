//! Writing commands: `move` and `note`. Both go through `write_loop` (fresh read, check before the
//! write, 3 attempts) and `Board::commit` (merge with outside edits, atomic files, AGENTS.md).
use super::read::{LOG_HEADING, log_section};
use super::{ApiErr, Step, code, find_card, find_column, fmt_stamp, is_manual, write_loop};
use crate::board::Board;
use serde::Serialize;

// ------------------------------------------------------------ work log

/// Add `line` (already `- ...`) to the end of the `## Ход работы` section of `body`; the section
/// is created at the end of the body when it is missing. Old lines are never touched.
pub fn append_log(body: &str, line: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let Some((a, b)) = log_section(&lines) else {
        let head = body.trim_end();
        return if head.is_empty() { format!("{LOG_HEADING}\n{line}") } else { format!("{head}\n\n{LOG_HEADING}\n{line}") };
    };
    let mut end = b;
    while end > a && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let mut out: Vec<&str> = lines[..end].to_vec();
    out.push(line);
    if b < lines.len() {
        out.push("");
        out.extend_from_slice(&lines[b..]);
    }
    out.join("\n")
}

/// The last line of the work log is a note with exactly this text (`- <when> — <text>`).
fn last_note_is(body: &str, text: &str) -> bool {
    let lines: Vec<&str> = body.lines().collect();
    let Some((a, b)) = log_section(&lines) else { return false };
    lines[a..b].iter().rev().find(|l| !l.trim().is_empty()).is_some_and(|l| l.trim_end().ends_with(&format!(" — {text}")))
}

/// One line of text: line breaks become spaces.
fn one_line(text: &str) -> String {
    text.split(['\r', '\n']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}

fn stamp(at: Option<&str>, now: u64) -> String {
    match at.map(str::trim).filter(|a| !a.is_empty()) {
        Some(a) => a.to_string(),
        None => fmt_stamp(now),
    }
}

fn manual_refusal(what: &str) -> ApiErr {
    ApiErr::new(code::FORBIDDEN, "доска в ручном режиме: без согласия пользователя не пишу (нужен --user-approved)").with_out(vec![
        "Обновление доски: «Вручную». Предложи пользователю:".into(),
        format!("«{what}. Сделать? (да/нет)»"),
        "После его явного «да» повтори команду с --user-approved.".into(),
    ])
}

// ---------------------------------------------------------------- move

#[derive(Clone, Debug, Default)]
pub struct MoveReq {
    pub id: String,
    /// Column id, or `next` for the `next` of the card's column.
    pub to: String,
    pub expect_from: Option<String>,
    pub note: Option<String>,
    pub user_approved: bool,
    /// Explicit time label for the note instead of the current time.
    pub at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveOut {
    pub id: String,
    pub from: String,
    pub from_name: String,
    pub to: String,
    pub to_name: String,
    /// False when the card was already in the column (nothing written).
    pub changed: bool,
}

impl MoveOut {
    /// `T-005: in-progress → review`
    pub fn render(&self) -> String {
        if self.changed { format!("{}: {} → {}\n", self.id, self.from, self.to) } else { format!("{}: уже в {}\n", self.id, self.to) }
    }
}

/// Move a card. Order of the refusals: no card 3, trash 4, no target column or `next` 3, already
/// there (success, nothing written), `--expect-from` mismatch 6, blocked by dependencies on a move
/// forward 5, manual board without `--user-approved` 7; a card changed under the write 6.
pub fn move_card(board: &mut Board, req: &MoveReq, now: u64) -> Result<MoveOut, ApiErr> {
    move_card_hooked(board, req, now, &mut || {})
}

pub(crate) fn move_card_hooked(board: &mut Board, req: &MoveReq, now: u64, hook: &mut dyn FnMut()) -> Result<MoveOut, ApiErr> {
    let id = find_card(board, &req.id)?;
    let id = board.tasks[id].id.clone();
    write_loop(board, &id, hook, |b| {
        let ti = find_card(b, &id)?;
        let t = &b.tasks[ti];
        if t.deleted {
            return Err(ApiErr::new(code::DELETED, format!("{id} в корзине; с ней не работают")));
        }
        let from = b.effective_col(t);
        let to = if req.to.trim() == "next" {
            let next = b.cfg.columns[from].next.trim().to_string();
            if next.is_empty() {
                return Err(ApiErr::new(code::NOT_FOUND, format!("у колонки {} нет next", b.cfg.columns[from].id)).with_out(super::column_list(b)));
            }
            find_column(b, &next)?
        } else {
            find_column(b, &req.to)?
        };
        let (cf, ct) = (b.cfg.columns[from].clone(), b.cfg.columns[to].clone());
        let out = MoveOut { id: id.clone(), from: cf.id.clone(), from_name: cf.name.clone(), to: ct.id.clone(), to_name: ct.name.clone(), changed: true };
        if t.column == ct.id {
            return Ok(Step::Done(MoveOut { changed: false, ..out }));
        }
        if let Some(ef) = req.expect_from.as_deref().filter(|e| !e.is_empty()) {
            if ef != cf.id {
                return Err(ApiErr::new(code::CONFLICT, format!("{id}: ожидалась колонка {ef}, карточка в {} (кто-то её уже перенёс)", cf.id)));
            }
        }
        if to > from {
            let blockers = b.blockers(t);
            if !blockers.is_empty() {
                let out: Vec<String> = blockers
                    .iter()
                    .map(|x| {
                        let c = &b.cfg.columns[b.effective_col(x)];
                        format!("{} — «{}» (`{}`)", x.id, c.name, c.id)
                    })
                    .collect();
                return Err(ApiErr::new(code::BLOCKED, format!("{id} заблокирована зависимостями; перенос вперёд запрещён")).with_out(out));
            }
        }
        if is_manual(b) && !req.user_approved {
            return Err(manual_refusal(&format!("Перенести {id} «{}» из «{}» в «{}»", t.title, cf.name, ct.name)));
        }
        b.move_card_to(&id, to);
        if let Some(note) = req.note.as_deref().map(one_line).filter(|n| !n.is_empty()) {
            let line = format!("- {} — «{}» → «{}»: {note}", stamp(req.at.as_deref(), now), cf.name, ct.name);
            let t = &mut b.tasks[ti];
            t.body = append_log(&t.body, &line);
        }
        Ok(Step::Write { save: vec![id.clone()], out })
    })
}

// ---------------------------------------------------------------- note

#[derive(Clone, Debug, Default)]
pub struct NoteReq {
    pub id: String,
    pub text: String,
    pub user_approved: bool,
    pub at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NoteOut {
    pub id: String,
    /// False when the last note already has this text (nothing written).
    pub changed: bool,
}

impl NoteOut {
    pub fn render(&self) -> String {
        if self.changed { format!("{}: заметка добавлена\n", self.id) } else { format!("{}: такая заметка уже последняя\n", self.id) }
    }
}

/// Append a note under `## Ход работы`. Errors: empty text 2, no card 3, trash 4, manual board
/// without `--user-approved` 7. The same text as the last note is a success without a write.
pub fn add_note(board: &mut Board, req: &NoteReq, now: u64) -> Result<NoteOut, ApiErr> {
    add_note_hooked(board, req, now, &mut || {})
}

pub(crate) fn add_note_hooked(board: &mut Board, req: &NoteReq, now: u64, hook: &mut dyn FnMut()) -> Result<NoteOut, ApiErr> {
    let text = one_line(&req.text);
    if text.is_empty() {
        return Err(ApiErr::new(code::ARGS, "пустой текст заметки"));
    }
    let id = find_card(board, &req.id)?;
    let id = board.tasks[id].id.clone();
    write_loop(board, &id, hook, |b| {
        let ti = find_card(b, &id)?;
        let t = &b.tasks[ti];
        if t.deleted {
            return Err(ApiErr::new(code::DELETED, format!("{id} в корзине; с ней не работают")));
        }
        if last_note_is(&t.body, &text) {
            return Ok(Step::Done(NoteOut { id: id.clone(), changed: false }));
        }
        if is_manual(b) && !req.user_approved {
            return Err(manual_refusal(&format!("Добавить в {id} заметку: {text}")));
        }
        let line = format!("- {} — {text}", stamp(req.at.as_deref(), now));
        let t = &mut b.tasks[ti];
        t.body = append_log(&t.body, &line);
        Ok(Step::Write { save: vec![id.clone()], out: NoteOut { id: id.clone(), changed: true } })
    })
}
