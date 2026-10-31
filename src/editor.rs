//! Tiny text editor buffer: single-line or multi-line with soft wrapping.
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct TextBuf {
    pub lines: Vec<Vec<char>>,
    pub row: usize,
    pub col: usize,
    pub multiline: bool,
    /// Render width, updated on every draw (needed for visual up/down).
    pub width: usize,
    pub scroll: usize,
}

impl TextBuf {
    pub fn new(s: &str, multiline: bool) -> Self {
        let mut lines: Vec<Vec<char>> = if multiline {
            s.split('\n').map(|l| l.trim_end_matches('\r').chars().collect()).collect()
        } else {
            vec![s.replace('\n', " ").chars().collect()]
        };
        if lines.is_empty() {
            lines.push(vec![]);
        }
        let row = lines.len() - 1;
        let col = lines[row].len();
        Self { lines, row, col, multiline, width: 60, scroll: 0 }
    }

    pub fn text(&self) -> String {
        self.lines.iter().map(|l| l.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        self.lines[self.row].insert(self.col, c);
        self.col += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '\r' => {}
                '\n' if self.multiline => self.newline(),
                '\n' => self.insert_char(' '),
                c => self.insert_char(c),
            }
        }
    }

    fn newline(&mut self) {
        let rest = self.lines[self.row].split_off(self.col);
        self.lines.insert(self.row + 1, rest);
        self.row += 1;
        self.col = 0;
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.lines[self.row].remove(self.col);
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].extend(cur);
        }
    }

    fn delete(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.lines[self.row].remove(self.col);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].extend(next);
        }
    }

    fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.lines[self.row].len();
        }
    }

    fn right(&mut self) {
        if self.col < self.lines[self.row].len() {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    /// Move one visual row up. False when already on the first row.
    fn up(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let w = self.width.max(1);
        if self.col >= w {
            self.col -= w;
            true
        } else if self.row > 0 {
            let off = self.col;
            self.row -= 1;
            let len = self.lines[self.row].len();
            self.col = ((len / w) * w + off).min(len);
            true
        } else {
            false
        }
    }

    /// Move one visual row down. False when already on the last row.
    fn down(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let w = self.width.max(1);
        let len = self.lines[self.row].len();
        if self.col / w < len / w {
            self.col = (self.col + w).min(len);
            true
        } else if self.row + 1 < self.lines.len() {
            let off = self.col % w;
            self.row += 1;
            self.col = off.min(self.lines[self.row].len());
            true
        } else {
            false
        }
    }

    /// Returns true if the key was consumed by the editor.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char(c) if !ctrl => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = self.lines[self.row].len(),
            KeyCode::Enter if self.multiline => self.newline(),
            KeyCode::Up => return self.up(),
            KeyCode::Down => return self.down(),
            _ => return false,
        }
        true
    }

    // ---- `@` file mentions ----

    /// The `@token` ending at the cursor: `@` at line start or after whitespace, followed by
    /// non-whitespace chars up to the cursor. Returns (column of `@`, query without `@`).
    pub fn mention_token(&self) -> Option<(usize, String)> {
        let line = &self.lines[self.row];
        let end = self.col.min(line.len());
        let mut start = end;
        while start > 0 && !line[start - 1].is_whitespace() {
            start -= 1;
        }
        if start < end && line[start] == '@' {
            Some((start, line[start + 1..end].iter().collect()))
        } else {
            None
        }
    }

    /// Replace the chars from `start_col` up to the cursor with `text`; the cursor lands after it.
    pub fn replace_mention(&mut self, start_col: usize, text: &str) {
        let end = self.col.min(self.lines[self.row].len());
        let start = start_col.min(end);
        self.lines[self.row].splice(start..end, text.chars());
        self.col = start + text.chars().count();
    }

    // ---- comma/space separated lists (card ids) ----

    /// The list item ending at the cursor: chars after the last comma/whitespace. None when empty.
    /// Returns (column of the item start, item text).
    pub fn list_token(&self) -> Option<(usize, String)> {
        let line = &self.lines[self.row];
        let end = self.col.min(line.len());
        let mut start = end;
        while start > 0 && line[start - 1] != ',' && !line[start - 1].is_whitespace() {
            start -= 1;
        }
        (start < end).then(|| (start, line[start..end].iter().collect()))
    }

    /// Replace the list item from `start_col` (through the end of the word under the cursor) with
    /// `item` followed by `, `. Separators after it are normalised, so a trailing one is not doubled.
    pub fn replace_list_token(&mut self, start_col: usize, item: &str) {
        let line = &mut self.lines[self.row];
        let end = self.col.min(line.len());
        let start = start_col.min(end);
        let is_sep = |c: &char| *c == ',' || c.is_whitespace();
        let rest: Vec<char> = line[end..].iter().copied().skip_while(|c| !is_sep(c)).skip_while(is_sep).collect();
        let mut new: Vec<char> = line[..start].to_vec();
        new.extend(item.chars());
        new.extend(", ".chars());
        let col = new.len();
        new.extend(rest);
        *line = new;
        self.col = col;
    }

    // ---- rendering helpers (multi-line) ----

    pub fn visual_lines(&self) -> Vec<String> {
        let w = self.width.max(1);
        let mut out = Vec::new();
        for l in &self.lines {
            for i in 0..(l.len() / w + 1) {
                let end = ((i + 1) * w).min(l.len());
                out.push(l[(i * w).min(end)..end].iter().collect());
            }
        }
        out
    }

    /// (visual row, visual col) of the cursor.
    pub fn visual_pos(&self) -> (usize, usize) {
        let w = self.width.max(1);
        let before: usize = self.lines[..self.row].iter().map(|l| l.len() / w + 1).sum();
        (before + self.col / w, self.col % w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(s: &str) -> TextBuf {
        TextBuf::new(s, true)
    }

    #[test]
    fn mention_token_detection() {
        assert_eq!(at_end("@").mention_token(), Some((0, String::new())));
        assert_eq!(at_end("@src/u").mention_token(), Some((0, "src/u".into())));
        assert_eq!(at_end("see @ui").mention_token(), Some((4, "ui".into())));
        assert_eq!(at_end("see\t@ui").mention_token(), Some((4, "ui".into())));
        // mid-word `@` is not a mention
        assert_eq!(at_end("a@b").mention_token(), None);
        assert_eq!(at_end("mail me a@").mention_token(), None);
        // whitespace between `@` and the cursor ends the token
        assert_eq!(at_end("@ui ").mention_token(), None);
        assert_eq!(at_end("plain").mention_token(), None);
        assert_eq!(at_end("").mention_token(), None);
        // only the current line counts
        assert_eq!(at_end("@ui\nnext").mention_token(), None);
        assert_eq!(at_end("x\n@ui").mention_token(), Some((0, "ui".into())));
        // the token ends at the cursor, not at the end of the word
        let mut tb = at_end("@src/ui.rs");
        tb.col = 4;
        assert_eq!(tb.mention_token(), Some((0, "src".into())));
    }

    #[test]
    fn list_token_and_replace() {
        let mut tb = TextBuf::new("T-001, T-0", false);
        assert_eq!(tb.list_token(), Some((7, "T-0".into())));
        tb.replace_list_token(7, "T-002");
        assert_eq!((tb.text().as_str(), tb.col), ("T-001, T-002, ", 14));
        assert_eq!(tb.list_token(), None); // after a separator: nothing typed yet
        assert_eq!(TextBuf::new("", false).list_token(), None);
        assert_eq!(TextBuf::new("a,b", false).list_token(), Some((2, "b".into())));
        // trailing separator already there: not doubled; text after is kept
        let mut tb = TextBuf::new("T-0, T-009", false);
        tb.col = 3;
        tb.replace_list_token(0, "T-001");
        assert_eq!((tb.text().as_str(), tb.col), ("T-001, T-009", 7));
        let mut tb = TextBuf::new("T-0,", false);
        tb.col = 3;
        tb.replace_list_token(0, "T-001");
        assert_eq!(tb.text(), "T-001, ");
        // cursor mid-word: the rest of the word goes too
        let mut tb = TextBuf::new("T-0xx", false);
        tb.col = 3;
        tb.replace_list_token(0, "T-007");
        assert_eq!(tb.text(), "T-007, ");
    }

    #[test]
    fn replace_mention_swaps_query() {
        let mut tb = at_end("see @ui");
        let (s, _) = tb.mention_token().unwrap();
        tb.replace_mention(s, "@src/ui.rs ");
        assert_eq!(tb.text(), "see @src/ui.rs ");
        assert_eq!(tb.col, 15);
        // text after the cursor is kept
        let mut tb = at_end("@u tail");
        tb.col = 2;
        tb.replace_mention(0, "@a/b/");
        assert_eq!(tb.text(), "@a/b/ tail");
        assert_eq!(tb.col, 5);
    }
}
