use super::util::*;
use crate::files;
use crate::form::{AgentRow, CardRow, OptRow, PickKind, PickerView};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph};

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
pub(super) fn draw_picker(f: &mut Frame, form_rect: Rect, anchor: (u16, u16, u16), view: &PickerView) {
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
