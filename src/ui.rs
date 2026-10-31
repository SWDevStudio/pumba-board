//! All drawing. Pure view code: reads `App`, only adjusts scroll offsets.
use crate::app::{Action, App, Mode, ToolPick};
use crate::editor::TextBuf;
use crate::files;
use crate::form::{AgentRow, CardRow, FieldKind, Form, Opt, OptRow, PickKind, PickerView, Purpose, Suggest, FILTER_MIN_OPTS};
use crate::logo;
use crate::model::{Kind, Task};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

const MIN_COL_W: u16 = 30;
const CARD_H: u16 = 6;
const DETAIL_H: u16 = 9;
const LABEL_W: usize = 16;

// ---------------------------------------------------------------- helpers

fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

fn italic(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::ITALIC)
}

/// Truncate by chars, ending with `…` when cut.
fn trunc(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut o: String = s.chars().take(w - 1).collect();
    o.push('…');
    o
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

fn vis_cols(width: u16, n: usize) -> usize {
    ((width / MIN_COL_W) as usize).max(1).min(n.max(1))
}

/// (agent, level) for a card header. Default model shows as `авто`; default
/// effort is empty. Both default -> both empty.
fn agent_level(t: &Task) -> (String, String) {
    let is_def = |s: &str| s.is_empty() || s == "default";
    if is_def(&t.model) && is_def(&t.effort) {
        return (String::new(), String::new());
    }
    let agent = if is_def(&t.model) { "авто".to_string() } else { t.model.clone() };
    let level = if is_def(&t.effort) { String::new() } else { t.effort.clone() };
    (agent, level)
}

/// Word-wrap `s` by chars to `w` columns, at most `max` lines; the last line
/// is truncated with `…` when text remains.
fn wrap_title(s: &str, w: usize, max: usize) -> Vec<String> {
    if w == 0 || max == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;
    for word in s.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        if cur_len > 0 && cur_len + 1 + word.len() <= w {
            cur.push(' ');
            cur.extend(word.iter());
            cur_len += 1 + word.len();
            continue;
        }
        if cur_len > 0 {
            lines.push(std::mem::take(&mut cur));
        }
        while word.len() > w {
            let rest = word.split_off(w);
            lines.push(word.iter().collect());
            word = rest;
        }
        cur_len = word.len();
        cur = word.iter().collect();
    }
    if cur_len > 0 {
        lines.push(cur);
    }
    if lines.len() > max {
        let tail = lines[max - 1..].join(" ");
        lines.truncate(max - 1);
        // force the `…`: the tail is known to be longer than one line
        let head: String = tail.chars().take(w.saturating_sub(1)).collect();
        let mut cut = head.trim_end().to_string();
        cut.push('…');
        lines.push(cut);
    }
    lines
}

fn panel_block() -> Block<'static> {
    Block::bordered().border_type(BorderType::Rounded).style(Style::new().bg(PANEL).fg(FG))
}

// ------------------------------------------------------------------- draw

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(BG).fg(FG)), area);
    let [header, cols, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(DETAIL_H),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_columns(f, cols, app);
    draw_header(f, header, app);
    draw_detail(f, detail, app);
    draw_footer(f, footer, app);

    match &mut app.mode {
        Mode::Board => {}
        Mode::Form(form) => draw_form(f, form),
        Mode::Confirm(msg, Action::Init) => draw_confirm_init(f, msg),
        Mode::Confirm(msg, _) => draw_confirm(f, msg),
        Mode::Help => draw_help(f),
        Mode::Trash(_) => {}
        Mode::InitTools(p) => draw_init_tools(f, p),
    }
    if let Mode::Trash(sel) = app.mode {
        draw_trash(f, app, sel);
    }
}

// ----------------------------------------------------------------- header

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let mut sp = vec![
        Span::styled(" PUMBA ", Style::new().fg(BG_SOLID).bg(ORANGE).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", app.project), fg(FG)),
        Span::styled(format!(" Спринт {} ", app.cfg.sprint), fg(GREEN)),
    ];
    if app.sprint_only {
        sp.push(Span::styled(" [только спринт]", fg(ORANGE)));
    }
    if app.show_hidden {
        sp.push(Span::styled(" [скрытые видны]", fg(ORANGE)));
    }
    let trash = app.tasks.iter().filter(|t| t.deleted).count();
    if trash > 0 {
        sp.push(Span::styled(format!(" корзина: {trash}"), fg(MUTED)));
    }
    f.render_widget(Paragraph::new(Line::from(sp)), area);

    let n = app.cfg.columns.len();
    let vis = vis_cols(area.width, n);
    if n > 0 && vis < n {
        let a = app.col_scroll + 1;
        let txt = format!("кол. {}-{}/{} ", a, a + vis - 1, n);
        f.render_widget(Paragraph::new(Span::styled(txt, fg(MUTED))).alignment(Alignment::Right), area);
    }
}

// ---------------------------------------------------------------- columns

fn draw_columns(f: &mut Frame, area: Rect, app: &mut App) {
    let n = app.cfg.columns.len();
    if n == 0 || area.width == 0 || area.height == 0 {
        return;
    }
    let vis = vis_cols(area.width, n);
    if app.sel_col >= n {
        app.sel_col = n - 1;
    }
    if app.sel_col < app.col_scroll {
        app.col_scroll = app.sel_col;
    }
    if app.sel_col >= app.col_scroll + vis {
        app.col_scroll = app.sel_col + 1 - vis;
    }
    app.col_scroll = app.col_scroll.min(n - vis);
    if app.scrolls.len() != n {
        app.scrolls.resize(n, 0);
    }
    let w = area.width / vis as u16;
    for k in 0..vis {
        let x = area.x + k as u16 * w;
        let cw = if k == vis - 1 { area.x + area.width - x } else { w };
        draw_column(f, Rect::new(x, area.y, cw, area.height), app.col_scroll + k, app);
    }
}

