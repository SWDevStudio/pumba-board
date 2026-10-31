//! Dependency fields of a card: «Зависит от» (`depends:`, stored) and «Блокирует» (the reverse
//! edges, stored only as `depends:` of the other cards). Parsing, validation and cycle checks.
use super::Board;

/// Ids split by commas/whitespace, empty parts skipped, duplicates dropped in order.
fn split_ids(raw: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for d in raw.split(|c: char| c == ',' || c.is_whitespace()).filter(|d| !d.is_empty()) {
        if !ids.iter().any(|x| x == d) {
            ids.push(d.to_string());
        }
    }
    ids
}

impl Board {
    /// Parse and validate a dependency list (ids split by commas/spaces, duplicates dropped silently).
    /// Every id must exist and not be the card itself (cycles are checked later over both fields).
    pub fn parse_depends(&self, raw: &str, edit: Option<&str>) -> Result<Vec<String>, String> {
        let kept: Vec<String> = edit
            .and_then(|id| self.tasks.iter().find(|t| t.id == id))
            .map(|t| t.depends.clone())
            .unwrap_or_default();
        let deps = split_ids(raw);
        for d in &deps {
            if edit == Some(d.as_str()) {
                return Err(format!("{d} не может зависеть от самой себя"));
            }
            match self.tasks.iter().find(|t| t.id == *d) {
                None => return Err(format!("Зависимость {d}: такой карточки нет")),
                // a trashed card cannot be newly picked; an entry that is already stored stays as is
                Some(t) if t.deleted && !kept.contains(d) => return Err(format!("Зависимость {d}: карточка в корзине")),
                Some(_) => {}
            }
        }
        Ok(deps)
    }

    /// Parse and validate the «Блокирует» list: cards that will depend on this one. Same rules as
    /// `parse_depends`, plus: an id cannot also sit in `depends` of the same form. For a new card
    /// (`edit` is None) there is no id yet, so the self check cannot trigger.
    pub fn parse_blocks(&self, raw: &str, edit: Option<&str>, depends: &[String]) -> Result<Vec<String>, String> {
        let blocks = split_ids(raw);
        for b in &blocks {
            if edit == Some(b.as_str()) {
                return Err(format!("{b} не может блокировать саму себя"));
            }
            if depends.contains(b) {
                return Err(format!("{b}: нельзя одновременно зависеть от карточки и блокировать её"));
            }
            match self.tasks.iter().find(|t| t.id == *b) {
                None => return Err(format!("Блокирует {b}: такой карточки нет")),
                // a trashed card cannot be newly picked; one that already depends on this card stays as is
                Some(t) if t.deleted && !edit.is_some_and(|me| t.depends.iter().any(|d| d == me)) => {
                    return Err(format!("Блокирует {b}: карточка в корзине"))
                }
                Some(_) => {}
            }
        }
        Ok(blocks)
    }

    /// Reject the combination of this card's new `depends` and the reverse edges (`added` dependents
    /// get `me`, `removed` ones lose it). Every new edge touches `me`, so a new cycle exists iff `me`
    /// is reachable from one of its new dependencies in the resulting graph.
    pub fn check_cycles(&self, me: &str, deps: &[String], added: &[String], removed: &[String]) -> Result<(), String> {
        let next = |id: &str| -> Vec<String> {
            if id == me {
                return deps.to_vec();
            }
            let mut d = self.tasks.iter().find(|t| t.id == id).map(|t| t.depends.clone()).unwrap_or_default();
            if added.iter().any(|a| a == id) {
                d.push(me.to_string());
            }
            if removed.iter().any(|r| r == id) {
                d.retain(|x| x != me);
            }
            d
        };
        for d in deps {
            let mut seen: Vec<String> = Vec::new();
            let mut path = vec![me.to_string()];
            if let Some(p) = find_path(d, me, &next, &mut seen, &mut path) {
                return Err(format!("Цикл зависимостей: {}", p.join(" → ")));
            }
        }
        Ok(())
    }
}

/// DFS along `next` edges from `cur` looking for `target`; returns the full path when found.
fn find_path(
    cur: &str,
    target: &str,
    next: &dyn Fn(&str) -> Vec<String>,
    seen: &mut Vec<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    path.push(cur.to_string());
    if cur == target {
        return Some(path.clone());
    }
    if !seen.iter().any(|s| s == cur) {
        seen.push(cur.to_string());
        for n in next(cur) {
            if let Some(p) = find_path(&n, target, next, seen, path) {
                return Some(p);
            }
        }
    }
    path.pop();
    None
}
