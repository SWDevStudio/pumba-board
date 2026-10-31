//! Operations of the `pumba` command line for agents (`brief`, `move`, `note`, `rules`, ...).
//! Pure functions over a `Board`: they return plain structs (text rendering is a method, JSON is
//! `serde`), nothing is printed here and no process exits. The CLI layer prints and maps `ApiErr`
//! to the exit code. Contract: `templates/API-DESIGN.md`.
use crate::board::Board;
use crate::model::Store;
use serde::Serialize;
use std::path::Path;

mod read;
#[cfg(test)]
mod tests;
mod write;

pub use read::*;
pub use write::*;

/// Exit codes shared by all commands.
pub mod code {
    pub const OK: i32 = 0;
    pub const IO: i32 = 1;
    pub const ARGS: i32 = 2;
    pub const NOT_FOUND: i32 = 3;
    pub const DELETED: i32 = 4;
    pub const BLOCKED: i32 = 5;
    pub const CONFLICT: i32 = 6;
    pub const FORBIDDEN: i32 = 7;
    pub const NO_BOARD: i32 = 9;
}

/// How many times a write is repeated on fresh data when the card changed under it.
pub const ATTEMPTS: usize = 3;

/// A failed command: exit code, one-line message for stderr and extra lines for stdout
/// (similar ids, the column list, the blocking cards, the phrase for the user).
#[derive(Clone, Debug, PartialEq)]
pub struct ApiErr {
    pub code: i32,
    pub msg: String,
    pub out: Vec<String>,
}

impl ApiErr {
    pub fn new(code: i32, msg: impl Into<String>) -> ApiErr {
        ApiErr { code, msg: msg.into(), out: Vec::new() }
    }

    pub fn with_out(mut self, out: Vec<String>) -> ApiErr {
        self.out = out;
        self
    }
}

/// JSON of any output struct (one line).
pub fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

// ----------------------------------------------------------------- board

/// Load the board: `root` is the folder given by `--root` (the search for `.pumba/` goes up from
/// it), otherwise `cwd`. Code 9 when there is no board or board.json does not load.
pub fn open_board(root: Option<&Path>, cwd: &Path) -> Result<Board, ApiErr> {
    let start = root.unwrap_or(cwd);
    let Some(found) = Store::find(start) else {
        return Err(ApiErr::new(
            code::NO_BOARD,
            format!("доски нет: в {} и выше не найден .pumba/board.json (создай доску: pumba init)", start.display()),
        ));
    };
    let mut board = Board::new(&found);
    board.reload().map_err(|e| ApiErr::new(code::NO_BOARD, e))?;
    Ok(board)
}

/// Selected value of a project rule (`board`, `tests`, ...).
pub fn rule_value<'a>(board: &'a Board, id: &str) -> Option<&'a str> {
    board.cfg.rules.iter().find(|r| r.id == id).map(|r| r.value.as_str())
}

/// «Обновление доски: Вручную».
pub fn is_manual(board: &Board) -> bool {
    rule_value(board, "board") == Some("manual")
}

// ------------------------------------------------------- ids and columns

/// `t1`, `T001`, `1`, `e-2` become `T-001`, `T-001`, `T-001`, `E-002`; anything else is only
/// trimmed and upper-cased.
pub fn normalize_id(raw: &str) -> String {
    let s = raw.trim().to_uppercase();
    let digits = |r: &str| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit());
    if digits(&s) {
        return format!("T-{:03}", s.parse::<u64>().unwrap_or(0));
    }
    if let Some(p) = s.chars().next().filter(|c| *c == 'T' || *c == 'E') {
        let rest = s[1..].trim_start_matches('-');
        if digits(rest) {
            return format!("{p}-{:03}", rest.parse::<u64>().unwrap_or(0));
        }
    }
    s
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Up to 5 card ids close to `q`.
pub fn similar_ids(board: &Board, q: &str) -> Vec<String> {
    let q = normalize_id(q);
    let mut v: Vec<(usize, &str)> = board
        .tasks
        .iter()
        .map(|t| (levenshtein(&t.id, &q), t.id.as_str()))
        .filter(|(d, id)| *d <= 2 || id.contains(q.as_str()) || q.contains(*id))
        .collect();
    v.sort();
    v.into_iter().take(5).map(|(_, id)| id.to_string()).collect()
}

