use super::card::draw_card;
use super::util::*;
use crate::app::App;
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

pub(super) const CARD_H: u16 = 6;

// ---------------------------------------------------------------- columns

pub(super) fn draw_columns(f: &mut Frame, area: Rect, app: &mut App) {
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

pub(super) fn draw_column(f: &mut Frame, r: Rect, ci: usize, app: &mut App) {
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
