//! Tool picker of the agent pack: the init flow (new board) and `I` on an existing board.
use super::{Action, App, Mode, latin_key};
use pumba_install::install::{self, Tool, ToolInfo};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// State of the tool checklist popup.
pub struct ToolPick {
    /// Ready tools of the registry and whether each was found in the project.
    pub items: Vec<(ToolInfo, bool)>,
    pub checked: Vec<bool>,
    pub cursor: usize,
    /// True while `.pumba/` is still to be created (the init flow); false for an existing board.
    pub creating: bool,
}

impl ToolPick {
    pub fn chosen(&self) -> Vec<Tool> {
        self.items.iter().zip(&self.checked).filter(|(_, c)| **c).map(|((i, _), _)| i.tool).collect()
    }
}

impl App {
    /// Checklist of ready tools. New board: found in the project (else Claude Code); existing
    /// board: the saved choice (same default when nothing was saved yet).
    pub(super) fn tool_pick(&self, creating: bool) -> ToolPick {
        let root = self.root();
        let found = install::detect(&root);
        let saved = if creating { Vec::new() } else { install::from_ids(&self.cfg.tools) };
        let pre = if saved.is_empty() { install::preselect(&root) } else { saved };
        let items: Vec<(ToolInfo, bool)> = install::ready_tools().into_iter().map(|t| (t.clone(), found.contains(&t.tool))).collect();
        let checked = items.iter().map(|(t, _)| pre.contains(&t.tool)).collect();
        ToolPick { items, checked, cursor: 0, creating }
    }

    /// Install result: the summary, or the first warning (and «и ещё N») followed by the summary.
    fn report_status(&mut self, summary: String, warnings: &[String]) {
        match install::warnings_line(warnings) {
            Some(w) => self.warn(format!("Внимание: {w} · {summary}")),
            None => self.info(summary),
        }
    }

    // ------------------------------------------------------------- tools

    pub(super) fn tools_key(&mut self, mut p: ToolPick, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(latin_key(c)),
            other => other,
        };
        let n = p.items.len();
        match code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            _ if ctrl => self.mode = Mode::InitTools(p),
            // new board: only `.pumba/`, no agents; existing board: just close
            KeyCode::Esc if p.creating => self.create_board(&[]),
            KeyCode::Esc => {}
            KeyCode::Enter => {
                let tools = p.chosen();
                if p.creating { self.create_board(&tools) } else { self.install_tools(&tools) }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                p.cursor = p.cursor.saturating_sub(1);
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                p.cursor = (p.cursor + 1).min(n.saturating_sub(1));
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Char(' ') => {
                if let Some(c) = p.checked.get_mut(p.cursor) {
                    *c = !*c;
                }
                self.mode = Mode::InitTools(p);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all = p.checked.iter().all(|&c| c);
                p.checked.iter_mut().for_each(|c| *c = !all);
                self.mode = Mode::InitTools(p);
            }
            _ => self.mode = Mode::InitTools(p),
        }
    }

    /// Init flow: create `.pumba/`, remember the chosen tools, install their packs.
    fn create_board(&mut self, tools: &[Tool]) {
        if let Err(e) = self.store.init() {
            self.mode = Mode::Confirm(self.init_msg(), Action::Init);
            self.err(format!("Не удалось создать .pumba/: {e}"));
            return;
        }
        // .pumba/ stays created even when the installer fails
        let root = self.root();
        let saved = self.store.set_tools(&install::to_ids(tools));
        let report = if tools.is_empty() { Ok(install::InstallReport::default()) } else { install::install(&root, tools, false) };
        self.reload();
        if !self.ok {
            return;
        }
        match (report, saved) {
            (Ok(_), Ok(())) if tools.is_empty() => self.info("Создано. Агенты не установлены (установить: I)"),
            (Ok(r), Ok(())) => {
                self.report_status(format!("Создано. Установлено: {} файлов, пропущено: {}", r.installed.len(), r.skipped.len()), &r.warnings)
            }
            (Err(e), _) => self.err(format!("Папка .pumba/ создана, установка не удалась: {e}")),
            (_, Err(e)) => self.err(format!("Папка .pumba/ создана, выбор не записан в board.json: {e}")),
        }
    }

    /// `I` on an existing board: install for the chosen tools (existing files are kept) and save the choice.
    fn install_tools(&mut self, tools: &[Tool]) {
        let report = if tools.is_empty() { Ok(install::InstallReport::default()) } else { install::install(&self.root(), tools, false) };
        let r = match report {
            Ok(r) => r,
            Err(e) => return self.err(format!("Установка не удалась: {e}")),
        };
        let ids = install::to_ids(tools);
        if self.cfg.tools != ids {
            self.cfg.tools = ids;
            if !self.save(&[], true) {
                return;
            }
        }
        if tools.is_empty() {
            self.info("Инструменты не выбраны, ничего не установлено");
        } else {
            self.report_status(format!("Установлено: {} файлов, пропущено: {}", r.installed.len(), r.skipped.len()), &r.warnings);
        }
    }
}