/// Index of the card by id (exact, then normalized). Code 3 with similar ids.
pub fn find_card(board: &Board, raw: &str) -> Result<usize, ApiErr> {
    let raw = raw.trim();
    if let Some(i) = board.find(raw).or_else(|| board.find(&normalize_id(raw))) {
        return Ok(i);
    }
    let near = similar_ids(board, raw);
    let mut out = vec![format!("NOT-FOUND {}", normalize_id(raw))];
    if !near.is_empty() {
        out.push(format!("похожие: {}", near.join(", ")));
    }
    Err(ApiErr::new(code::NOT_FOUND, format!("карточки {raw} нет")).with_out(out))
}

/// One line per column for the «no such column» answers: `id «Название»`.
pub fn column_list(board: &Board) -> Vec<String> {
    let mut v = vec!["колонки:".to_string()];
    v.extend(board.cfg.columns.iter().map(|c| format!("  {} «{}»", c.id, c.name)));
    v
}

/// Index of the column by id (exact, then without case, then by name). Code 3 with the column list.
pub fn find_column(board: &Board, key: &str) -> Result<usize, ApiErr> {
    let key = key.trim();
    let cols = &board.cfg.columns;
    cols.iter()
        .position(|c| c.id == key)
        .or_else(|| cols.iter().position(|c| c.id.eq_ignore_ascii_case(key)))
        .or_else(|| cols.iter().position(|c| c.name.trim().to_lowercase() == key.to_lowercase()))
        .ok_or_else(|| ApiErr::new(code::NOT_FOUND, format!("колонки {key} нет")).with_out(column_list(board)))
}

// ------------------------------------------------------------------ time

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `YYYY-MM-DD HH:MM UTC` from unix seconds (civil-from-days, no deps).
pub fn fmt_stamp(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + (month <= 2) as i64;
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", rem / 3_600, rem % 3_600 / 60)
}

/// Unix seconds from `YYYY-MM-DD HH:MM` (an optional ` UTC` is accepted); None when it is not one.
pub fn parse_stamp(s: &str) -> Option<u64> {
    let s = s.trim().trim_end_matches("UTC").trim_end();
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let (h, mi) = time.split_once(':')?;
    let (h, mi): (i64, i64) = (h.parse().ok()?, mi.parse().ok()?);
    if d.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&day) || !(0..24).contains(&h) || !(0..60).contains(&mi) {
        return None;
    }
    let y = y - (m <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3_600 + mi * 60).ok()
}

// ----------------------------------------------------------- write loop

/// What one attempt of a write decided.
pub(crate) enum Step<T> {
    /// Nothing to write (already so): the result.
    Done(T),
    /// Write the cards `save`, then return `out`.
    Write { save: Vec<String>, out: T },
}

/// Run a write on fresh data: reload the board, let `op` change it in memory, check that the card
/// file `id` is still the text it was read as, then commit. A file that changed in between means
/// another writer was quicker: the whole attempt is repeated (up to `ATTEMPTS`), then code 6.
/// `hook` runs right before the check (tests change the file there).
pub(crate) fn write_loop<T>(
    board: &mut Board,
    id: &str,
    hook: &mut dyn FnMut(),
    mut op: impl FnMut(&mut Board) -> Result<Step<T>, ApiErr>,
) -> Result<T, ApiErr> {
    for _ in 0..ATTEMPTS {
        board.reload().map_err(|e| ApiErr::new(code::NO_BOARD, e))?;
        let seen = board.store.read_task_raw(id);
        match op(board)? {
            Step::Done(v) => return Ok(v),
            Step::Write { save, out } => {
                hook();
                if board.store.read_task_raw(id) != seen {
                    continue;
                }
                board.commit(&save, false).map_err(|e| ApiErr::new(code::IO, e))?;
                if !board.ok {
                    return Err(ApiErr::new(code::IO, "после записи доска не читается"));
                }
                return Ok(out);
            }
        }
    }
    Err(ApiErr::new(code::CONFLICT, format!("{id}: файл меняли параллельно, {ATTEMPTS} попытки не удались; ничего не записано")))
}
