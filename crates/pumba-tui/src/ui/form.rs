use super::picker::draw_picker;
use super::util::*;
use crate::editor::TextBuf;
use crate::form::{FieldKind, Form, Opt, Purpose, Suggest, FILTER_MIN_OPTS};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph};

const LABEL_W: usize = 16;

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

pub(super) fn draw_form(f: &mut Frame, form: &mut Form) {
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

/// Draws a text field; returns the screen position of the cursor when the field is focused.
pub(super) fn draw_text(f: &mut Frame, r: Rect, label: &str, tb: &mut TextBuf, focused: bool) -> Option<(u16, u16)> {
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

pub(super) fn draw_select(f: &mut Frame, r: Rect, label: &str, opts: &[Opt], idx: usize, focused: bool, label_w: usize) {
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
