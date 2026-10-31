//! Read-only commands: `brief`, `column`, `rules`, `unblocks`, `rules-hash`.
use super::{ApiErr, code, find_card, find_column};
use crate::board::Board;
use crate::model::{Column, Task};
use crate::rulehash::{self, PROTOCOL};
use serde::Serialize;

/// Lines of the card body `brief --body head` shows.
pub const HEAD_LINES: usize = 40;
/// Transitions `brief` lists in `log`.
pub const LOG_SHOWN: usize = 3;
/// Name of the section of the card body that holds the notes.
pub const LOG_HEADING: &str = "## Ход работы";

// ------------------------------------------------------------ work log

/// One transition line of `## Ход работы`: `- <when> — «from» → «to»: text`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LogEntry {
    pub when: String,
    pub from: String,
    pub to: String,
}

/// Index range of the lines of the last `## Ход работы` section (heading excluded); None when
/// the body has no such section. The section ends at the next heading.
pub(crate) fn log_section(lines: &[&str]) -> Option<(usize, usize)> {
    let h = lines.iter().rposition(|l| l.trim() == LOG_HEADING)?;
    let end = lines[h + 1..].iter().position(|l| l.starts_with('#') && l.trim_start_matches('#').starts_with(' ')).map_or(lines.len(), |p| h + 1 + p);
    Some((h + 1, end))
}

/// Transition lines of the work log in file order.
pub fn log_entries(body: &str) -> Vec<LogEntry> {
    let lines: Vec<&str> = body.lines().collect();
    let Some((a, b)) = log_section(&lines) else { return Vec::new() };
    lines[a..b].iter().filter_map(|l| parse_transition(l)).collect()
}

fn parse_transition(line: &str) -> Option<LogEntry> {
    let rest = line.trim().strip_prefix("- ")?;
    let (when, tail) = rest.split_once(" — ")?;
    let tail = tail.strip_prefix('«')?;
    let (from, tail) = tail.split_once('»')?;
    let tail = tail.trim_start().strip_prefix('→')?.trim_start().strip_prefix('«')?;
    let (to, _) = tail.split_once('»')?;
    Some(LogEntry { when: when.trim().to_string(), from: from.to_string(), to: to.to_string() })
}

// -------------------------------------------------------------- column

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NextView {
    pub id: String,
    pub name: String,
}

/// A column with its position on the board.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ColView {
    pub id: String,
    pub name: String,
    /// 1-based position.
    pub pos: usize,
    pub total: usize,
    pub agent: String,
    pub next: Option<NextView>,
    pub last: String,
    pub hash: String,
}

/// The three parts of a column rule.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Parts {
    pub before: String,
    pub rule: String,
    pub after: String,
}

fn col_view(board: &Board, ci: usize) -> ColView {
    let cols = &board.cfg.columns;
    let c = &cols[ci];
    let next = cols.iter().find(|o| !c.next.trim().is_empty() && o.id == c.next).map(|o| NextView { id: o.id.clone(), name: o.name.clone() });
    ColView {
        id: c.id.clone(),
        name: c.name.clone(),
        pos: ci + 1,
        total: cols.len(),
        agent: c.agent.trim().to_string(),
        next,
        last: cols.last().map(|l| l.id.clone()).unwrap_or_default(),
        hash: rulehash::column_hash(c),
    }
}

fn parts_of(c: &Column) -> Parts {
    Parts { before: c.before.trim().to_string(), rule: c.rule.trim().to_string(), after: c.after.trim().to_string() }
}

impl ColView {
    /// `col=in-progress «В работе» (2/5) agent=pumba-worker next=review «На ревью» last=done`
    fn line(&self) -> String {
        let agent = if self.agent.is_empty() { "-" } else { self.agent.as_str() };
        let next = self.next.as_ref().map_or("-".to_string(), |n| format!("{} «{}»", n.id, n.name));
        format!("col={} «{}» ({}/{}) agent={agent} next={next} last={}", self.id, self.name, self.pos, self.total, self.last)
    }
}

impl Parts {
    /// `## Перед работой` / `## Работа` / `## После работы` with the text word for word; an empty
    /// part is left out (nothing to do in that step).
    fn render(&self) -> String {
        let mut s = String::new();
        for (title, text) in [("Перед работой", &self.before), ("Работа", &self.rule), ("После работы", &self.after)] {
            if !text.is_empty() {
                s += &format!("## {title}\n{text}\n");
            }
        }
        s
    }
}

