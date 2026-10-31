//! All drawing. Pure view code: reads `App`, only adjusts scroll offsets.
use crate::app::{Action, App, Mode};
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::Block;

mod board;
mod card;
mod chrome;
mod form;
mod help;
mod picker;
mod popups;
#[cfg(test)]
mod tests;
mod util;

use board::draw_columns;
use card::draw_detail;
use chrome::{draw_footer, draw_header};
use form::draw_form;
use help::draw_help;
use popups::{draw_confirm, draw_confirm_init, draw_init_tools, draw_trash};

const DETAIL_H: u16 = 9;

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
