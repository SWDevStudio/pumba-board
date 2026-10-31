use super::util::*;
use crate::app::App;
use crate::theme::*;
use pumba_core::model::{Kind, Task};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

/// (agent, level) for a card header. Default model shows as `авто`; default
/// effort is empty. Both default -> both empty.
pub(super) fn agent_level(t: &Task) -> (String, String) {
    let is_def = |s: &str| s.is_empty() || s == "default";
    if is_def(&t.model) && is_def(&t.effort) {
        return (String::new(), String::new());
    }
    let agent = if is_def(&t.model) { "авто".to_string() } else { t.model.clone() };
    let level = if is_def(&t.effort) { String::new() } else { t.effort.clone() };
    (agent, level)
}

pub(super) fn draw_card(f: &mut Frame, r: Rect, app: &App, t: &Task, selected: bool) {
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

pub(super) fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
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
fn rule_lines(col: Option<&pumba_core::model::Column>) -> Vec<Line<'static>> {
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