fn draw_column(f: &mut Frame, r: Rect, ci: usize, app: &mut App) {
    let col = app.cfg.columns[ci].clone();
    let color = by_name(&col.color);
    let idxs = app.col_task_idx(ci);
    let total = idxs.len();
    let is_sel = ci == app.sel_col;
    let cap = (r.height.saturating_sub(1) / CARD_H) as usize;

    let mut scroll = app.scrolls[ci];
    if is_sel && cap > 0 {
        if app.sel_idx < scroll {
            scroll = app.sel_idx;
        } else if app.sel_idx >= scroll + cap {
            scroll = app.sel_idx + 1 - cap;
        }
    }
    scroll = scroll.min(total.saturating_sub(cap.max(1)));
    app.scrolls[ci] = scroll;

    // header
    let hr = Rect::new(r.x, r.y, r.width.saturating_sub(1).max(1).min(r.width), 1);
    let mut hs = Style::new().fg(color);
    if is_sel {
        hs = hs.bg(PANEL).add_modifier(Modifier::BOLD);
    }
    f.render_widget(Block::new().style(hs), hr);
    let text = trunc(&format!(" ● {}  {}", col.name, total), (hr.width as usize).saturating_sub(3));
    f.render_widget(Paragraph::new(text).style(hs), hr);
    let mut arrows = String::new();
    if scroll > 0 {
        arrows.push('▲');
    }
    if scroll + cap < total {
        arrows.push('▼');
    }
    if !arrows.is_empty() {
        f.render_widget(Paragraph::new(format!("{arrows} ")).style(hs).alignment(Alignment::Right), hr);
    }

    if r.height <= 1 {
        return;
    }
    if total == 0 {
        f.render_widget(
            Paragraph::new(Span::styled(" пусто", italic(MUTED))),
            Rect::new(r.x, r.y + 1, r.width, 1),
        );
        return;
    }
    for (k, &ti) in idxs.iter().skip(scroll).take(cap).enumerate() {
        let cr = Rect::new(r.x, r.y + 1 + k as u16 * CARD_H, r.width.saturating_sub(1).max(1).min(r.width), CARD_H);
        let selected = is_sel && scroll + k == app.sel_idx;
        draw_card(f, cr, app, &app.tasks[ti], selected);
    }
}

fn draw_card(f: &mut Frame, r: Rect, app: &App, t: &Task, selected: bool) {
    let (border, bg) = if selected { (ORANGE, PANEL) } else { (MUTED, BG) };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(border))
        .style(Style::new().bg(bg).fg(FG));
    let inner = block.inner(r);
    f.render_widget(block, r);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let iw = inner.width as usize;

    let epic = t.kind == Kind::Epic;
    let (mk, mkc) = if epic { ("◆", PINK) } else { ("▪", STRING) };
    let mut ts = Style::new().fg(if t.hidden { MUTED } else { FG });
    if epic {
        ts = ts.add_modifier(Modifier::BOLD);
    }
    if t.hidden {
        ts = ts.add_modifier(Modifier::ITALIC);
    }
    // line 1: `<marker> <id>  <agent>*<level>`
    let mut l1 = vec![
        Span::styled(mk, fg(mkc)),
        Span::raw(" "),
        Span::styled(t.id.clone(), fg(MUTED)),
    ];
    let used = 2 + t.id.chars().count() + 2;
    let (agent, level) = agent_level(t);
    if !agent.is_empty() && iw > used {
        let room = iw - used;
        let a = trunc(&agent, room);
        let a_len = a.chars().count();
        l1.push(Span::raw("  "));
        l1.push(Span::styled(a, fg(PURPLE)));
        if !level.is_empty() && room > a_len + 1 && a_len == agent.chars().count() {
            l1.push(Span::styled("*", fg(MUTED)));
            l1.push(Span::styled(trunc(&level, room - a_len - 1), fg(PURPLE)));
        }
    }
    let mut lines = vec![Line::from(l1)];
    // lines 2..3: title, wrapped to at most 2 lines
    let tl = wrap_title(&t.title, iw, 2);
    for k in 0..2 {
        lines.push(Line::from(Span::styled(tl.get(k).cloned().unwrap_or_default(), ts)));
    }

    // line 4 (text, color, drop priority): when too wide the highest priority number goes first.
    let mut parts: Vec<(String, Color, u8)> = Vec::new();
    if epic {
        let (done, tot) = app.epic_progress(&t.id);
        let filled = if tot == 0 { 0 } else { (done * 4 / tot).min(4) };
        parts.push((format!("{}{} {}/{}", "▰".repeat(filled), "▱".repeat(4 - filled), done, tot), GREEN, 0));
    } else {
        if let Some(n) = t.sprint {
            parts.push((format!("S{n}"), GREEN, 2));
        }
        if let Some(p) = &t.parent {
            parts.push((format!("↳ {p}"), PINK, 4));
        }
        if t.hidden {
            parts.push(("скрыта".into(), MUTED, 5));
        }
    }
    // the card's `column:` names no column: it shows in the first one, flagged. Dropped last of all
    // (priority 0), placed before `скрыта`.
    if !app.cfg.columns.iter().any(|c| c.id == t.column) {
        let at = parts.iter().position(|p| p.0 == "скрыта").unwrap_or(parts.len());
        parts.insert(at, ("?колонка".into(), RED, 0));
    }
    // `→ T-005,T-006`: cards waiting on this one (dependents outside the last column). Shown before
    // `скрыта` and dropped right after it; when the ids do not fit it shrinks to `→ n`.
    let last = app.cfg.columns.len().saturating_sub(1);
    let waiting: Vec<&str> =
        app.dependents(&t.id).into_iter().filter(|d| app.effective_col(d) != last).map(|d| d.id.as_str()).collect();
    let mut blocks_cnt = None;
    if !waiting.is_empty() {
        let at = parts.iter().position(|p| p.0 == "скрыта").unwrap_or(parts.len());
        parts.insert(at, (format!("→ {}", waiting.join(",")), ORANGE_DIM, 5));
        blocks_cnt = Some(format!("→ {}", waiting.len()));
    }
    let blockers = app.blockers(t);
    if !blockers.is_empty() {
        let ids: Vec<&str> = blockers.iter().map(|b| b.id.as_str()).collect();
        parts.insert(0, (format!("⊘ {}", ids.join(",")), RED, 1));
    } else if !t.depends.is_empty() {
        parts.push(("✓ зав.".into(), MUTED, 4));
    }
    let width = |p: &[(String, Color, u8)]| p.iter().map(|x| x.0.chars().count()).sum::<usize>() + 2 * p.len().saturating_sub(1);
    if let Some(cnt) = blocks_cnt {
        if width(&parts) > iw {
            if let Some(p) = parts.iter_mut().find(|p| p.1 == ORANGE_DIM) {
                p.0 = cnt;
            }
        }
    }
    while parts.len() > 1 && width(&parts) > iw {
        let worst = parts.iter().enumerate().max_by_key(|(i, p)| (p.2, *i)).map(|(i, _)| i).unwrap();
        parts.remove(worst);
    }
    if let [only] = parts.as_mut_slice() {
        only.0 = trunc(&only.0, iw);
    }
    let mut s2v: Vec<Span<'static>> = Vec::new();
    for (txt, c, _) in parts {
        if !s2v.is_empty() {
            s2v.push(Span::raw("  "));
        }
        s2v.push(Span::styled(txt, fg(c)));
    }
    lines.push(Line::from(s2v));
    f.render_widget(Paragraph::new(lines), inner);
}

