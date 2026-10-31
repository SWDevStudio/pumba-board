//! Card suggestions for the dependency field: search by id, title, description or column name.
use crate::model::Kind;

/// A card as the suggestion popup sees it. Lowercased copies are kept for matching.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardRef {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    /// Display name of the column the card sits in.
    pub column: String,
    id_l: String,
    title_l: String,
    body_l: String,
    col_l: String,
}

impl CardRef {
    pub fn new(id: &str, kind: Kind, title: &str, column: &str, body: &str) -> Self {
        CardRef {
            id: id.to_string(),
            kind,
            title: title.to_string(),
            column: column.to_string(),
            id_l: id.to_lowercase(),
            title_l: title.to_lowercase(),
            body_l: body.to_lowercase(),
            col_l: column.to_lowercase(),
        }
    }

    /// Lower is better; None when the card does not match `q` (already lowercase, non-empty).
    fn tier(&self, q: &str) -> Option<u8> {
        if self.id_l.starts_with(q) {
            Some(0)
        } else if self.id_l.contains(q) {
            Some(1)
        } else if self.title_l.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(q)) {
            Some(2)
        } else if self.title_l.contains(q) {
            Some(3)
        } else if self.body_l.contains(q) || self.col_l.contains(q) {
            Some(4)
        } else if q.chars().count() >= 2 && (subseq(&self.title_l, q) || subseq(&self.id_l, q)) {
            Some(5)
        } else {
            None
        }
    }
}

/// True when the chars of `q` appear in `hay` in order.
fn subseq(hay: &str, q: &str) -> bool {
    let mut it = hay.chars();
    q.chars().all(|c| it.any(|h| h == c))
}

/// Up to `limit` best cards for `query`, skipping ids in `skip` (compared case-insensitively).
/// Ranking: id prefix, id substring, title word prefix, title substring, body/column, fuzzy
/// title; ties by id.
pub fn search<'a>(cards: &'a [CardRef], query: &str, skip: &[String], limit: usize) -> Vec<&'a CardRef> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &CardRef)> = cards
        .iter()
        .filter(|c| !skip.iter().any(|s| *s == c.id_l))
        .filter_map(|c| c.tier(&q).map(|t| (t, c)))
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    hits.into_iter().take(limit).map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards() -> Vec<CardRef> {
        vec![
            CardRef::new("E-001", Kind::Epic, "Авторизация", "В работе", "Большой эпик про вход"),
            CardRef::new("T-001", Kind::Task, "Форма входа", "В работе", "см. T-005 и токены"),
            CardRef::new("T-002", Kind::Task, "Refactor parser", "Бэклог", ""),
            CardRef::new("T-003", Kind::Task, "Docs", "Бэклог", "parser notes: refactor the T-0 thing"),
            CardRef::new("T-005", Kind::Task, "Ship it", "Готово", ""),
        ]
    }

    fn ids(v: Vec<&CardRef>) -> Vec<&str> {
        v.into_iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn ranking_tiers() {
        let c = cards();
        // id prefix beats body match, ties by id
        assert_eq!(ids(search(&c, "T-0", &[], 8)), ["T-001", "T-002", "T-003", "T-005"]);
        // title word-prefix beats body match
        assert_eq!(ids(search(&c, "parser", &[], 8)), ["T-002", "T-003"]);
        // id prefix beats everything: "t-003" is also in nobody's body but T-003's id
        assert_eq!(ids(search(&c, "T-003", &[], 8)), ["T-003"]);
        // title substring after word prefix
        assert_eq!(ids(search(&c, "actor", &[], 8)), ["T-002", "T-003"]);
        // id substring
        assert_eq!(ids(search(&c, "005", &[], 8)), ["T-005", "T-001"]);
        // column name and body
        assert_eq!(ids(search(&c, "готово", &[], 8)), ["T-005"]);
        assert_eq!(ids(search(&c, "токен", &[], 8)), ["T-001"]);
        // fuzzy subsequence in title
        assert_eq!(ids(search(&c, "rfp", &[], 8)), ["T-002"]);
        assert!(search(&c, "zzzz", &[], 8).is_empty());
        assert!(search(&c, "", &[], 8).is_empty());
        assert_eq!(search(&c, "t", &[], 2).len(), 2);
    }

    #[test]
    fn cyrillic_case_insensitive() {
        let c = cards();
        assert_eq!(ids(search(&c, "ФОРМА", &[], 8)), ["T-001"]);
        assert_eq!(ids(search(&c, "авториз", &[], 8)), ["E-001"]);
        assert_eq!(ids(search(&c, "вход", &[], 8)), ["T-001", "E-001"]);
        assert_eq!(ids(search(&c, "бэк", &[], 8)), ["T-002", "T-003"]);
    }

    #[test]
    fn skip_listed_ids() {
        let c = cards();
        let skip = vec!["t-001".to_string(), "t-002".to_string()];
        assert_eq!(ids(search(&c, "T-0", &skip, 8)), ["T-003", "T-005"]);
    }
}
