use super::util::*;
use crate::app::App;
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

// ----------------------------------------------------------------- header

pub(super) fn draw_header(f: &mut Frame, area: Rect, app: &App) {
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

pub(super) fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
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