/// `pumba column`: one column with its rule.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ColumnOut {
    pub column: ColView,
    pub parts: Parts,
}

pub fn column(board: &Board, key: &str) -> Result<ColumnOut, ApiErr> {
    let ci = find_column(board, key)?;
    Ok(ColumnOut { column: col_view(board, ci), parts: parts_of(&board.cfg.columns[ci]) })
}

impl ColumnOut {
    pub fn render(&self) -> String {
        format!("{}\nhash col:{}={}\n{}", self.column.line(), self.column.id, self.column.hash, self.parts.render())
    }
}

// --------------------------------------------------------------- brief

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BodyMode {
    /// First `HEAD_LINES` lines and the last notes.
    #[default]
    Head,
    Full,
    None,
}

#[derive(Clone, Debug, Default)]
pub struct BriefOpts {
    /// Entries `col:<id>=<hash>` / `rules=<hash>` the caller has already seen.
    pub known: Vec<String>,
    pub body: BodyMode,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ParentView {
    pub id: String,
    pub title: String,
    pub column: String,
}

/// A dependency of the card.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DepView {
    pub id: String,
    /// Column id; empty when the card does not exist.
    pub column: String,
    pub column_name: String,
    /// In the last column.
    pub done: bool,
    pub deleted: bool,
    pub missing: bool,
}