// ----------------------------------------------------------------- detail

fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
    let [l, r] = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);
    let sel = app.selected_idx().and_then(|i| app.tasks.get(i));

    let lb = Block::bordered().border_style(fg(MUTED));
    match sel {
        Some(t) => {
            let ci = app.effective_col(t);
            let known = app.cfg.columns.iter().any(|c| c.id == t.column);
            let colname = app.cfg.columns.get(ci).map(|c| c.name.as_str()).unwrap_or("?");
            let lb = lb.title(Line::from(vec![
                Span::styled(format!(" {}", t.id), bold(ORANGE)),
                Span::styled(format!(" · {} ", if t.kind == Kind::Epic { "эпик" } else { "задача" }), fg(MUTED)),
            ]));
            let lab = |s: &str| Span::styled(format!("{s} "), fg(MUTED));
            let val = |s: String| Span::styled(s, fg(FG));
            let mut meta = if known {
                vec![lab("колонка"), val(colname.to_string())]
            } else {
                let id = if t.column.is_empty() { "<пусто>" } else { t.column.as_str() };
                vec![lab("колонка:"), Span::styled(format!("{id} (нет такой колонки)"), fg(RED))]
            };
            meta.extend([
                Span::raw("   "),
                lab("модель"),
                val(t.model.clone()),
                Span::raw("   "),
                lab("уровень"),
                val(t.effort.clone()),
                Span::raw("   "),
                lab("спринт"),
                val(t.sprint.map(|n| n.to_string()).unwrap_or_else(|| "-".into())),
            ]);
            if let Some(p) = &t.parent {
                meta.push(Span::raw("   "));
                meta.push(lab("эпик"));
                meta.push(val(p.clone()));
            }
            if !t.depends.is_empty() {
                let deps: Vec<String> = t
                    .depends
                    .iter()
                    .map(|d| {
                        let dep = app.tasks.iter().find(|x| x.id == *d);
                        let name = if dep.is_some_and(|x| x.deleted) {
                            "в корзине"
                        } else {
                            dep.and_then(|x| app.cfg.columns.get(app.effective_col(x)))
                                .map_or("?", |c| c.name.as_str())
                        };
                        format!("{d} ({name})")
                    })
                    .collect();
                meta.push(Span::raw("   "));
                meta.push(lab("зависит:"));
                meta.push(val(deps.join(", ")));
            }
            let dependents = app.dependents(&t.id);
            if !dependents.is_empty() {
                let list: Vec<String> = dependents
                    .iter()
                    .map(|x| {
                        let name = app.cfg.columns.get(app.effective_col(x)).map_or("?", |c| c.name.as_str());
                        format!("{} ({name})", x.id)
                    })
                    .collect();
                meta.push(Span::raw("   "));
                meta.push(lab("блокирует:"));
                meta.push(val(list.join(", ")));
            }
            if !app.blockers(t).is_empty() {
                meta.push(Span::raw("   "));
                meta.push(Span::styled("ЗАБЛОКИРОВАНА", bold(RED)));
            }
            let mut lines = vec![Line::styled(t.title.clone(), bold(ORANGE)), Line::from(meta), Line::raw("")];
            if t.body.trim().is_empty() {
                lines.push(Line::styled("(нет описания)", italic(MUTED)));
            } else {
                for ln in t.body.lines() {
                    lines.push(Line::styled(ln.to_string(), fg(PARAM)));
                }
            }
            f.render_widget(Paragraph::new(lines).block(lb).wrap(Wrap { trim: false }), l);
        }
        None => {
            let p = Paragraph::new(Span::styled(
                "Ничего не выбрано. Нажмите n для новой задачи или N для нового эпика.",
                italic(MUTED),
            ))
            .block(lb.title(Span::styled(" Детали ", fg(MUTED))))
            .wrap(Wrap { trim: false });
            f.render_widget(p, l);
        }
    }

    let col = app.cfg.columns.get(app.sel_col);
    let name = col.map_or(String::new(), |c| c.name.clone());
    let rb = Block::bordered()
        .border_style(fg(MUTED))
        .title(Span::styled(format!(" Правило колонки · {name} "), fg(MUTED)));
    f.render_widget(Paragraph::new(rule_lines(col)).block(rb).wrap(Wrap { trim: true }), r);
}

/// The column rule in reading order: «Перед работой», «Работа», «После работы»; empty parts are
/// left out (old boards have only «Работа»).
fn rule_lines(col: Option<&crate::model::Column>) -> Vec<Line<'static>> {
    let Some(c) = col else { return Vec::new() };
    let mut out = Vec::new();
    for (label, text) in [("Перед работой", &c.before), ("Работа", &c.rule), ("После работы", &c.after)] {
        for (i, l) in text.trim().lines().enumerate() {
            let mut sp = Vec::new();
            if i == 0 {
                sp.push(Span::styled(format!("{label}: "), bold(PURPLE)));
            }
            sp.push(Span::styled(l.to_string(), italic(PURPLE)));
            out.push(Line::from(sp));
        }
    }
    out
}

// ----------------------------------------------------------------- footer

const HINTS: [(&str, &str); 12] = [
    ("h/l", "кол."),
    ("j/k", "карт."),
    ("H/L", "сдвиг"),
    ("Ctrl+←/→", "колонку"),
    ("n", "задача"),
    ("N", "эпик"),
    ("↵", "правка"),
    ("d", "удалить"),
    ("x/v", "скрыть"),
    ("s", "спринт"),
    ("?", "помощь"),
    ("q", "выход"),
];

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    if let Some((msg, is_err, _)) = &app.status {
        let c = if *is_err { RED } else if app.status_warn { ORANGE } else { GREEN };
        f.render_widget(
            Paragraph::new(Span::styled(format!(" {}", trunc(msg, area.width.saturating_sub(2) as usize)), fg(c))),
            area,
        );
        return;
    }
    // card files that could not be read stay in view until the next load clears them
    if let Some(note) = app.unreadable_note() {
        f.render_widget(
            Paragraph::new(Span::styled(format!(" {}", trunc(&note, area.width.saturating_sub(2) as usize)), fg(RED))),
            area,
        );
        return;
    }
    let mut sp = vec![Span::raw(" ")];
    for (i, (k, t)) in HINTS.iter().enumerate() {
        if i > 0 {
            sp.push(Span::raw(" "));
        }
        sp.push(Span::styled(*k, fg(ORANGE)));
        sp.push(Span::styled(format!(" {t}"), fg(MUTED)));
    }
    f.render_widget(Paragraph::new(Line::from(sp)), area);
}

