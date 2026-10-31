use super::util::*;
use crate::app::{App, ToolPick};
use crate::logo;
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph, Wrap};

// ---------------------------------------------------------------- confirm

pub(super) fn draw_confirm(f: &mut Frame, msg: &str) {
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
pub(super) fn draw_confirm_init(f: &mut Frame, msg: &str) {
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

pub(super) fn draw_init_tools(f: &mut Frame, p: &ToolPick) {
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

pub(super) fn draw_trash(f: &mut Frame, app: &App, sel: usize) {
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
        let when = t.deleted_at.map_or("—".to_string(), pumba_core::model::fmt_utc);
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