impl DepView {
    /// Holds the card back: exists, is not trashed and is not done.
    pub fn blocks(&self) -> bool {
        !self.done && !self.deleted && !self.missing
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DependentView {
    pub id: String,
    /// Other cards this dependent still waits for.
    pub waiting: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuleView {
    pub id: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BodyView {
    /// Lines of the card text without the work log.
    pub lines: usize,
    pub shown: usize,
    pub text: String,
    /// Last notes of the work log (only for `head`).
    pub notes: Vec<String>,
    pub notes_total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Brief {
    /// `OK`, `BLOCKED`, `DELETED`, `NO-PIPELINE`, `UNKNOWN-COLUMN`.
    pub status: String,
    /// Exit code of the command for this status.
    pub exit: i32,
    pub api: u32,
    pub id: String,
    pub kind: String,
    pub title: String,
    pub sprint: Option<u32>,
    pub model: String,
    pub effort: String,
    /// `column:` as stored in the card.
    pub card_column: String,
    pub column: ColView,
    pub parent: Option<ParentView>,
    pub depends: Vec<DepView>,
    pub dependents: Vec<DependentView>,
    pub board: String,
    pub rules: Vec<RuleView>,
    pub rules_hash: String,
    /// Parts the caller already knew (`--known`): not repeated below.
    pub known: Vec<String>,
    pub log: Vec<LogEntry>,
    /// Entries into the current column during the last 24 hours.
    pub visits: usize,
    pub parts: Option<Parts>,
    pub body: Option<BodyView>,
}

/// `known` has the entry `key=hash` with this hash (a prefix of at least 4 characters is enough;
/// a trailing `…` or `...` is ignored).
fn is_known(known: &[String], key: &str, hash: &str) -> bool {
    known.iter().any(|e| {
        let Some((k, v)) = e.split_once('=') else { return false };
        let v = v.trim().trim_end_matches(['…', '.']);
        k.trim() == key && v.len() >= 4 && hash.starts_with(v)
    })
}

fn dep_view(board: &Board, id: &str) -> DepView {
    let last = board.cfg.columns.len().saturating_sub(1);
    match board.tasks.iter().find(|t| t.id == id) {
        None => DepView { id: id.into(), column: String::new(), column_name: String::new(), done: false, deleted: false, missing: true },
        Some(t) => {
            let ci = board.effective_col(t);
            let c = &board.cfg.columns[ci];
            DepView { id: id.into(), column: c.id.clone(), column_name: c.name.clone(), done: ci == last, deleted: t.deleted, missing: false }
        }
    }
}

/// Visits: transitions into column `name` no longer than 24 hours before `now`.
fn count_visits(log: &[LogEntry], name: &str, now: u64) -> usize {
    log.iter().filter(|e| e.to == name && super::parse_stamp(&e.when).is_some_and(|ts| now.saturating_sub(ts) <= 86_400)).count()
}

fn body_view(body: &str, mode: BodyMode) -> Option<BodyView> {
    if mode == BodyMode::None {
        return None;
    }
    let lines: Vec<&str> = body.lines().collect();
    if mode == BodyMode::Full {
        let text = body.trim_end().to_string();
        return (!text.is_empty()).then(|| BodyView { lines: lines.len(), shown: lines.len(), text, notes: Vec::new(), notes_total: 0 });
    }
    let (main, notes): (&[&str], Vec<&str>) = match log_section(&lines) {
        Some((a, b)) => (&lines[..a - 1], lines[a..b].iter().copied().filter(|l| !l.trim().is_empty()).collect()),
        None => (&lines[..], Vec::new()),
    };
    let mut main = main.to_vec();
    while main.last().is_some_and(|l| l.trim().is_empty()) {
        main.pop();
    }
    let shown = main.len().min(HEAD_LINES);
    let notes_total = notes.len();
    let notes: Vec<String> = notes[notes_total.saturating_sub(LOG_SHOWN)..].iter().map(|l| l.to_string()).collect();
    if main.is_empty() && notes.is_empty() {
        return None;
    }
    Some(BodyView { lines: main.len(), shown, text: main[..shown].join("\n"), notes, notes_total })
}

/// Everything a driver needs for one step on card `id`. `now` (unix seconds) only counts `visits`.
/// Errors: code 3 (no such card, with similar ids). Statuses with their exit codes: `OK` 0,
/// `BLOCKED` 5, `DELETED` 4, `NO-PIPELINE` 0, `UNKNOWN-COLUMN` 0.
pub fn brief(board: &Board, id: &str, opts: &BriefOpts, now: u64) -> Result<Brief, ApiErr> {
    let ti = find_card(board, id)?;
    let t: &Task = &board.tasks[ti];
    let ci = board.effective_col(t);
    let c = &board.cfg.columns[ci];
    let col = col_view(board, ci);

    let depends: Vec<DepView> = t.depends.iter().map(|d| dep_view(board, d)).collect();
    let dependents: Vec<DependentView> = board
        .dependents(&t.id)
        .iter()
        .map(|d| DependentView { id: d.id.clone(), waiting: board.blockers(d).iter().filter(|b| b.id != t.id).map(|b| b.id.clone()).collect() })
        .collect();
    let parent = t.parent.as_ref().and_then(|p| board.tasks.iter().find(|x| x.id == *p)).map(|p| ParentView {
        id: p.id.clone(),
        title: p.title.clone(),
        column: board.cfg.columns[board.effective_col(p)].id.clone(),
    });

    let known_col = is_known(&opts.known, &format!("col:{}", c.id), &col.hash);
    let rules_hash = rulehash::rules_hash(&board.cfg);
    let known_rules = is_known(&opts.known, "rules", &rules_hash);
    let mut known = Vec::new();
    if known_col {
        known.push(format!("col:{}", c.id));
    }
    if known_rules {
        known.push("rules".to_string());
    }

    let blocked = depends.iter().any(DepView::blocks);
    let unknown_col = !board.cfg.columns.iter().any(|x| x.id == t.column);
    let no_pipeline = board.cfg.columns.iter().all(|x| x.agent.trim().is_empty() && x.next.trim().is_empty());
    let (status, exit) = if t.deleted {
        ("DELETED", code::DELETED)
    } else if blocked {
        ("BLOCKED", code::BLOCKED)
    } else if unknown_col {
        ("UNKNOWN-COLUMN", code::OK)
    } else if no_pipeline {
        ("NO-PIPELINE", code::OK)
    } else {
        ("OK", code::OK)
    };
    // a card that must not be worked on needs neither the column text nor the body
    let live = exit == code::OK;

    let log = log_entries(&t.body);
    let visits = count_visits(&log, &c.name, now);
    let skip = log.len().saturating_sub(LOG_SHOWN);

    Ok(Brief {
        status: status.into(),
        exit,
        api: PROTOCOL,
        id: t.id.clone(),
        kind: t.kind.as_str().into(),
        title: t.title.clone(),
        sprint: t.sprint,
        model: t.model.clone(),
        effort: t.effort.clone(),
        card_column: t.column.clone(),
        column: col,
        parent,
        depends,
        dependents,
        board: super::rule_value(board, "board").unwrap_or("auto").to_string(),
        rules: board.cfg.rules.iter().map(|r| RuleView { id: r.id.clone(), value: r.value.clone() }).collect(),
        rules_hash,
        known,
        log: log[skip..].to_vec(),
        visits,
        parts: (live && !known_col).then(|| parts_of(c)),
        body: if live { body_view(&t.body, opts.body) } else { None },
    })
}

impl Brief {
    fn card_line(&self) -> String {
        let sprint = self.sprint.map_or("-".to_string(), |n| n.to_string());
        format!("{} {} «{}» sprint={sprint} model={} effort={}", self.id, self.kind, self.title, self.model, self.effort)
    }

    /// Compact text for an agent: first line is the status, then `key=value` lines, then `##` sections.
    pub fn render(&self) -> String {
        let mut s = String::new();
        match self.status.as_str() {
            "DELETED" => {
                s += &format!("DELETED {} «{}»\n", self.id, self.title);
                return s;
            }
            "BLOCKED" => {
                let b: Vec<String> =
                    self.depends.iter().filter(|d| d.blocks()).map(|d| format!("{} «{}» ({})", d.id, d.column_name, d.column)).collect();
                s += &format!("BLOCKED {}\n{} api={}\n", b.join(", "), self.card_line(), self.api);
            }
            st => {
                s += &format!("{st} {} api={}", self.card_line(), self.api);
                if st == "UNKNOWN-COLUMN" {
                    s += &format!(" column={}", self.card_column);
                }
                s += "\n";
            }
        }
        s += &self.column.line();
        s += "\n";

        let mut rel: Vec<String> = Vec::new();
        rel.push(match &self.parent {
            Some(p) => format!("parent={} «{}» {}", p.id, p.title, p.column),
            None => "parent=-".into(),
        });
        let deps: Vec<String> = self
            .depends
            .iter()
            .map(|d| {
                if d.missing {
                    format!("{} нет (не блокирует)", d.id)
                } else if d.deleted {
                    format!("{} в корзине (не блокирует)", d.id)
                } else {
                    format!("{} {} {}", d.id, d.column, if d.done { "✓" } else { "✗" })
                }
            })
            .collect();
        rel.push(format!("depends: {}", if deps.is_empty() { "-".to_string() } else { deps.join(", ") }));
        let dd: Vec<String> = self
            .dependents
            .iter()
            .map(|d| if d.waiting.is_empty() { d.id.clone() } else { format!("{} (ждёт ещё {})", d.id, d.waiting.join(", ")) })
            .collect();
        rel.push(format!("dependents: {}", if dd.is_empty() { "-".to_string() } else { dd.join(", ") }));
        s += &rel.join(" | ");
        s += "\n";

        // project rule labels: only `board` once the whole set is known to the caller
        let known_rules = self.known.iter().any(|k| k == "rules");
        let mut labels = vec![format!("board={}", self.board)];
        if !known_rules {
            labels.extend(self.rules.iter().filter(|r| r.id != "board").map(|r| format!("{}={}", r.id, r.value)));
        }
        s += &labels.join(" ");
        s += "\n";

        s += &format!("hash rules={} col:{}={}\n", self.rules_hash, self.column.id, self.column.hash);
        if !self.known.is_empty() {
            s += &format!("known: {}\n", self.known.join(", "));
        }
        let log: Vec<String> = self.log.iter().map(|e| format!("{} «{}»→«{}»", e.when, e.from, e.to)).collect();
        s += &format!("log: {}; visits={}\n", if log.is_empty() { "-".to_string() } else { log.join("; ") }, self.visits);

        if let Some(p) = &self.parts {
            s += &p.render();
        }
        if let Some(b) = &self.body {
            if b.shown < b.lines {
                s += &format!("## Тело ({} из {} строк; всё: --body full)\n", b.shown, b.lines);
            } else {
                s += &format!("## Тело ({} строк)\n", b.lines);
            }
            if !b.text.is_empty() {
                s += &b.text;
                s += "\n";
            }
            if !b.notes.is_empty() {
                s += &format!("## Ход работы (последние {} из {})\n", b.notes.len(), b.notes_total);
                for n in &b.notes {
                    s += n.trim_end();
                    s += "\n";
                }
            }
        }
        s
    }
}

// --------------------------------------------------------------- rules

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuleLine {
    pub id: String,
    pub title: String,
    pub value: String,
    pub label: String,
    pub text: String,
}

/// `pumba rules`: selected project rules and the custom rules.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RulesOut {
    pub rules: Vec<RuleLine>,
    pub custom: Option<String>,
}

/// Project rules; `parts` picks rule ids (and `custom`), empty means all. Code 2 for an unknown id.
pub fn rules(board: &Board, parts: &[String]) -> Result<RulesOut, ApiErr> {
    let cfg = &board.cfg;
    if let Some(bad) = parts.iter().find(|p| *p != "custom" && !cfg.rules.iter().any(|r| r.id == **p)) {
        let mut ids: Vec<&str> = cfg.rules.iter().map(|r| r.id.as_str()).collect();
        ids.push("custom");
        return Err(ApiErr::new(code::ARGS, format!("неизвестная часть правил «{bad}»; есть: {}", ids.join(", "))));
    }
    let all = parts.is_empty();
    let rules = cfg
        .rules
        .iter()
        .filter(|r| all || parts.contains(&r.id))
        .filter_map(|r| {
            let o = r.options.iter().find(|o| o.id == r.value)?;
            Some(RuleLine { id: r.id.clone(), title: r.title.clone(), value: r.value.clone(), label: o.label.clone(), text: o.text.clone() })
        })
        .collect();
    let custom = cfg.custom_rules.trim();
    let custom = ((all || parts.iter().any(|p| p == "custom")) && !custom.is_empty()).then(|| custom.to_string());
    Ok(RulesOut { rules, custom })
}

impl RulesOut {
    /// A block ready to paste into a subagent prompt.
    pub fn render(&self) -> String {
        let mut s = String::new();
        if !self.rules.is_empty() {
            s += "## Правила проекта\n";
            for r in &self.rules {
                s += &format!("- {}: {}. {}\n", r.title, r.label, r.text);
            }
        }
        if let Some(c) = &self.custom {
            s += &format!("## Свои правила\n{c}\n");
        }
        s
    }
}

// ------------------------------------------------------------ unblocks

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UnblockLine {
    pub id: String,
    pub title: String,
    /// `unblocked`, `waits` (other cards still hold it) or `pending` (this card is not done yet).
    pub state: String,
    /// Other cards the dependent still waits for.
    pub waiting: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UnblocksOut {
    pub id: String,
    pub done: bool,
    /// Name of the last column.
    pub last_name: String,
    pub dependents: Vec<UnblockLine>,
}

pub fn unblocks(board: &Board, id: &str) -> Result<UnblocksOut, ApiErr> {
    let ti = find_card(board, id)?;
    let t = &board.tasks[ti];
    let last = board.cfg.columns.len().saturating_sub(1);
    let done = board.effective_col(t) == last;
    let dependents = board
        .dependents(&t.id)
        .iter()
        .map(|d| {
            let waiting: Vec<String> = board.blockers(d).iter().filter(|b| b.id != t.id).map(|b| b.id.clone()).collect();
            let state = match (done, waiting.is_empty()) {
                (false, _) => "pending",
                (true, true) => "unblocked",
                (true, false) => "waits",
            };
            UnblockLine { id: d.id.clone(), title: d.title.clone(), state: state.into(), waiting }
        })
        .collect();
    Ok(UnblocksOut { id: t.id.clone(), done, last_name: board.cfg.columns[last].name.clone(), dependents })
}

impl UnblocksOut {
    pub fn render(&self) -> String {
        if self.dependents.is_empty() {
            return format!("{}: никого не блокирует\n", self.id);
        }
        let mut s = String::new();
        for d in &self.dependents {
            let more = if d.waiting.is_empty() { String::new() } else { format!("ждёт ещё {}", d.waiting.join(", ")) };
            let state = match d.state.as_str() {
                "unblocked" => "разблокирована".to_string(),
                "waits" => more,
                _ if d.waiting.is_empty() => format!("разблокируется, когда {} дойдёт до «{}»", self.id, self.last_name),
                _ => format!("разблокируется, когда {} дойдёт до «{}»; {more}", self.id, self.last_name),
            };
            s += &format!("{} «{}» — {state}\n", d.id, d.title);
        }
        s
    }
}

// ---------------------------------------------------------- rules-hash

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HashOut {
    pub hash: String,
    pub protocol: u32,
    pub parts: Vec<rulehash::Part>,
}

pub fn rules_hash(board: &Board) -> HashOut {
    HashOut { hash: rulehash::rules_hash(&board.cfg), protocol: PROTOCOL, parts: rulehash::parts(&board.cfg) }
}

impl HashOut {
    /// One line with the hash, or with `parts` the hash and one `name=hash` line per part.
    pub fn render(&self, parts: bool) -> String {
        if !parts {
            return format!("{}\n", self.hash);
        }
        let mut s = format!("rules={}\n", self.hash);
        for p in &self.parts {
            s += &format!("{}={}\n", p.name, p.hash);
        }
        s
    }
}
