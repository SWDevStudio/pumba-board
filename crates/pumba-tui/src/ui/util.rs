use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType};

const MIN_COL_W: u16 = 30;

pub(super) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub(super) fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

pub(super) fn italic(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::ITALIC)
}

/// Truncate by chars, ending with `…` when cut.
pub(super) fn trunc(s: &str, w: usize) -> String {
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

pub(super) fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

pub(super) fn vis_cols(width: u16, n: usize) -> usize {
    ((width / MIN_COL_W) as usize).max(1).min(n.max(1))
}


/// Word-wrap `s` by chars to `w` columns, at most `max` lines; the last line
/// is truncated with `…` when text remains.
pub(super) fn wrap_title(s: &str, w: usize, max: usize) -> Vec<String> {
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

pub(super) fn panel_block() -> Block<'static> {
    Block::bordered().border_type(BorderType::Rounded).style(Style::new().bg(PANEL).fg(FG))
}
