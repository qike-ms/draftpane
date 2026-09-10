use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub struct Editor {
    lines: Vec<String>,
    row: usize,
    column: usize,
    revision: u64,
}

impl Editor {
    pub fn from_text(text: &str) -> Self {
        let mut lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self {
            lines,
            row: 0,
            column: 0,
            revision: 0,
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.column)
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn set_cursor_row(&mut self, row: usize) {
        self.set_cursor(row, self.column);
    }

    pub fn set_cursor(&mut self, row: usize, column: usize) {
        self.row = row.min(self.lines.len().saturating_sub(1));
        self.column = column.min(self.lines[self.row].chars().count());
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let mut changed = false;
        match key.code {
            KeyCode::Char(ch)
                if !ch.is_control()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let byte = self.byte_index(self.row, self.column);
                self.lines[self.row].insert(byte, ch);
                self.column += 1;
                changed = true;
            }
            KeyCode::Enter => {
                let byte = self.byte_index(self.row, self.column);
                let remainder = self.lines[self.row].split_off(byte);
                self.row += 1;
                self.column = 0;
                self.lines.insert(self.row, remainder);
                changed = true;
            }
            KeyCode::Backspace if self.column > 0 => {
                let end = self.byte_index(self.row, self.column);
                let start = self.byte_index(self.row, self.column - 1);
                self.lines[self.row].replace_range(start..end, "");
                self.column -= 1;
                changed = true;
            }
            KeyCode::Backspace if self.row > 0 => {
                let current = self.lines.remove(self.row);
                self.row -= 1;
                self.column = self.lines[self.row].chars().count();
                self.lines[self.row].push_str(&current);
                changed = true;
            }
            KeyCode::Delete => {
                let length = self.lines[self.row].chars().count();
                if self.column < length {
                    let start = self.byte_index(self.row, self.column);
                    let end = self.byte_index(self.row, self.column + 1);
                    self.lines[self.row].replace_range(start..end, "");
                    changed = true;
                } else if self.row + 1 < self.lines.len() {
                    let next = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next);
                    changed = true;
                } else {
                    return false;
                }
            }
            KeyCode::Left if self.column > 0 => self.column -= 1,
            KeyCode::Left if self.row > 0 => {
                self.row -= 1;
                self.column = self.lines[self.row].chars().count();
            }
            KeyCode::Right if self.column < self.lines[self.row].chars().count() => {
                self.column += 1;
            }
            KeyCode::Right if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.column = 0;
            }
            KeyCode::Up if self.row > 0 => {
                self.row -= 1;
                self.clamp_column();
            }
            KeyCode::Down if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.clamp_column();
            }
            KeyCode::Home => self.column = 0,
            KeyCode::End => self.column = self.lines[self.row].chars().count(),
            KeyCode::Tab => {
                let byte = self.byte_index(self.row, self.column);
                self.lines[self.row].insert_str(byte, "    ");
                self.column += 4;
                changed = true;
            }
            _ => return false,
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    fn clamp_column(&mut self) {
        self.column = self.column.min(self.lines[self.row].chars().count());
    }

    fn byte_index(&self, row: usize, char_index: usize) -> usize {
        self.lines[row]
            .char_indices()
            .nth(char_index)
            .map(|(index, _)| index)
            .unwrap_or(self.lines[row].len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn edits_unicode_by_character_not_byte() {
        let mut editor = Editor::from_text("éx");
        editor.handle_key(key(KeyCode::Right));
        editor.handle_key(key(KeyCode::Backspace));
        assert_eq!(editor.text(), "x");
    }

    #[test]
    fn splits_and_joins_lines() {
        let mut editor = Editor::from_text("ab");
        editor.handle_key(key(KeyCode::Right));
        editor.handle_key(key(KeyCode::Enter));
        assert_eq!(editor.text(), "a\nb");
        editor.handle_key(key(KeyCode::Backspace));
        assert_eq!(editor.text(), "ab");
    }

    #[test]
    fn mouse_position_clamps_to_document_and_line() {
        let mut editor = Editor::from_text("long\nx");
        editor.set_cursor(99, 99);
        assert_eq!(editor.cursor(), (1, 1));
    }
}
