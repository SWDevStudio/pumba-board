use super::util::*;
use crate::theme::*;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph};

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

pub(super) fn draw_help(f: &mut Frame) {
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