// ------------------------------------------------------------------- form

fn field_height(k: &FieldKind) -> u16 {
    match k {
        FieldKind::Text(tb) if !tb.multiline => 3,
        FieldKind::Text(_) => 4,
        FieldKind::Select { opts, .. } => {
            if opts.iter().any(|o| !o.hint.is_empty()) {
                2
            } else {
                1
            }
        }
    }
}

/// Popup inner width from which task forms switch to the two-column layout.
const TWO_COL_MIN_W: u16 = 76;
/// The same for the column form: its side fields are short, so it goes two-column earlier
/// (80-column terminals included) and keeps the three rule parts tall enough.
const COL_FORM_TWO_COL_MIN_W: u16 = 60;
/// Text rows a multiline field of the column form gets when the terminal allows it.
const COL_FORM_MULTI_ROWS: u16 = 3;

/// Stack the fields `idx` top to bottom inside `area`; spare height is shared evenly between the
/// multiline fields (the remainder goes one row each to the first ones). Rects land in
/// `out[field index]`.
fn stack_fields(form: &Form, idx: &[usize], area: Rect, out: &mut [Rect]) {
    let mut hs: Vec<u16> = idx.iter().map(|&i| field_height(&form.fields[i].kind)).collect();
    let extra = area.height.saturating_sub(hs.iter().sum::<u16>());
    let multi: Vec<usize> = (0..idx.len())
        .filter(|&k| matches!(&form.fields[idx[k]].kind, FieldKind::Text(tb) if tb.multiline))
        .collect();
    if !multi.is_empty() {
        let n = multi.len() as u16;
        let (share, rest) = (extra / n, extra % n);
        for (j, &k) in multi.iter().enumerate() {
            hs[k] += share + u16::from((j as u16) < rest);
        }
    }
    let mut y = area.y;
    let bottom = area.bottom();
    for (k, &i) in idx.iter().enumerate() {
        let h = hs[k].min(bottom.saturating_sub(y));
        out[i] = Rect::new(area.x, y, area.width, h);
        y += h;
    }
}

fn draw_form(f: &mut Frame, form: &mut Form) {
    let area = f.area();
    let w = ((area.width as u32 * 9 / 10) as u16).max(10).min(area.width);
    let has_multi = form.fields.iter().any(|fl| matches!(&fl.kind, FieldKind::Text(tb) if tb.multiline));
    // Task and epic forms get a second footer line pointing at the model/effort list editor;
    // forms with `@`-capable fields use it for the file reference hint.
    let task_form = matches!(form.purpose, Purpose::NewTask(_) | Purpose::EditTask(_));
    let column_form = matches!(form.purpose, Purpose::NewColumn | Purpose::EditColumn(_));
    let has_mentions = form.picker_enabled() && has_multi;
    // fields with their own hint line (executor suggestions, long selects) keep it two rows tall too
    let has_hints = form.fields.iter().any(|fl| match &fl.kind {
        FieldKind::Text(_) => fl.suggest == Suggest::Agents,
        FieldKind::Select { opts, .. } => opts.len() > FILTER_MIN_OPTS,
    });
    let foot_h: u16 = if task_form || has_mentions || has_hints { 2 } else { 1 };
    // the popup's inner width: borders and a one-cell margin on each side
    let inner_w = w.saturating_sub(4);
    let (two_col_min_w, right_min_w) = if column_form { (COL_FORM_TWO_COL_MIN_W, 32) } else { (TWO_COL_MIN_W, 34) };
    let two_col =
        form.fields.iter().any(|fl| fl.side) && form.fields.iter().any(|fl| !fl.side) && inner_w >= two_col_min_w;

    let fixed: u16 = form.fields.iter().map(|fl| field_height(&fl.kind)).sum::<u16>() + 1;
    let cap = ((area.height as u32 * 9 / 10) as u16).min(area.height.saturating_sub(2)).max(5).min(area.height);
    let h = if !has_multi {
        (fixed + 2).min(cap)
    } else if column_form {
        // the column form grows past 90% (up to one free row above and below) when that is what
        // it takes to give each rule part COL_FORM_MULTI_ROWS text rows
        let want = |side: bool| -> u16 {
            form.fields
                .iter()
                .filter(|fl| !two_col || fl.side == side)
                .map(|fl| match &fl.kind {
                    FieldKind::Text(tb) if tb.multiline => COL_FORM_MULTI_ROWS + 2,
                    k => field_height(k),
                })
                .sum()
        };
        let need = want(false).max(want(true)) + foot_h + 2;
        cap.max(need.min(area.height.saturating_sub(2)))
    } else {
        cap
    };
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = panel_block()
        .border_style(fg(ORANGE))
        .title(Span::styled(format!(" {} ", form.title), bold(ORANGE)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);

    let (body_area, foot_area) = {
        let [b, ft] = Layout::vertical([Constraint::Min(0), Constraint::Length(foot_h.min(inner.height))]).areas(inner);
        (b, ft)
    };
    let mut rects = vec![Rect::default(); form.fields.len()];
    let side_label_w;
    if two_col {
        let right_w = ((inner.width as u32 * 40 / 100) as u16).clamp(right_min_w, 44);
        let left_w = inner.width.saturating_sub(right_w + 1);
        let left = Rect::new(body_area.x, body_area.y, left_w, body_area.height);
        let right = Rect::new(body_area.x + left_w + 1, body_area.y, right_w, body_area.height);
        let idx_left: Vec<usize> = (0..form.fields.len()).filter(|&i| !form.fields[i].side).collect();
        let idx_right: Vec<usize> = (0..form.fields.len()).filter(|&i| form.fields[i].side).collect();
        stack_fields(form, &idx_left, left, &mut rects);
        stack_fields(form, &idx_right, right, &mut rects);
        side_label_w = idx_right
            .iter()
            .filter(|&&i| matches!(form.fields[i].kind, FieldKind::Select { .. }))
            .map(|&i| form.fields[i].label.chars().count())
            .max()
            .unwrap_or(LABEL_W);
    } else {
        let all: Vec<usize> = (0..form.fields.len()).collect();
        stack_fields(form, &all, body_area, &mut rects);
        // long select labels (the defaults in the models form) get the room they need, within reason
        let longest = form
            .fields
            .iter()
            .filter(|fl| matches!(fl.kind, FieldKind::Select { .. }))
            .map(|fl| fl.label.chars().count())
            .max()
            .unwrap_or(0);
        side_label_w = longest.min((body_area.width as usize).saturating_sub(24)).max(LABEL_W);
    }

    form.two_col = two_col;
    form.rects = rects.clone();
    let focus = form.focus;
    // popup anchor of the focused text field: (x, first row below, first row above)
    let mut cursor = None;
    for (i, field) in form.fields.iter_mut().enumerate() {
        let r = rects[i];
        let focused = i == focus;
        match &mut field.kind {
            FieldKind::Text(tb) => {
                let c = draw_text(f, r, &field.label, tb, focused);
                if let Some((x, y)) = c {
                    cursor = cursor.or(Some(if tb.multiline { (x, y + 1, y) } else { (r.x, r.bottom(), r.y) }));
                }
            }
            FieldKind::Select { opts, idx } => {
                let lw = if two_col && field.side || !two_col { side_label_w } else { LABEL_W };
                draw_select(f, r, &field.label, opts, *idx, focused, lw);
                if focused {
                    // filter popup hangs under the value, like the text popups under their cursor
                    cursor = cursor.or(Some(((r.x + 2 + lw as u16 + 4).min(r.right().saturating_sub(1)), r.bottom(), r.y)));
                }
            }
        }
    }
    let mut foot = vec![Line::styled("Tab/Shift+Tab поле · ←/→ выбор · Ctrl+S сохранить · Esc отмена", fg(MUTED))];
    let mut second: Vec<&str> = Vec::new();
    if task_form {
        second.push("Список моделей: M на доске");
    }
    match form.focused_suggest() {
        Suggest::Files => second.push("@ — файл проекта"),
        Suggest::Cards => second.insert(0, "Введите id или название — подсказка карточек"),
        Suggest::Agents if !form.agents.is_empty() => second.insert(0, "Подсказка: ↑↓ Enter — выбрать субагента"),
        Suggest::Agents | Suggest::None => {}
    }
    if form.focused_filterable() {
        second.insert(0, "Печатайте — фильтр списка, ↑↓ Enter — выбрать");
    }
    if !second.is_empty() {
        foot.push(Line::styled(second.join(" · "), fg(MUTED)));
    }
    f.render_widget(Paragraph::new(foot), foot_area);

    if let (Some(view), Some(anchor)) = (form.picker_view(), cursor) {
        draw_picker(f, rect, anchor, &view);
    }
}

/// Text of one card suggestion row, before truncation: `◆ id  title  · column`.
fn card_row_len(c: &CardRow) -> usize {
    let epic = if c.epic { 2 } else { 0 };
    epic + c.id.chars().count() + 2 + c.label.chars().count() + 4 + c.column.chars().count()
}

/// Spans of a card row fitted into `avail` cells: id in PARAM/bold, title in FG, column in MUTED.
fn card_spans(c: &CardRow, sel: bool, avail: usize) -> Vec<Span<'static>> {
    let id_style = if sel { bold(ORANGE) } else { bold(PARAM) };
    let title_style = if sel { bold(FG) } else { fg(FG) };
    let head = (if c.epic { 2 } else { 0 }) + c.id.chars().count();
    let left = avail.saturating_sub(head + 2);
    let col_full = c.column.chars().count() + 4; // "  · " + column
    let title_len = c.label.chars().count();
    let col_w = if title_len + col_full <= left { col_full } else { col_full.min(left / 3) };
    let title_w = left.saturating_sub(col_w).min(title_len);
    let mut spans = Vec::new();
    if c.epic {
        spans.push(Span::styled("◆ ", fg(ORANGE)));
    }
    spans.push(Span::styled(c.id.clone(), id_style));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(trunc(&c.label, title_w), title_style));
    if col_w >= 5 {
        spans.push(Span::styled(trunc(&format!("  · {}", c.column), col_w), fg(MUTED)));
    }
    spans.into_iter().map(|s| s.patch_style(Style::new().bg(PANEL))).collect()
}

