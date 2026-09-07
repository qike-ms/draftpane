use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Position},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::{document::Document, editor::Editor, markdown, safety::printable};

pub struct App {
    document: Document,
    editor: Editor,
    message: String,
    preview_scroll: u16,
    editor_scroll: usize,
    editor_horizontal_scroll: usize,
    preview: Vec<Line<'static>>,
    preview_revision: u64,
    should_quit: bool,
    quit_armed: bool,
}

impl App {
    pub fn open(path: PathBuf) -> Result<Self> {
        let document = Document::open(path)?;
        let editor = Editor::from_text(document.text());
        let preview = markdown::render(&editor.text());
        Ok(Self {
            document,
            editor,
            message: "Ctrl+S save · Ctrl+Q quit · Ctrl+D/U scroll preview".into(),
            preview_scroll: 0,
            editor_scroll: 0,
            editor_horizontal_scroll: 0,
            preview,
            preview_revision: 0,
            should_quit: false,
            quit_armed: false,
        })
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        terminal.draw(|frame| self.draw(frame))?;
        while !self.should_quit {
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
                terminal.draw(|frame| self.draw(frame))?;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == crossterm::event::KeyEventKind::Release {
            return;
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                if !self.document.is_dirty() || self.quit_armed {
                    self.should_quit = true;
                } else {
                    self.quit_armed = true;
                    self.message = "Unsaved changes: Ctrl+S saves; Ctrl+Q again discards".into();
                }
            }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.document.replace_text(self.editor.text());
                self.message = match self.document.save() {
                    Ok(()) => "Saved".into(),
                    Err(error) => format!("Save failed: {error}"),
                };
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_add(8);
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_sub(8);
            }
            _ if self.editor.handle_key(key) => {
                self.document.replace_text(self.editor.text());
                self.quit_armed = false;
                self.message = "Modified".into();
            }
            _ => {}
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        if self.preview_revision != self.editor.revision() {
            self.preview = markdown::render(&self.editor.text());
            self.preview_revision = self.editor.revision();
        }
        let vertical = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]);
        let [body, status] = vertical.areas(frame.area());
        let panes = if body.width >= 80 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(body)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(body)
        };

        let editor_lines = self
            .editor
            .lines()
            .iter()
            .map(|line| Line::raw(printable(line)))
            .collect::<Vec<_>>();
        let editor_height = panes[0].height.saturating_sub(2);
        let editor_width = panes[0].width.saturating_sub(2);
        let (row, column) = self.editor.cursor();
        if row < self.editor_scroll {
            self.editor_scroll = row;
        } else if row >= self.editor_scroll + editor_height as usize {
            self.editor_scroll = (row + 1).saturating_sub(editor_height as usize);
        }
        let cursor_prefix = &self.editor.lines()[row];
        let cursor_prefix = cursor_prefix
            .char_indices()
            .nth(column)
            .map(|(index, _)| &cursor_prefix[..index])
            .unwrap_or(cursor_prefix);
        let cursor_width = printable(cursor_prefix).width();
        if cursor_width < self.editor_horizontal_scroll {
            self.editor_horizontal_scroll = cursor_width;
        } else if cursor_width
            >= self
                .editor_horizontal_scroll
                .saturating_add(editor_width as usize)
        {
            self.editor_horizontal_scroll = cursor_width
                .saturating_add(1)
                .saturating_sub(editor_width as usize);
        }
        let editor = Paragraph::new(editor_lines)
            .block(Block::default().title(" Editor ").borders(Borders::ALL))
            .scroll((
                self.editor_scroll.min(u16::MAX as usize) as u16,
                self.editor_horizontal_scroll.min(u16::MAX as usize) as u16,
            ));
        frame.render_widget(editor, panes[0]);

        let preview = Paragraph::new(self.preview.clone())
            .block(Block::default().title(" Preview ").borders(Borders::ALL))
            .wrap(Wrap { trim: false })
            .scroll((self.preview_scroll, 0));
        frame.render_widget(preview, panes[1]);

        let dirty = if self.document.is_dirty() {
            "●"
        } else {
            "✓"
        };
        let path = printable(&self.document.path().display().to_string());
        let status_line = Line::from(vec![
            Span::styled(
                format!(" {dirty} {path} "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("— {}", printable(&self.message))),
        ]);
        frame.render_widget(Paragraph::new(status_line), status);

        let inner = panes[0].inner(ratatui::layout::Margin::new(1, 1));
        let visible_row = row.saturating_sub(self.editor_scroll);
        if visible_row < inner.height as usize {
            let x = inner.x.saturating_add(
                cursor_width
                    .saturating_sub(self.editor_horizontal_scroll)
                    .min(inner.width.saturating_sub(1) as usize) as u16,
            );
            let y = inner.y.saturating_add(visible_row as u16);
            frame.set_cursor_position(Position::new(x, y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saving_updates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        std::fs::write(&path, "a").unwrap();
        let mut app = App::open(path.clone()).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ab");
    }
}
