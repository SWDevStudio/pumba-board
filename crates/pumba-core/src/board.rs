//! Board state and every board operation, without any UI: load, merge with external edits,
//! commit with rollback, visibility, trash, dependencies, columns. The TUI keeps only screen
//! state (selection, forms, statuses) on top of it; a CLI can drive the same API.
use crate::model::{Config, Kind, Store, Task};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod deps;
mod edit;
#[cfg(test)]
mod tests;

pub use edit::{ColumnInput, ModelsInput, TaskInput};

/// Display filters of the board (the TUI toggles them with `v` and `s`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// Show cards marked hidden.
    pub show_hidden: bool,
    /// Show only cards of the current sprint (and epics with a child in it).
    pub sprint_only: bool,
}

pub struct Board {
    pub store: Store,
    pub cfg: Config,
    pub tasks: Vec<Task>,
    /// `Store::signature` as of the last load or write: a different value means the board was
    /// changed on disk behind our back.
    pub sig: u128,
    /// False while board.json is missing or failed to load: nothing is written then.
    pub ok: bool,
    /// Card file names that failed to read or parse at the last load.
    pub unreadable: Vec<String>,
    /// How many times AGENTS.md was written by this board (tests watch it for write loops).
    pub agents_writes: u32,
    /// Raw text of every card file as this board last loaded or wrote it: the base for merging
    /// with edits made on disk behind our back.
    seen: HashMap<String, String>,
    /// Cards whose body was changed on disk since `seen`, noted by the last commit: (id, disk body kept).
    pub ext: Vec<(String, bool)>,
}

impl Board {
    /// Board of the project at `root` (its `.pumba/` is `root/.pumba`). Nothing is loaded yet:
    /// call `reload`; until a load succeeds nothing is written.
    pub fn new(root: &Path) -> Board {
        let store = Store::new(root);
        Board {
            sig: store.signature(),
            store,
            cfg: Config::default(),
            tasks: vec![],
            ok: false,
            unreadable: Vec::new(),
            agents_writes: 0,
            seen: HashMap::new(),
            ext: Vec::new(),
        }
    }

