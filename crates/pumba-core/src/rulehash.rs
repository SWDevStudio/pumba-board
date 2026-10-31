//! Fingerprint of the agent rules: lets a driver (or a script) notice that the board rules changed
//! since it last read them. FNV-1a written out by hand: `DefaultHasher` is not stable between Rust
//! versions, and the value is stored in files.
use crate::model::{Column, Config};

/// Version of the agent protocol (AGENTS.md text and the output of the CLI commands). Bump it by
/// one when either changes: every stored fingerprint then goes stale.
pub const PROTOCOL: u32 = 1;

/// 64-bit FNV-1a.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 12 hex characters: the top 48 bits of the hash.
fn short(h: u64) -> String {
    format!("{h:016x}")[..12].to_string()
}

/// Hash of a list of fields; the separator keeps `["ab", "c"]` and `["a", "bc"]` apart.
fn fields_hash(fields: &[&str]) -> String {
    let mut buf = Vec::new();
    for f in fields {
        buf.extend_from_slice(f.as_bytes());
        buf.push(0x1f);
    }
    short(fnv1a(&buf))
}

/// One named part of the fingerprint.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Part {
    pub name: String,
    pub hash: String,
}

/// Hash of the rule fields of a column (the color is not part of it).
pub fn column_hash(c: &Column) -> String {
    fields_hash(&[c.id.trim(), c.name.trim(), c.before.trim(), c.rule.trim(), c.after.trim(), c.agent.trim(), c.next.trim()])
}

/// Name of the part for column `id`.
pub fn column_part(id: &str) -> String {
    format!("col:{id}")
}

/// Every part: `proto`, `cols` (column order), `col:<id>`, `rule:<id>` (selected option), `custom`.
pub fn parts(cfg: &Config) -> Vec<Part> {
    let mut v = vec![Part { name: "proto".into(), hash: fields_hash(&[&PROTOCOL.to_string()]) }];
    let ids: Vec<&str> = cfg.columns.iter().map(|c| c.id.as_str()).collect();
    v.push(Part { name: "cols".into(), hash: fields_hash(&ids) });
    for c in &cfg.columns {
        v.push(Part { name: column_part(&c.id), hash: column_hash(c) });
    }
    for r in &cfg.rules {
        let text = r.options.iter().find(|o| o.id == r.value).map_or("", |o| o.text.as_str());
        v.push(Part { name: format!("rule:{}", r.id), hash: fields_hash(&[&r.id, &r.value, text]) });
    }
    v.push(Part { name: "custom".into(), hash: fields_hash(&[cfg.custom_rules.trim()]) });
    v
}

/// Overall fingerprint: from the sorted `name=hash` lines of the parts.
pub fn rules_hash(cfg: &Config) -> String {
    let mut lines: Vec<String> = parts(cfg).iter().map(|p| format!("{}={}", p.name, p.hash)).collect();
    lines.sort();
    short(fnv1a(lines.join("\n").as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_known_values() {
        // reference values of 64-bit FNV-1a
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn hash_is_stable_and_12_hex() {
        let cfg = Config::default();
        let h = rules_hash(&cfg);
        assert_eq!(h.len(), 12);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, rules_hash(&cfg.clone()));
    }

    #[test]
    fn hash_follows_rules_not_color() {
        let base = Config::default();
        let h = rules_hash(&base);
        let mut c = base.clone();
        c.columns[1].color = "blue".into();
        assert_eq!(rules_hash(&c), h, "the color is not a rule");
        let mut c = base.clone();
        c.columns[1].after.push_str(" ещё");
        assert_ne!(rules_hash(&c), h);
        let mut c = base.clone();
        c.rules[0].value = "free".into();
        assert_ne!(rules_hash(&c), h);
        let mut c = base.clone();
        c.custom_rules = "своё".into();
        assert_ne!(rules_hash(&c), h);
        let mut c = base.clone();
        c.columns.swap(1, 2);
        assert_ne!(rules_hash(&c), h);
        // sprint and models are not rules
        let mut c = base.clone();
        c.sprint = 7;
        c.models.push("x".into());
        assert_eq!(rules_hash(&c), h);
    }

    #[test]
    fn column_part_changes_only_its_hash() {
        let base = Config::default();
        let mut c = base.clone();
        c.columns[1].rule.push('!');
        let (a, b) = (parts(&base), parts(&c));
        let diff: Vec<&str> = a.iter().zip(&b).filter(|(x, y)| x != y).map(|(x, _)| x.name.as_str()).collect();
        assert_eq!(diff, ["col:in-progress"]);
    }
}