/// Text of one agent row, before truncation: `name  description  · tools`.
fn agent_row_len(a: &AgentRow) -> usize {
    let desc = if a.desc.is_empty() { 0 } else { 2 + a.desc.chars().count() };
    a.name.chars().count() + desc + 4 + a.tag.chars().count()
}

/// Spans of an agent row fitted into `avail`: name in PARAM/bold, description in FG, tools in MUTED.
fn agent_spans(a: &AgentRow, sel: bool, avail: usize) -> Vec<Span<'static>> {
    let name_style = if sel { bold(ORANGE) } else { bold(PARAM) };
    let tag_full = format!("  · {}", a.tag);
    let tag_len = tag_full.chars().count();
    let name_len = a.name.chars().count();
    let left = avail.saturating_sub(name_len);
    let desc_len = if a.desc.is_empty() { 0 } else { 2 + a.desc.chars().count() };
    // the tools tag survives; the description gives way first
    let tag_w = if desc_len + tag_len <= left { tag_len } else { tag_len.min(left.saturating_sub(2) / 2) };
    let desc_w = left.saturating_sub(tag_w).saturating_sub(2).min(a.desc.chars().count());
    let mut spans = vec![Span::styled(trunc(&a.name, avail), name_style)];
    if desc_w >= 2 && !a.desc.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(trunc(&a.desc, desc_w), if sel { bold(FG) } else { fg(FG) }));
    }
    if tag_w >= 5 {
        spans.push(Span::styled(trunc(&tag_full, tag_w), fg(MUTED)));
    }
    spans.into_iter().map(|s| s.patch_style(Style::new().bg(PANEL))).collect()
}

fn opt_row_len(o: &OptRow) -> usize {
    o.label.chars().count() + if o.hint.is_empty() { 0 } else { 4 + o.hint.chars().count() }
}

/// Spans of a select option row: label in FG, hint in MUTED.
fn opt_spans(o: &OptRow, sel: bool, avail: usize) -> Vec<Span<'static>> {
    let label_len = o.label.chars().count();
    let hint_full = if o.hint.is_empty() { 0 } else { 4 + o.hint.chars().count() };
    let hint_w = if label_len + hint_full <= avail { hint_full } else { hint_full.min(avail.saturating_sub(label_len) / 3) };
    let label_w = avail.saturating_sub(hint_w).min(label_len);
    let mut spans = vec![Span::styled(trunc(&o.label, label_w), if sel { bold(ORANGE) } else { fg(FG) })];
    if hint_w >= 5 {
        spans.push(Span::styled(trunc(&format!("  · {}", o.hint), hint_w), fg(MUTED)));
    }
    spans.into_iter().map(|s| s.patch_style(Style::new().bg(PANEL))).collect()
}