    /// Project root: the folder that holds `.pumba/`.
    pub fn root(&self) -> PathBuf {
        self.store.dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.store.dir.clone())
    }

    // ----------------------------------------------------------- queries

    /// Index of the column the card sits in; a card of an unknown column shows in the first one.
    pub fn effective_col(&self, t: &Task) -> usize {
        self.cfg.columns.iter().position(|c| c.id == t.column).unwrap_or(0)
    }

    /// Whether the card shows on the board under the filters of `view`. Trashed cards never do.
    pub fn visible(&self, t: &Task, view: View) -> bool {
        if t.deleted {
            return false;
        }
        if t.hidden && !view.show_hidden {
            return false;
        }
        if view.sprint_only {
            let cur = Some(self.cfg.sprint);
            return t.sprint == cur
                || (t.kind == Kind::Epic
                    && self.tasks.iter().any(|c| !c.deleted && c.parent.as_deref() == Some(t.id.as_str()) && c.sprint == cur));
        }
        true
    }

    /// Indices of the visible cards of column `ci` in board order (`order`, then id).
    pub fn column_tasks(&self, ci: usize, view: View) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.tasks.len())
            .filter(|&i| self.effective_col(&self.tasks[i]) == ci && self.visible(&self.tasks[i], view))
            .collect();
        v.sort_by(|&a, &b| {
            let (a, b) = (&self.tasks[a], &self.tasks[b]);
            a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id))
        });
        v
    }

    /// Index of the card with this id.
    pub fn find(&self, id: &str) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    /// Indices of trashed cards, newest deleted first (no timestamp sorts last, ties by id).
    pub fn trashed(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.tasks.len()).filter(|&i| self.tasks[i].deleted).collect();
        v.sort_by(|&a, &b| {
            let (a, b) = (&self.tasks[a], &self.tasks[b]);
            b.deleted_at.unwrap_or(0).cmp(&a.deleted_at.unwrap_or(0)).then_with(|| a.id.cmp(&b.id))
        });
        v
    }

    /// (done, total) children of an epic; done means in the last column. Trashed children are not counted.
    pub fn epic_progress(&self, epic_id: &str) -> (usize, usize) {
        let last = self.cfg.columns.len().saturating_sub(1);
        let kids = self.tasks.iter().filter(|t| !t.deleted && t.parent.as_deref() == Some(epic_id));
        kids.fold((0, 0), |(done, total), t| (done + (self.effective_col(t) == last) as usize, total + 1))
    }

    /// Dependencies of `t` that exist and are not in the last (done) column. Unknown and trashed ids are ignored.
    pub fn blockers(&self, t: &Task) -> Vec<&Task> {
        let last = self.cfg.columns.len().saturating_sub(1);
        t.depends
            .iter()
            .filter_map(|d| self.tasks.iter().find(|x| x.id == *d))
            .filter(|x| !x.deleted && self.effective_col(x) != last)
            .collect()
    }

    /// Non-trashed cards that list `id` in their `depends`, sorted by id. Derived on demand:
    /// the dependent card's `depends:` is the only stored side of the edge.
    pub fn dependents(&self, id: &str) -> Vec<&Task> {
        let mut v: Vec<&Task> = self.tasks.iter().filter(|t| !t.deleted && t.depends.iter().any(|d| d == id)).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// `order` for a card added at the end of column `ci`.
    pub fn next_order(&self, ci: usize) -> i64 {
        self.tasks.iter().filter(|t| self.effective_col(t) == ci).map(|t| t.order).max().map_or(0, |m| m + 1)
    }

    /// The first (backlog) and the last (done) columns are pinned by position, not by id.
    pub fn is_pinned(&self, i: usize) -> bool {
        i == 0 || i + 1 == self.cfg.columns.len()
    }

    /// Id for a new card of `kind`: one past the highest number in use (trashed cards count).
    pub fn new_id(&self, kind: Kind) -> String {
        let p = kind.prefix();
        let max = self.tasks.iter().filter_map(|t| t.id.strip_prefix(p)?.parse::<u32>().ok()).max().unwrap_or(0);
        // an id whose file already exists (a card created outside and not loaded yet) is skipped
        let mut n = max + 1;
        while self.store.task_exists(&format!("{p}{n:03}")) {
            n += 1;
        }
        format!("{p}{n:03}")
    }

    /// One-line warning about card files that could not be read: first 3 names plus the rest as a count.
    pub fn unreadable_note(&self) -> Option<String> {
        if self.unreadable.is_empty() {
            return None;
        }
        let names = self.unreadable.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        let rest = self.unreadable.len().saturating_sub(3);
        Some(if rest > 0 { format!("Не прочитано: {names} и ещё {rest}") } else { format!("Не прочитано: {names}") })
    }

    // ------------------------------------------------------- persistence

    /// The board files changed on disk since the last load or write.
    pub fn changed_on_disk(&self) -> bool {
        self.store.signature() != self.sig
    }

    /// Reload from disk. A failed load disables writing: Err with the message. When the loaded
    /// state differs from the previous (successfully loaded) one, AGENTS.md is regenerated so the
    /// index follows external edits; the signature only covers board.json and the cards, so that
    /// write does not trigger another reload. Ok(Some(msg)): loaded, but that AGENTS.md write failed.
    pub fn reload(&mut self) -> Result<Option<String>, String> {
        let was_ok = self.ok;
        let r = match self.store.load_full() {
            Ok(l) => {
                let mut cfg = l.cfg;
                if cfg.columns.is_empty() {
                    cfg.columns = Config::default().columns;
                }
                let changed = was_ok && (cfg != self.cfg || l.tasks != self.tasks);
                self.cfg = cfg;
                self.tasks = l.tasks;
                self.seen = l.raws;
                self.unreadable = l.unreadable;
                self.ok = true;
                let mut warn = None;
                if changed {
                    match self.store.write_agents(&self.cfg, &self.tasks) {
                        Ok(()) => self.agents_writes += 1,
                        Err(e) => warn = Some(format!("Не удалось обновить AGENTS.md: {e}")),
                    }
                }
                Ok(warn)
            }
            Err(e) => {
                self.ok = false;
                self.unreadable.clear();
                Err(format!("Не удалось загрузить .pumba: {e}"))
            }
        };
        self.sig = self.store.signature();
        r
    }

    /// Before a card is rewritten from memory: when its file changed on disk since this board last
    /// loaded or wrote it, merge (see `Task::merge_disk`): fields this operation did not change take
    /// the disk value, in particular the body of a card that was only moved, reordered, hidden, ...
    fn merge_external(&mut self, save: &[String]) -> Result<(), String> {
        self.ext.clear();
        for id in save {
            let Some(i) = self.tasks.iter().position(|t| t.id == *id) else { continue };
            match (self.store.read_task_raw(id), self.seen.get(id)) {
                (Some(disk), Some(base)) if disk != *base => {
                    let (Some(dt), Some(bt)) = (Task::from_md(&disk, id), Task::from_md(base, id)) else {
                        return Err(format!("{id}.md изменён снаружи и не читается; ничего не записано"));
                    };
                    if dt.body != bt.body {
                        let ours_changed = self.tasks[i].body != bt.body;
                        self.ext.push((id.clone(), !ours_changed));
                    }
                    self.tasks[i].merge_disk(&bt, &dt);
                }
                (Some(_), None) => return Err(format!("{id}.md уже есть на диске (создан снаружи); обновите доску")),
                _ => {}
            }
        }
        Ok(())
    }

    /// Write the cards `save` (and board.json when `cfg`), then AGENTS.md; task files are never
    /// removed. Cards changed on disk meanwhile are merged first (see `merge_external`); a failure
    /// part-way puts every touched file back as it was. Ok(true): something else touched the board
    /// since our last load, so the caller must `reload` to resync the rest of it (`commit` does).
    pub fn write(&mut self, save: &[String], cfg: bool) -> Result<bool, String> {
        if !self.ok {
            return Err("board.json не удалось загрузить; сначала исправьте его (ничего не записано)".into());
        }
        // something else touched the board since our last load: resync the rest of it after writing
        let external = self.store.signature() != self.sig;
        self.merge_external(save)?;
        // Remember every file about to be written so a failure part-way leaves the disk as it was.
        let mut paths: Vec<PathBuf> = save.iter().map(|id| self.store.dir.join("tasks").join(format!("{id}.md"))).collect();
        if cfg {
            paths.push(self.store.dir.join("board.json"));
        }
        paths.push(self.store.dir.join("AGENTS.md"));
        let before: Vec<(PathBuf, Option<Vec<u8>>)> =
            paths.into_iter().map(|p| { let old = std::fs::read(&p).ok(); (p, old) }).collect();
        let r = (|| -> crate::model::Res<()> {
            for t in self.tasks.iter().filter(|t| save.contains(&t.id)) {
                self.store.save_task(t)?;
            }
            if cfg {
                self.store.save_config(&self.cfg)?;
            }
            self.store.write_agents(&self.cfg, &self.tasks)?;
            Ok(())
        })();
        if r.is_err() {
            for (p, old) in &before {
                let _ = match old {
                    Some(bytes) => std::fs::write(p, bytes),
                    None => std::fs::remove_file(p),
                };
            }
        } else {
            self.agents_writes += 1;
            for t in self.tasks.iter().filter(|t| save.contains(&t.id)) {
                self.seen.insert(t.id.clone(), t.to_md());
            }
        }
        self.sig = self.store.signature();
        r.map_err(|e| format!("Ошибка записи: {e}"))?;
        Ok(external)
    }

    /// `write`, then a `reload` when the board was changed on disk behind our back. A failed
    /// resync does not fail the commit (the files are written): `ok` tells.
    pub fn commit(&mut self, save: &[String], cfg: bool) -> Result<(), String> {
        if self.write(save, cfg)? {
            let _ = self.reload();
        }
        Ok(())
    }
}
