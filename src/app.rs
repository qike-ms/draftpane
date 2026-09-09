use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::{document::Document, editor::Editor, markdown, safety::printable, theme};

const SCROLL_ROWS: usize = 3;

pub struct App {
    document: Document,
    editor: Editor,
    message: String,
    preview_scroll: u16,
    editor_scroll: usize,
    editor_horizontal_scroll: usize,
    preview: Vec<Line<'static>>,
    preview_revision: u64,
    editor_pane: Rect,
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
            message: "Ctrl+S save · Ctrl+Q quit · mouse wheel scrolls editor".into(),
            preview_scroll: 0,
            editor_scroll: 0,
            editor_horizontal_scroll: 0,
            preview,
            preview_revision: 0,
            editor_pane: Rect::default(),
            should_quit: false,
            quit_armed: false,
        })
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        terminal.draw(|frame| self.draw(frame))?;
        while !self.should_quit {
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key),
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
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
                self.scroll_editor(SCROLL_ROWS * 2, true);
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.scroll_editor(SCROLL_ROWS * 2, false);
            }
            _ if self.editor.handle_key(key) => {
                self.document.replace_text(self.editor.text());
                self.quit_armed = false;
                self.message = "Modified".into();
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !contains(self.editor_pane, mouse.column, mouse.row) {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_editor(SCROLL_ROWS, true),
            MouseEventKind::ScrollUp => self.scroll_editor(SCROLL_ROWS, false),
            _ => {}
        }
    }

    fn scroll_editor(&mut self, rows: usize, down: bool) {
        let viewport_height = self.editor_pane.height.saturating_sub(2) as usize;
        let max_scroll = self.editor.lines().len().saturating_sub(viewport_height);
        let previous_scroll = self.editor_scroll;
        self.editor_scroll = if down {
            self.editor_scroll.saturating_add(rows).min(max_scroll)
        } else {
            self.editor_scroll.saturating_sub(rows)
        };
        if self.editor_scroll == previous_scroll {
            return;
        }

        let (row, _) = self.editor.cursor();
        let visible_last = self
            .editor_scroll
            .saturating_add(viewport_height.saturating_sub(1));
        if row < self.editor_scroll {
            self.editor.set_cursor_row(self.editor_scroll);
        } else if row > visible_last {
            self.editor.set_cursor_row(visible_last);
        }
        self.quit_armed = false;
        self.message = "Editor and preview scrolled".into();
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
        self.editor_pane = panes[0];

        let editor_lines = self
            .editor
            .lines()
            .iter()
            .map(|line| Line::styled(printable(line), theme::editor()))
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
            .style(theme::editor())
            .block(
                Block::default()
                    .title(Span::styled(" Editor ", theme::pane_title()))
                    .borders(Borders::ALL)
                    .border_style(theme::pane_border()),
            )
            .scroll((
                self.editor_scroll.min(u16::MAX as usize) as u16,
                self.editor_horizontal_scroll.min(u16::MAX as usize) as u16,
            ));
        frame.render_widget(editor, panes[0]);

        let editor_height = editor_height as usize;
        let preview_height = panes[1].height.saturating_sub(2) as usize;
        let preview_width = panes[1].width.saturating_sub(2);
        let preview_line_count = Paragraph::new(self.preview.clone())
            .wrap(Wrap { trim: false })
            .line_count(preview_width)
            .max(1);
        self.preview_scroll = synced_preview_scroll(
            self.editor_scroll,
            self.editor.lines().len(),
            editor_height,
            preview_line_count,
            preview_height,
        );
        let preview = Paragraph::new(self.preview.clone())
            .style(theme::preview())
            .block(
                Block::default()
                    .title(Span::styled(" Preview ", theme::pane_title()))
                    .borders(Borders::ALL)
                    .border_style(theme::pane_border()),
            )
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
                theme::status().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("— {}", printable(&self.message)), theme::status()),
        ]);
        frame.render_widget(Paragraph::new(status_line).style(theme::status()), status);

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

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn synced_preview_scroll(
    editor_scroll: usize,
    editor_lines: usize,
    editor_height: usize,
    preview_lines: usize,
    preview_height: usize,
) -> u16 {
    let editor_max = editor_lines.saturating_sub(editor_height);
    let preview_max = preview_lines.saturating_sub(preview_height);
    if editor_max == 0 || preview_max == 0 {
        return 0;
    }
    let scroll = editor_scroll.min(editor_max).saturating_mul(preview_max) / editor_max;
    scroll.min(u16::MAX as usize) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
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

    #[test]
    fn mouse_wheel_scrolls_editor_when_pointer_is_over_left_pane() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        std::fs::write(
            &path,
            (0..20)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        let mut app = App::open(path).unwrap();
        let backend = TestBackend::new(100, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        let wheel_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse(wheel_down);
        app.handle_mouse(wheel_down);
        terminal.draw(|frame| app.draw(frame)).unwrap();

        assert_eq!(app.editor.cursor().0, SCROLL_ROWS * 2);
        assert_eq!(app.editor_scroll, SCROLL_ROWS * 2);
        assert!(app.preview_scroll > 0);
    }

    #[test]
    fn mouse_wheel_outside_editor_does_not_move_cursor() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        std::fs::write(&path, "a\nb\nc\nd").unwrap();
        let mut app = App::open(path).unwrap();
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 75,
            row: 3,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.editor.cursor().0, 0);
    }

    #[test]
    fn wheel_at_boundary_preserves_quit_warning() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        std::fs::write(&path, "one\ntwo").unwrap();
        let mut app = App::open(path).unwrap();
        let backend = TestBackend::new(100, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        let warning = app.message.clone();
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 3,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(app.message, warning);
        assert!(app.quit_armed);
    }

    #[test]
    fn successful_wheel_scroll_disarms_quit_confirmation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        let content = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content).unwrap();
        let mut app = App::open(path).unwrap();
        let backend = TestBackend::new(100, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 3,
            modifiers: KeyModifiers::NONE,
        });

        assert!(!app.quit_armed);
        assert_eq!(app.message, "Editor and preview scrolled");
    }

    #[test]
    fn preview_scroll_tracks_reachable_editor_progress_and_clamps() {
        assert_eq!(synced_preview_scroll(0, 100, 20, 200, 20), 0);
        assert_eq!(synced_preview_scroll(40, 100, 20, 200, 20), 90);
        assert_eq!(synced_preview_scroll(80, 100, 20, 200, 20), 180);
        assert_eq!(synced_preview_scroll(200, 100, 20, 200, 20), 180);
        assert_eq!(synced_preview_scroll(50, 100, 20, 10, 20), 0);
        assert_eq!(synced_preview_scroll(50, 10, 20, 200, 20), 0);
    }

    #[test]
    fn ratatui_line_count_accounts_for_word_wrapping() {
        let paragraph = Paragraph::new(vec![Line::raw("one two three"), Line::raw("")])
            .wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_count(7), 3);
        assert_eq!(paragraph.line_count(0), 0);
    }
}