/// Suggestion popup (`@` files, card ids, subagents, or the filter of a long select): a small popup right under the anchor (above it when
/// there is no room). `anchor` is (x, first row below, first row above).
fn draw_picker(f: &mut Frame, form_rect: Rect, anchor: (u16, u16, u16), view: &PickerView) {
    let area = f.area();
    // a select filter without a match still shows one row saying so
    let n = if view.kind == PickKind::Select { view.results.len().max(1) } else { view.results.len() };
    let (longest, cap) = match view.kind {
        PickKind::Cards => (view.cards.iter().map(card_row_len).max().unwrap_or(0), 76),
        PickKind::Agents => (view.agents.iter().map(agent_row_len).max().unwrap_or(0), 76),
        PickKind::Select => (view.opts.iter().map(opt_row_len).max().unwrap_or(0).max(view.query.chars().count() + 4).max(14), 76),
        PickKind::Files => (view.results.iter().map(|p| p.chars().count()).max().unwrap_or(0), 60),
    };
    let w = (longest + 4).min(form_rect.width.saturating_sub(2) as usize).min(cap).max(6).min(area.width as usize) as u16;
    let below = area.bottom().saturating_sub(anchor.1) as usize;
    let above = anchor.2.saturating_sub(area.y) as usize;
    let want = n + 2;
    let (place_below, h) = if want <= below {
        (true, want)
    } else if want <= above {
        (false, want)
    } else if below >= above {
        (true, below)
    } else {
        (false, above)
    };
    if h < 3 {
        return;
    }
    let rows = (h - 2).min(n);
    let h = (rows + 2) as u16;
    let y = if place_below { anchor.1 } else { anchor.2 - h };
    let x_max = (form_rect.right().saturating_sub(1)).saturating_sub(w).max(form_rect.x);
    let x = anchor.0.min(x_max).max(form_rect.x);
    let rect = Rect::new(x, y, w.min(area.right().saturating_sub(x)), h);
    f.render_widget(Clear, rect);
    let mut block = panel_block().border_style(fg(ORANGE));
    if view.kind == PickKind::Select {
        let t = trunc(&format!("{}▏", view.query), (rect.width as usize).saturating_sub(4));
        block = block.title(Span::styled(format!(" {t} "), bold(ORANGE)));
    }
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let avail = (inner.width as usize).saturating_sub(2);
    let start = if view.selected < rows { 0 } else { view.selected + 1 - rows };
    let mut lines: Vec<Line> = Vec::new();
    if view.kind == PickKind::Select && view.results.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("  нет совпадений", fg(MUTED).bg(PANEL))), inner);
        return;
    }
    for (i, path) in view.results.iter().enumerate().skip(start).take(rows) {
        let sel = i == view.selected;
        if let Some(a) = view.agents.get(i) {
            let mut spans = vec![Span::styled(if sel { "▸ " } else { "  " }, bold(ORANGE).bg(PANEL))];
            spans.extend(agent_spans(a, sel, avail));
            lines.push(Line::from(spans));
            continue;
        }
        if let Some(o) = view.opts.get(i) {
            let mut spans = vec![Span::styled(if sel { "▸ " } else { "  " }, bold(ORANGE).bg(PANEL))];
            spans.extend(opt_spans(o, sel, avail));
            lines.push(Line::from(spans));
            continue;
        }
        if let Some(c) = view.cards.get(i) {
            let mut spans = vec![Span::styled(if sel { "▸ " } else { "  " }, bold(ORANGE).bg(PANEL))];
            spans.extend(card_spans(c, sel, avail));
            lines.push(Line::from(spans));
            continue;
        }
        let base = if sel {
            bold(ORANGE).bg(PANEL)
        } else if path.ends_with('/') {
            fg(STRING).bg(PANEL)
        } else {
            fg(FG).bg(PANEL)
        };
        let chars: Vec<char> = path.chars().collect();
        // too long: keep the tail (the file name matters most), marked with a leading ellipsis
        let skip = if chars.len() > avail { chars.len() - avail.saturating_sub(1) } else { 0 };
        let hits = files::match_positions(path, &view.query);
        let mut spans = vec![Span::styled(if sel { "▸ " } else { "  " }, base)];
        if skip > 0 {
            spans.push(Span::styled("…", base));
        }
        for (ci, &c) in chars.iter().enumerate().skip(skip) {
            let st = if !sel && hits.contains(&ci) { base.fg(PARAM) } else { base };
            spans.push(Span::styled(c.to_string(), st));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// Draws a text field; returns the screen position of the cursor when the field is focused.
fn draw_text(f: &mut Frame, r: Rect, label: &str, tb: &mut TextBuf, focused: bool) -> Option<(u16, u16)> {
    let c = if focused { ORANGE } else { MUTED };
    let block = Block::bordered()
        .border_style(fg(c))
        .title(Span::styled(format!(" {label} "), fg(c)))
        .style(Style::new().bg(PANEL).fg(FG));
    let inner = block.inner(r);
    f.render_widget(block, r);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let w = inner.width as usize;
    let h = inner.height as usize;
    tb.width = w;
    let mut cursor = None;
    if tb.multiline {
        let lines = tb.visual_lines();
        let (vr, vc) = tb.visual_pos();
        if focused {
            if vr < tb.scroll {
                tb.scroll = vr;
            } else if vr >= tb.scroll + h {
                tb.scroll = vr + 1 - h;
            }
        }
        tb.scroll = tb.scroll.min(lines.len().saturating_sub(1));
        let shown: Vec<Line> = lines.into_iter().skip(tb.scroll).take(h).map(Line::from).collect();
        f.render_widget(Paragraph::new(shown), inner);
        if focused {
            let y = inner.y + (vr - tb.scroll).min(h - 1) as u16;
            let x = inner.x + (vc as u16).min(inner.width - 1);
            f.set_cursor_position((x, y));
            cursor = Some((x, y));
        }
    } else {
        let line = &tb.lines[0];
        let start = if tb.col >= w { tb.col + 1 - w } else { 0 };
        let shown: String = line.iter().skip(start).take(w).collect();
        f.render_widget(Paragraph::new(shown), inner);
        if focused {
            let pos = (inner.x + (tb.col - start) as u16, inner.y);
            f.set_cursor_position(pos);
            cursor = Some(pos);
        }
    }
    cursor
}

fn draw_select(f: &mut Frame, r: Rect, label: &str, opts: &[Opt], idx: usize, focused: bool, label_w: usize) {
    let o = opts.get(idx);
    let val = o.map(|o| o.label.as_str()).unwrap_or("");
    let (ls, ar, vs) = if focused {
        (bold(ORANGE), fg(ORANGE), bold(FG))
    } else {
        (fg(MUTED), fg(MUTED), fg(FG))
    };
    let indent = 2 + label_w + 2;
    let val = trunc(val, (r.width as usize).saturating_sub(indent + 4));
    let line = Line::from(vec![
        Span::styled(format!("  {:<w$}", trunc(label, label_w), w = label_w), ls),
        Span::raw("  "),
        Span::styled("◀ ", ar),
        Span::styled(val, vs),
        Span::styled(" ▶", ar),
    ]);
    f.render_widget(Paragraph::new(line), Rect::new(r.x, r.y, r.width, 1));
    if r.height >= 2 {
        let hint = o.map(|o| o.hint.as_str()).unwrap_or("");
        let txt = format!("{}{}", " ".repeat(indent), trunc(hint, (r.width as usize).saturating_sub(indent)));
        let c = if focused { PARAM } else { MUTED };
        f.render_widget(Paragraph::new(Span::styled(txt, fg(c))), Rect::new(r.x, r.y + 1, r.width, 1));
    }
}

// ---------------------------------------------------------------- confirm

fn draw_confirm(f: &mut Frame, msg: &str) {
    let area = f.area();
    let w = 50u16.min(area.width.saturating_sub(2)).max(12).min(area.width);
    let tw = (w as usize).saturating_sub(4).max(1);
    // word-wrapped row count (a long message with a path wraps into more rows than chars / width)
    let lines = wrap_title(msg, tw, 64).len().max(1) as u16;
    let h = (lines + 4).min(area.height);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = panel_block().border_style(fg(ORANGE)).title(Span::styled(" Подтверждение ", bold(ORANGE)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    let [m, k] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    f.render_widget(Paragraph::new(msg.to_string()).wrap(Wrap { trim: true }), m);
    let keys = Line::from(vec![
        Span::styled("[y]", bold(ORANGE)),
        Span::styled(" да   ", fg(FG)),
        Span::styled("[n]", bold(ORANGE)),
        Span::styled(" нет", fg(FG)),
    ]);
    f.render_widget(Paragraph::new(keys).alignment(Alignment::Center), k);
}

/// One terminal row of the pixel logo: pixel rows `2r` (top) and `2r+1` (bottom) as half blocks.
/// Transparent pixels show the popup background (`PANEL`).
fn logo_row(r: usize) -> Line<'static> {
    let px = |y: usize, x: usize| {
        let c = logo::PIXELS[y * logo::W + x];
        (c != logo::TRANSPARENT).then(|| Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8))
    };
    let spans: Vec<Span> = (0..logo::W)
        .map(|x| match (px(2 * r, x), px(2 * r + 1, x)) {
            (Some(t), Some(b)) => Span::styled("▀", Style::new().fg(t).bg(b)),
            (Some(t), None) => Span::styled("▀", Style::new().fg(t).bg(PANEL)),
            (None, Some(b)) => Span::styled("▄", Style::new().fg(b).bg(PANEL)),
            (None, None) => Span::styled(" ", Style::new().bg(PANEL)),
        })
        .collect();
    Line::from(spans)
}

/// Confirm popup with the boar logo above the question (project not initialised).
/// Falls back to the plain confirm when the terminal is too small.
fn draw_confirm_init(f: &mut Frame, msg: &str) {
    let area = f.area();
    let art_h = (logo::H / 2) as u16;
    let art_w = logo::W as u16;
    if area.height < art_h + 8 || area.width < art_w + 4 {
        return draw_confirm(f, msg);
    }
    // the question carries a project path: a wider popup keeps it short
    let w = (art_w + 6).max(60).min(area.width);
    let tw = (w as usize).saturating_sub(4).max(1);
    // +1: ratatui's word wrap may need a row more than ours
    let lines = wrap_title(msg, tw, 64).len().max(1) as u16 + 1;
    // borders 2 + art + title + blank + message + keys
    let h = art_h + 5 + lines;
    if h > area.height {
        return draw_confirm(f, msg);
    }
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = panel_block().border_style(fg(ORANGE)).title(Span::styled(" Подтверждение ", bold(ORANGE)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    let [a, t, _, m, k] = Layout::vertical([
        Constraint::Length(art_h),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    let art_lines: Vec<Line> = (0..logo::H / 2).map(logo_row).collect();
    let art_rect = centered(a, art_w, art_h);
    f.render_widget(Paragraph::new(art_lines), art_rect);
    let title = Line::from(Span::styled(logo::TITLE, bold(ORANGE)));
    f.render_widget(Paragraph::new(title).alignment(Alignment::Center), t);
    f.render_widget(Paragraph::new(msg.to_string()).wrap(Wrap { trim: true }).alignment(Alignment::Center), m);
    let keys = Line::from(vec![
        Span::styled("[y]", bold(ORANGE)),
        Span::styled(" да   ", fg(FG)),
        Span::styled("[n]", bold(ORANGE)),
        Span::styled(" нет", fg(FG)),
    ]);
    f.render_widget(Paragraph::new(keys).alignment(Alignment::Center), k);
}

// ------------------------------------------------------------- tool picker

const TOOLS_QUESTION: &str = "Для каких инструментов установить агентов?";

fn draw_init_tools(f: &mut Frame, p: &ToolPick) {
    let area = f.area();
    let w = 64u16.min(area.width);
    let tw = (w as usize).saturating_sub(4).max(1);
    let q_h = wrap_title(TOOLS_QUESTION, tw, 4).len().max(1) as u16;
    let hint_h: u16 = if tw >= 52 { 2 } else { 4 };
    let n = p.items.len().max(1) as u16;
    // borders 2 + question + blank + rows + blank + hints
    let h = (q_h + n + hint_h + 4).min(area.height);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = panel_block().border_style(fg(ORANGE)).title(Span::styled(" Агенты и скиллы ", bold(ORANGE)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    let [q, _, list, _, hints] = Layout::vertical([
        Constraint::Length(q_h),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(hint_h),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(TOOLS_QUESTION).style(bold(FG)).wrap(Wrap { trim: true }), q);

    let vh = list.height as usize;
    let off = if vh > 0 && p.cursor >= vh { p.cursor + 1 - vh } else { 0 };
    let mut lines: Vec<Line> = Vec::new();
    for (i, (t, found)) in p.items.iter().enumerate().skip(off).take(vh) {
        let on = i == p.cursor;
        let base = if on { Style::new().bg(BG).fg(FG) } else { Style::new() };
        let checked = p.checked.get(i).copied().unwrap_or(false);
        let mut sp = vec![
            Span::styled(if on { "▶ " } else { "  " }, bold(ORANGE)),
            Span::styled(if checked { "[x] " } else { "[ ] " }, if checked { base.fg(GREEN) } else { base.fg(MUTED) }),
            Span::styled(t.name.to_string(), base),
        ];
        if *found {
            sp.push(Span::styled("  найден в проекте", base.fg(PURPLE)));
        }
        lines.push(Line::from(sp));
    }
    f.render_widget(Paragraph::new(lines), list);

    let end = if p.creating { "Esc: только .pumba/, без агентов" } else { "Esc: закрыть" };
    let hint = vec![
        Line::from(vec![
            Span::styled("↑↓", fg(ORANGE)),
            Span::styled(" выбор  ", fg(MUTED)),
            Span::styled("Пробел", fg(ORANGE)),
            Span::styled(" отметить  ", fg(MUTED)),
            Span::styled("a", fg(ORANGE)),
            Span::styled(" все/никого", fg(MUTED)),
        ]),
        Line::from(vec![Span::styled("Enter", fg(ORANGE)), Span::styled(format!(" установить  {end}"), fg(MUTED))]),
    ];
    f.render_widget(Paragraph::new(hint).wrap(Wrap { trim: true }), hints);
}

// ------------------------------------------------------------------ trash

fn draw_trash(f: &mut Frame, app: &App, sel: usize) {
    let area = f.area();
    let rows = app.trashed();
    let w = area.width.saturating_sub(4).clamp(10, 96).min(area.width);
    let h = (rows.len().max(1) as u16 + 4).clamp(7, 22).min(area.height);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let bottom = if w >= 54 { " ↑↓ выбор   r/Enter восстановить   Esc закрыть " } else { " r - вернуть  Esc " };
    let block = panel_block()
        .border_style(fg(ORANGE))
        .title(Span::styled(format!(" Корзина ({}) · UTC ", rows.len()), bold(ORANGE)))
        .title_bottom(Span::styled(bottom, fg(MUTED)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    if rows.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("Корзина пуста", italic(MUTED))), inner);
        return;
    }
    let iw = inner.width as usize;
    let vh = inner.height as usize;
    let sel = sel.min(rows.len() - 1);
    let off = if vh > 0 && sel >= vh { sel + 1 - vh } else { 0 };
    let mut lines: Vec<Line> = Vec::new();
    for (k, &ti) in rows.iter().enumerate().skip(off).take(vh) {
        let t = &app.tasks[ti];
        let col = app.cfg.columns.get(app.effective_col(t)).map_or("?", |c| c.name.as_str());
        let when = t.deleted_at.map_or("—".to_string(), crate::model::fmt_utc);
        let date = format!("  · удалена: {when}");
        let colseg = format!("  · {}", trunc(col, 14));
        let head = 2 + t.id.chars().count() + 2;
        // the column segment is the first to go when the title would get too little room
        let colseg = if iw >= head + date.chars().count() + colseg.chars().count() + 10 { colseg } else { String::new() };
        let tw = iw.saturating_sub(head + date.chars().count() + colseg.chars().count());
        let title = trunc(&t.title, tw);
        let pad = tw.saturating_sub(title.chars().count());
        let on = k == sel;
        let base = if on { Style::new().bg(BG).fg(FG) } else { Style::new() };
        lines.push(Line::from(vec![
            Span::styled(if on { "▶ " } else { "  " }, bold(ORANGE)),
            Span::styled(t.id.clone(), base.fg(if on { ORANGE } else { MUTED })),
            Span::styled("  ", base),
            Span::styled(title, base),
            Span::styled(" ".repeat(pad), base),
            Span::styled(colseg, base.fg(PURPLE)),
            Span::styled(date, base.fg(MUTED)),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

// ------------------------------------------------------------------- help

type HelpGroup = (&'static str, &'static [(&'static str, &'static str)]);

const HELP_LEFT: &[HelpGroup] = &[
    (
        "Карточки",
        &[
            ("n", "новая задача"),
            ("N", "новый эпик"),
            ("Enter / e", "правка карточки"),
            ("зависим.", "поле «Зависит от» в форме правки"),
            ("блокирует", "поле «Блокирует» в форме правки"),
            ("d", "в корзину"),
            ("x", "скрыть / показать"),
            ("p", "в спринт / из спринта"),
            ("H / L", "в пред. / след. колонку"),
            ("J / K", "порядок вниз / вверх"),
        ],
    ),
    (
        "Колонки",
        &[
            ("c", "новая колонка (перед «Готово»)"),
            ("r", "правка колонки и правила"),
            ("X", "удалить колонку"),
            ("Ctrl/Alt+←/→", "колонку влево / вправо (< >)"),
            ("закреплены", "Бэклог и Готово закреплены"),
            ("ЙЦУКЕН", "рус. раскладка работает"),
        ],
    ),
];

const HELP_RIGHT: &[HelpGroup] = &[
    (
        "Вид",
        &[
            ("h / l", "выбор колонки"),
            ("j / k", "выбор карточки"),
            ("v", "показать / скрыть скрытые"),
            ("s", "фильтр: только спринт"),
            ("?", "эта справка"),
            ("q", "выход"),
        ],
    ),
    (
        "Проект",
        &[
            ("]", "начать след. спринт"),
            ("P", "правила для агентов"),
            ("M", "модели и уровни"),
            ("@", "вставить ссылку на файл"),
            ("B", "корзина (восстановить)"),
            ("I", "агенты и скиллы (инструменты)"),
        ],
    ),
];

fn help_lines(groups: &[HelpGroup]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for (gi, (title, items)) in groups.iter().enumerate() {
        if gi > 0 {
            out.push(Line::raw(""));
        }
        out.push(Line::styled(title.to_string(), bold(PINK)));
        for (k, d) in items.iter() {
            out.push(Line::from(vec![
                Span::styled(format!(" {:<10}", k), fg(ORANGE)),
                Span::styled(d.to_string(), fg(PARAM)),
            ]));
        }
    }
    out
}

fn draw_help(f: &mut Frame) {
    let area = f.area();
    let w = area.width.saturating_sub(4).min(92).max(10).min(area.width);
    let h = 21u16.min(area.height);
    let rect = centered(area, w, h);
    f.render_widget(Clear, rect);
    let block = panel_block()
        .border_style(fg(ORANGE))
        .title(Span::styled(" Клавиши ", bold(ORANGE)))
        .title_bottom(Span::styled(" любая клавиша - закрыть ", fg(MUTED)));
    let inner = block.inner(rect).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(block, rect);
    let [a, b] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);
    f.render_widget(Paragraph::new(help_lines(HELP_LEFT)), a);
    f.render_widget(Paragraph::new(help_lines(HELP_RIGHT)), b);
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Store, Task};
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
}
