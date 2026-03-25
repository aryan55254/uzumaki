use unicode_segmentation::UnicodeSegmentation;
use winit::keyboard::{Key, NamedKey};

// ── Selection ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Selection {
    /// Anchor point (where selection started), flat grapheme index
    pub anchor: usize,
    /// Active point / cursor position, flat grapheme index
    pub active: usize,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            anchor: 0,
            active: 0,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.active
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.active)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.active)
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.anchor = pos;
        self.active = pos;
    }
}

// ── EditEvent ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum EditKind {
    Insert,
    DeleteBackward,
    DeleteForward,
    DeleteWordBackward,
    DeleteWordForward,
}

#[derive(Clone, Debug)]
pub struct EditEvent {
    pub kind: EditKind,
    pub inserted: Option<String>,
}

// ── KeyResult ────────────────────────────────────────────────────────

pub enum KeyResult {
    Edit(EditEvent),
    Blur,
    Handled,
    Ignored,
}

// ── TextModel ────────────────────────────────────────────────────────

pub struct TextModel {
    /// Internal line-based buffer. Each entry is one line's content (no trailing \n).
    /// Always has at least one entry (empty string for empty document).
    lines: Vec<String>,
    pub selection: Selection,
    pub max_length: Option<usize>,
}

impl TextModel {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            selection: Selection::new(),
            max_length: None,
        }
    }

    // ── Public accessors ─────────────────────────────────────────────

    /// Full text as a single string (lines joined with \n).
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line's content (without trailing \n).
    pub fn line(&self, row: usize) -> &str {
        &self.lines[row]
    }

    /// Grapheme count for a specific line.
    fn line_grapheme_count(&self, row: usize) -> usize {
        self.lines[row].graphemes(true).count()
    }

    /// Total grapheme count across all lines (including \n separators).
    pub fn grapheme_count(&self) -> usize {
        let mut count = 0;
        for (i, line) in self.lines.iter().enumerate() {
            count += line.graphemes(true).count();
            if i < self.lines.len() - 1 {
                count += 1; // \n separator
            }
        }
        count
    }

    // ── Coordinate conversion ────────────────────────────────────────

    /// Convert a flat grapheme index to (row, col) in the line buffer.
    fn flat_to_rowcol(&self, idx: usize) -> (usize, usize) {
        let mut remaining = idx;
        for (row, line) in self.lines.iter().enumerate() {
            let line_len = line.graphemes(true).count();
            if remaining <= line_len && (row == self.lines.len() - 1 || remaining < line_len + 1) {
                return (row, remaining.min(line_len));
            }
            remaining -= line_len + 1;
        }
        let last = self.lines.len() - 1;
        (last, self.line_grapheme_count(last))
    }

    /// Convert (row, col) to a flat grapheme index.
    fn rowcol_to_flat(&self, row: usize, col: usize) -> usize {
        let mut flat = 0;
        for r in 0..row.min(self.lines.len()) {
            flat += self.line_grapheme_count(r) + 1;
        }
        let clamped_row = row.min(self.lines.len() - 1);
        flat + col.min(self.line_grapheme_count(clamped_row))
    }

    /// Get the grapheme-to-byte offset within a single line.
    fn grapheme_to_byte_in_line(line: &str, grapheme_idx: usize) -> usize {
        line.grapheme_indices(true)
            .nth(grapheme_idx)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    // ── Selected text ────────────────────────────────────────────────

    pub fn selected_text(&self) -> String {
        if self.selection.is_collapsed() {
            return String::new();
        }
        let start = self.selection.start();
        let end = self.selection.end();
        let (sr, sc) = self.flat_to_rowcol(start);
        let (er, ec) = self.flat_to_rowcol(end);

        if sr == er {
            let line = &self.lines[sr];
            let byte_start = Self::grapheme_to_byte_in_line(line, sc);
            let byte_end = Self::grapheme_to_byte_in_line(line, ec);
            return line[byte_start..byte_end].to_string();
        }

        let mut result = String::new();
        let first = &self.lines[sr];
        let byte_start = Self::grapheme_to_byte_in_line(first, sc);
        result.push_str(&first[byte_start..]);
        result.push('\n');
        for r in (sr + 1)..er {
            result.push_str(&self.lines[r]);
            result.push('\n');
        }
        let last = &self.lines[er];
        let byte_end = Self::grapheme_to_byte_in_line(last, ec);
        result.push_str(&last[..byte_end]);
        result
    }

    // ── Editing ──────────────────────────────────────────────────────

    /// Delete selected text. Returns true if something was deleted.
    pub fn delete_selection(&mut self) -> bool {
        if self.selection.is_collapsed() {
            return false;
        }
        let start = self.selection.start();
        let end = self.selection.end();
        let (sr, sc) = self.flat_to_rowcol(start);
        let (er, ec) = self.flat_to_rowcol(end);

        if sr == er {
            let line = &self.lines[sr];
            let byte_start = Self::grapheme_to_byte_in_line(line, sc);
            let byte_end = Self::grapheme_to_byte_in_line(line, ec);
            let mut new_line = String::with_capacity(line.len() - (byte_end - byte_start));
            new_line.push_str(&line[..byte_start]);
            new_line.push_str(&line[byte_end..]);
            self.lines[sr] = new_line;
        } else {
            let first = &self.lines[sr];
            let last = &self.lines[er];
            let byte_start = Self::grapheme_to_byte_in_line(first, sc);
            let byte_end = Self::grapheme_to_byte_in_line(last, ec);
            let mut merged = String::new();
            merged.push_str(&first[..byte_start]);
            merged.push_str(&last[byte_end..]);
            self.lines[sr] = merged;
            self.lines.drain((sr + 1)..=er);
        }

        self.selection.set_cursor(start);
        true
    }

    pub fn set_value(&mut self, value: String) {
        let current = self.text();
        if current == value {
            return;
        }
        self.lines = value.split('\n').map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let count = self.grapheme_count();
        if self.selection.active > count {
            self.selection.active = count;
        }
        if self.selection.anchor > count {
            self.selection.anchor = count;
        }
    }

    /// Insert text at cursor.
    pub fn insert(&mut self, ch: &str) -> Option<EditEvent> {
        if ch.is_empty() {
            return None;
        }

        if let Some(max) = self.max_length {
            let current = self.grapheme_count() - (self.selection.end() - self.selection.start());
            let insert_count = ch.graphemes(true).count();
            if current + insert_count > max {
                return None;
            }
        }

        self.delete_selection();
        let (row, col) = self.flat_to_rowcol(self.selection.active);

        let insert_lines: Vec<&str> = ch.split('\n').collect();

        if insert_lines.len() == 1 {
            let line = &self.lines[row];
            let byte_pos = Self::grapheme_to_byte_in_line(line, col);
            let mut new_line = String::with_capacity(line.len() + insert_lines[0].len());
            new_line.push_str(&line[..byte_pos]);
            new_line.push_str(insert_lines[0]);
            new_line.push_str(&line[byte_pos..]);
            self.lines[row] = new_line;
        } else {
            let line = &self.lines[row];
            let byte_pos = Self::grapheme_to_byte_in_line(line, col);
            let before = line[..byte_pos].to_string();
            let after = line[byte_pos..].to_string();

            let mut first = before;
            first.push_str(insert_lines[0]);
            self.lines[row] = first;

            for (i, &ins_line) in insert_lines.iter().enumerate().skip(1) {
                if i < insert_lines.len() - 1 {
                    self.lines.insert(row + i, ins_line.to_string());
                } else {
                    let mut last = ins_line.to_string();
                    last.push_str(&after);
                    self.lines.insert(row + i, last);
                }
            }
        }

        let inserted_graphemes = ch.graphemes(true).count();
        self.selection.active += inserted_graphemes;
        self.selection.anchor = self.selection.active;
        Some(EditEvent {
            kind: EditKind::Insert,
            inserted: Some(ch.to_string()),
        })
    }

    pub fn delete_backward(&mut self) -> Option<EditEvent> {
        if self.delete_selection() {
            return Some(EditEvent {
                kind: EditKind::DeleteBackward,
                inserted: None,
            });
        }
        if self.selection.active == 0 {
            return None;
        }
        let (row, col) = self.flat_to_rowcol(self.selection.active);
        if col == 0 {
            let current_line = self.lines.remove(row);
            self.lines[row - 1].push_str(&current_line);
        } else {
            let line = &self.lines[row];
            let byte_end = Self::grapheme_to_byte_in_line(line, col);
            let byte_start = Self::grapheme_to_byte_in_line(line, col - 1);
            let mut new_line = String::with_capacity(line.len() - (byte_end - byte_start));
            new_line.push_str(&line[..byte_start]);
            new_line.push_str(&line[byte_end..]);
            self.lines[row] = new_line;
        }
        self.selection.active -= 1;
        self.selection.anchor = self.selection.active;
        Some(EditEvent {
            kind: EditKind::DeleteBackward,
            inserted: None,
        })
    }

    pub fn delete_forward(&mut self) -> Option<EditEvent> {
        if self.delete_selection() {
            return Some(EditEvent {
                kind: EditKind::DeleteForward,
                inserted: None,
            });
        }
        let count = self.grapheme_count();
        if self.selection.active >= count {
            return None;
        }
        let (row, col) = self.flat_to_rowcol(self.selection.active);
        let line_len = self.line_grapheme_count(row);
        if col == line_len && row < self.lines.len() - 1 {
            let next_line = self.lines.remove(row + 1);
            self.lines[row].push_str(&next_line);
        } else {
            let line = &self.lines[row];
            let byte_start = Self::grapheme_to_byte_in_line(line, col);
            let byte_end = Self::grapheme_to_byte_in_line(line, col + 1);
            let mut new_line = String::with_capacity(line.len() - (byte_end - byte_start));
            new_line.push_str(&line[..byte_start]);
            new_line.push_str(&line[byte_end..]);
            self.lines[row] = new_line;
        }
        Some(EditEvent {
            kind: EditKind::DeleteForward,
            inserted: None,
        })
    }

    pub fn delete_word_backward(&mut self) -> Option<EditEvent> {
        if self.delete_selection() {
            return Some(EditEvent {
                kind: EditKind::DeleteWordBackward,
                inserted: None,
            });
        }
        if self.selection.active == 0 {
            return None;
        }
        let text = self.text();
        let graphemes: Vec<&str> = text.graphemes(true).collect();
        let end = self.selection.active;
        let mut pos = end;
        while pos > 0 && graphemes[pos - 1].chars().all(char::is_whitespace) {
            pos -= 1;
        }
        while pos > 0 && !graphemes[pos - 1].chars().all(char::is_whitespace) {
            pos -= 1;
        }
        self.selection.anchor = pos;
        self.selection.active = end;
        self.delete_selection();
        Some(EditEvent {
            kind: EditKind::DeleteWordBackward,
            inserted: None,
        })
    }

    pub fn delete_word_forward(&mut self) -> Option<EditEvent> {
        if self.delete_selection() {
            return Some(EditEvent {
                kind: EditKind::DeleteWordForward,
                inserted: None,
            });
        }
        let count = self.grapheme_count();
        if self.selection.active >= count {
            return None;
        }
        let text = self.text();
        let graphemes: Vec<&str> = text.graphemes(true).collect();
        let start = self.selection.active;
        let mut pos = start;
        while pos < count && !graphemes[pos].chars().all(char::is_whitespace) {
            pos += 1;
        }
        while pos < count && graphemes[pos].chars().all(char::is_whitespace) {
            pos += 1;
        }
        self.selection.anchor = start;
        self.selection.active = pos;
        self.delete_selection();
        Some(EditEvent {
            kind: EditKind::DeleteWordForward,
            inserted: None,
        })
    }

    // ── Movement ─────────────────────────────────────────────────────

    pub fn move_left(&mut self, extend: bool) {
        if !extend && !self.selection.is_collapsed() {
            let pos = self.selection.start();
            self.selection.set_cursor(pos);
        } else if self.selection.active > 0 {
            self.selection.active -= 1;
            if !extend {
                self.selection.anchor = self.selection.active;
            }
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        let count = self.grapheme_count();
        if !extend && !self.selection.is_collapsed() {
            let pos = self.selection.end();
            self.selection.set_cursor(pos);
        } else if self.selection.active < count {
            self.selection.active += 1;
            if !extend {
                self.selection.anchor = self.selection.active;
            }
        }
    }

    pub fn move_word_left(&mut self, extend: bool) {
        let text = self.text();
        let graphemes: Vec<&str> = text.graphemes(true).collect();
        let mut pos = self.selection.active;
        while pos > 0 && graphemes[pos - 1].chars().all(char::is_whitespace) {
            pos -= 1;
        }
        while pos > 0 && !graphemes[pos - 1].chars().all(char::is_whitespace) {
            pos -= 1;
        }
        self.selection.active = pos;
        if !extend {
            self.selection.anchor = pos;
        }
    }

    pub fn move_word_right(&mut self, extend: bool) {
        let text = self.text();
        let graphemes: Vec<&str> = text.graphemes(true).collect();
        let count = graphemes.len();
        let mut pos = self.selection.active;
        while pos < count && !graphemes[pos].chars().all(char::is_whitespace) {
            pos += 1;
        }
        while pos < count && graphemes[pos].chars().all(char::is_whitespace) {
            pos += 1;
        }
        self.selection.active = pos;
        if !extend {
            self.selection.anchor = pos;
        }
    }

    pub fn move_home(&mut self, extend: bool) {
        self.move_line_start(extend);
    }

    pub fn move_end(&mut self, extend: bool) {
        self.move_line_end(extend);
    }

    pub fn move_absolute_home(&mut self, extend: bool) {
        self.selection.active = 0;
        if !extend {
            self.selection.anchor = 0;
        }
    }

    pub fn move_absolute_end(&mut self, extend: bool) {
        let count = self.grapheme_count();
        self.selection.active = count;
        if !extend {
            self.selection.anchor = count;
        }
    }

    pub fn move_line_start(&mut self, extend: bool) {
        let (row, _col) = self.flat_to_rowcol(self.selection.active);
        let flat = self.rowcol_to_flat(row, 0);
        self.selection.active = flat;
        if !extend {
            self.selection.anchor = flat;
        }
    }

    pub fn move_line_end(&mut self, extend: bool) {
        let (row, _col) = self.flat_to_rowcol(self.selection.active);
        let line_len = self.line_grapheme_count(row);
        let flat = self.rowcol_to_flat(row, line_len);
        self.selection.active = flat;
        if !extend {
            self.selection.anchor = flat;
        }
    }

    /// Move up one line, preserving column. Returns true if moved to a different line.
    pub fn move_up(&mut self, extend: bool, sticky_col: Option<usize>) -> bool {
        let (row, col) = self.flat_to_rowcol(self.selection.active);
        if row == 0 {
            self.selection.active = 0;
            if !extend {
                self.selection.anchor = 0;
            }
            return false;
        }
        let target_col = sticky_col.unwrap_or(col);
        let prev_line_len = self.line_grapheme_count(row - 1);
        let new_col = target_col.min(prev_line_len);
        let flat = self.rowcol_to_flat(row - 1, new_col);
        self.selection.active = flat;
        if !extend {
            self.selection.anchor = flat;
        }
        true
    }

    /// Move down one line, preserving column. Returns true if moved to a different line.
    pub fn move_down(&mut self, extend: bool, sticky_col: Option<usize>) -> bool {
        let (row, col) = self.flat_to_rowcol(self.selection.active);
        if row >= self.lines.len() - 1 {
            let count = self.grapheme_count();
            self.selection.active = count;
            if !extend {
                self.selection.anchor = count;
            }
            return false;
        }
        let target_col = sticky_col.unwrap_or(col);
        let next_line_len = self.line_grapheme_count(row + 1);
        let new_col = target_col.min(next_line_len);
        let flat = self.rowcol_to_flat(row + 1, new_col);
        self.selection.active = flat;
        if !extend {
            self.selection.anchor = flat;
        }
        true
    }

    /// Current (row, col) of the active cursor position.
    pub fn cursor_rowcol(&self) -> (usize, usize) {
        self.flat_to_rowcol(self.selection.active)
    }

    /// Move cursor to a specific grapheme index.
    pub fn move_to(&mut self, pos: usize, extend: bool) {
        let count = self.grapheme_count();
        self.selection.active = pos.min(count);
        if !extend {
            self.selection.anchor = self.selection.active;
        }
    }

    pub fn select_all(&mut self) {
        self.selection.anchor = 0;
        self.selection.active = self.grapheme_count();
    }

    pub fn word_at(&self, grapheme_idx: usize) -> (usize, usize) {
        let text = self.text();
        let graphemes: Vec<&str> = text.graphemes(true).collect();
        if graphemes.is_empty() {
            return (0, 0);
        }
        let idx = grapheme_idx.min(graphemes.len().saturating_sub(1));

        let mut start = idx;
        while start > 0 && !graphemes[start - 1].chars().all(char::is_whitespace) {
            start -= 1;
        }

        let mut end = idx;
        while end < graphemes.len() && !graphemes[end].chars().all(char::is_whitespace) {
            end += 1;
        }

        (start, end)
    }

    // ── Key handling ─────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: &Key, modifiers: u32) -> KeyResult {
        let shift = modifiers & 4 != 0;
        let ctrl = modifiers & 1 != 0;

        match key {
            Key::Character(ch) => {
                if ctrl {
                    if ch.eq_ignore_ascii_case("a") {
                        self.select_all();
                        return KeyResult::Handled;
                    }
                    return KeyResult::Ignored;
                }
                match self.insert(ch) {
                    Some(edit) => KeyResult::Edit(edit),
                    None => KeyResult::Handled,
                }
            }
            Key::Named(named) => match named {
                NamedKey::Backspace => {
                    if ctrl {
                        match self.delete_word_backward() {
                            Some(edit) => KeyResult::Edit(edit),
                            None => KeyResult::Handled,
                        }
                    } else {
                        match self.delete_backward() {
                            Some(edit) => KeyResult::Edit(edit),
                            None => KeyResult::Handled,
                        }
                    }
                }
                NamedKey::Delete => {
                    if ctrl {
                        match self.delete_word_forward() {
                            Some(edit) => KeyResult::Edit(edit),
                            None => KeyResult::Handled,
                        }
                    } else {
                        match self.delete_forward() {
                            Some(edit) => KeyResult::Edit(edit),
                            None => KeyResult::Handled,
                        }
                    }
                }
                NamedKey::ArrowLeft => {
                    if ctrl {
                        self.move_word_left(shift);
                    } else {
                        self.move_left(shift);
                    }
                    KeyResult::Handled
                }
                NamedKey::ArrowRight => {
                    if ctrl {
                        self.move_word_right(shift);
                    } else {
                        self.move_right(shift);
                    }
                    KeyResult::Handled
                }
                NamedKey::ArrowUp | NamedKey::ArrowDown => KeyResult::Ignored,
                NamedKey::Home => {
                    if ctrl {
                        self.move_absolute_home(shift);
                    } else {
                        self.move_home(shift);
                    }
                    KeyResult::Handled
                }
                NamedKey::End => {
                    if ctrl {
                        self.move_absolute_end(shift);
                    } else {
                        self.move_end(shift);
                    }
                    KeyResult::Handled
                }
                NamedKey::Space => match self.insert(" ") {
                    Some(edit) => KeyResult::Edit(edit),
                    None => KeyResult::Handled,
                },
                NamedKey::Escape => KeyResult::Blur,
                NamedKey::Enter => match self.insert("\n") {
                    Some(edit) => KeyResult::Edit(edit),
                    None => KeyResult::Ignored,
                },
                NamedKey::Tab => match self.insert("    ") {
                    Some(edit) => KeyResult::Edit(edit),
                    None => KeyResult::Ignored,
                },
                _ => KeyResult::Ignored,
            },
            _ => KeyResult::Ignored,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn model(text: &str) -> TextModel {
        let mut m = TextModel::new();
        m.set_value(text.to_string());
        m
    }

    // ── flat_to_rowcol / rowcol_to_flat round-trip ───────────────────

    #[test]
    fn flat_to_rowcol_single_line() {
        let m = model("hello");
        assert_eq!(m.flat_to_rowcol(0), (0, 0));
        assert_eq!(m.flat_to_rowcol(3), (0, 3));
        assert_eq!(m.flat_to_rowcol(5), (0, 5));
    }

    #[test]
    fn flat_to_rowcol_multiline() {
        let m = model("ab\ncd\nef");
        assert_eq!(m.flat_to_rowcol(0), (0, 0));
        assert_eq!(m.flat_to_rowcol(2), (0, 2));
        assert_eq!(m.flat_to_rowcol(3), (1, 0));
        assert_eq!(m.flat_to_rowcol(5), (1, 2));
        assert_eq!(m.flat_to_rowcol(6), (2, 0));
        assert_eq!(m.flat_to_rowcol(8), (2, 2));
    }

    #[test]
    fn rowcol_to_flat_roundtrip() {
        let m = model("hello\nworld\nfoo");
        for i in 0..=m.grapheme_count() {
            let (r, c) = m.flat_to_rowcol(i);
            assert_eq!(m.rowcol_to_flat(r, c), i, "round-trip failed for flat index {i}");
        }
    }

    #[test]
    fn rowcol_to_flat_values() {
        let m = model("ab\ncd\nef");
        assert_eq!(m.rowcol_to_flat(0, 0), 0);
        assert_eq!(m.rowcol_to_flat(0, 2), 2);
        assert_eq!(m.rowcol_to_flat(1, 0), 3);
        assert_eq!(m.rowcol_to_flat(1, 2), 5);
        assert_eq!(m.rowcol_to_flat(2, 0), 6);
        assert_eq!(m.rowcol_to_flat(2, 2), 8);
    }

    // ── Insert / newline ─────────────────────────────────────────────

    #[test]
    fn insert_text_then_newline_then_more() {
        let mut m = TextModel::new();
        m.insert("hello");
        m.insert("\n");
        m.insert("world");
        assert_eq!(m.text(), "hello\nworld");
        assert_eq!(m.line_count(), 2);
        assert_eq!(m.line(0), "hello");
        assert_eq!(m.line(1), "world");
    }

    #[test]
    fn insert_in_middle_of_line() {
        let mut m = model("hllo");
        m.move_to(1, false);
        m.insert("e");
        assert_eq!(m.text(), "hello");
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut m = model("helloworld");
        m.move_to(5, false);
        m.insert("\n");
        assert_eq!(m.text(), "hello\nworld");
        assert_eq!(m.line_count(), 2);
    }

    // ── Backspace ────────────────────────────────────────────────────

    #[test]
    fn backspace_at_col0_joins_lines() {
        let mut m = model("hello\nworld");
        m.move_to(6, false);
        m.delete_backward();
        assert_eq!(m.text(), "helloworld");
        assert_eq!(m.line_count(), 1);
        assert_eq!(m.selection.active, 5);
    }

    #[test]
    fn backspace_within_line() {
        let mut m = model("hello");
        m.move_to(5, false);
        m.delete_backward();
        assert_eq!(m.text(), "hell");
    }

    // ── Delete forward ───────────────────────────────────────────────

    #[test]
    fn delete_forward_at_eol_joins_lines() {
        let mut m = model("hello\nworld");
        m.move_to(5, false);
        m.delete_forward();
        assert_eq!(m.text(), "helloworld");
        assert_eq!(m.line_count(), 1);
    }

    // ── Move left/right across newline ───────────────────────────────

    #[test]
    fn move_right_across_newline() {
        let mut m = model("ab\ncd");
        m.move_to(2, false);
        m.move_right(false);
        assert_eq!(m.selection.active, 3);
        assert_eq!(m.flat_to_rowcol(3), (1, 0));
    }

    #[test]
    fn move_left_across_newline() {
        let mut m = model("ab\ncd");
        m.move_to(3, false);
        m.move_left(false);
        assert_eq!(m.selection.active, 2);
        assert_eq!(m.flat_to_rowcol(2), (0, 2));
    }

    // ── move_home / move_end per line ────────────────────────────────

    #[test]
    fn move_home_on_second_line() {
        let mut m = model("hello\nworld");
        m.move_to(8, false);
        m.move_home(false);
        assert_eq!(m.selection.active, 6);
    }

    #[test]
    fn move_end_on_first_line() {
        let mut m = model("hello\nworld");
        m.move_to(2, false);
        m.move_end(false);
        assert_eq!(m.selection.active, 5);
    }

    #[test]
    fn move_home_end_on_each_line() {
        let mut m = model("abc\ndef\nghi");
        m.move_to(1, false);
        m.move_home(false);
        assert_eq!(m.selection.active, 0);
        m.move_end(false);
        assert_eq!(m.selection.active, 3);

        m.move_to(5, false);
        m.move_home(false);
        assert_eq!(m.selection.active, 4);
        m.move_end(false);
        assert_eq!(m.selection.active, 7);

        m.move_to(9, false);
        m.move_home(false);
        assert_eq!(m.selection.active, 8);
        m.move_end(false);
        assert_eq!(m.selection.active, 11);
    }

    // ── select_all / selected_text multiline ─────────────────────────

    #[test]
    fn select_all_multiline() {
        let mut m = model("hello\nworld");
        m.select_all();
        assert_eq!(m.selection.anchor, 0);
        assert_eq!(m.selection.active, 11);
        assert_eq!(m.selected_text(), "hello\nworld");
    }

    #[test]
    fn selected_text_across_lines() {
        let mut m = model("abc\ndef\nghi");
        m.selection.anchor = 2;
        m.selection.active = 6;
        assert_eq!(m.selected_text(), "c\nde");
    }

    // ── Arrow up/down with sticky column ─────────────────────────────

    #[test]
    fn move_up_down_preserves_column() {
        let mut m = model("abcdef\nab\nabcdef");
        m.move_to(5, false);
        let (_, col) = m.cursor_rowcol();
        assert_eq!(col, 5);

        m.move_down(false, Some(5));
        assert_eq!(m.cursor_rowcol(), (1, 2));

        m.move_down(false, Some(5));
        assert_eq!(m.cursor_rowcol(), (2, 5));
    }

    #[test]
    fn move_up_from_first_line_snaps_to_start() {
        let mut m = model("hello\nworld");
        m.move_to(3, false);
        let moved = m.move_up(false, None);
        assert!(!moved);
        assert_eq!(m.selection.active, 0);
    }

    #[test]
    fn move_down_from_last_line_snaps_to_end() {
        let mut m = model("hello\nworld");
        m.move_to(8, false);
        let moved = m.move_down(false, None);
        assert!(!moved);
        assert_eq!(m.selection.active, m.grapheme_count());
    }

    // ── Delete selection spanning multiple lines ─────────────────────

    #[test]
    fn delete_selection_multiline() {
        let mut m = model("abc\ndef\nghi");
        m.selection.anchor = 2;
        m.selection.active = 9;
        m.delete_selection();
        assert_eq!(m.text(), "abhi");
        assert_eq!(m.selection.active, 2);
    }

    #[test]
    fn delete_selection_same_line() {
        let mut m = model("hello world");
        m.selection.anchor = 5;
        m.selection.active = 11;
        m.delete_selection();
        assert_eq!(m.text(), "hello");
    }

    // ── Empty document edge cases ────────────────────────────────────

    #[test]
    fn empty_model_state() {
        let m = TextModel::new();
        assert_eq!(m.text(), "");
        assert_eq!(m.grapheme_count(), 0);
        assert_eq!(m.line_count(), 1);
        assert_eq!(m.line(0), "");
        assert_eq!(m.flat_to_rowcol(0), (0, 0));
        assert_eq!(m.rowcol_to_flat(0, 0), 0);
    }

    // ── set_value splits on newlines ─────────────────────────────────

    #[test]
    fn set_value_splits_lines() {
        let mut m = TextModel::new();
        m.set_value("line1\nline2\nline3".to_string());
        assert_eq!(m.line_count(), 3);
        assert_eq!(m.line(0), "line1");
        assert_eq!(m.line(1), "line2");
        assert_eq!(m.line(2), "line3");
        assert_eq!(m.text(), "line1\nline2\nline3");
    }

    // ── Trailing newline ─────────────────────────────────────────────

    #[test]
    fn trailing_newline() {
        let mut m = TextModel::new();
        m.insert("hello");
        m.insert("\n");
        assert_eq!(m.text(), "hello\n");
        assert_eq!(m.line_count(), 2);
        assert_eq!(m.line(0), "hello");
        assert_eq!(m.line(1), "");
        let (row, col) = m.cursor_rowcol();
        assert_eq!(row, 1);
        assert_eq!(col, 0);
    }

    // ── Multi-line paste ─────────────────────────────────────────────

    #[test]
    fn insert_multiline_text() {
        let mut m = TextModel::new();
        m.insert("line1\nline2\nline3");
        assert_eq!(m.text(), "line1\nline2\nline3");
        assert_eq!(m.line_count(), 3);
        assert_eq!(m.selection.active, m.grapheme_count());
    }
}
