use pulldown_cmark::HeadingLevel;
use ratatui::style::{Color, Modifier, Style};

// A high-contrast palette for terminal Markdown. It follows familiar dark
// documentation conventions while leaving font selection to the terminal.
const BG: Color = Color::Rgb(13, 17, 23);
const BG_MUTED: Color = Color::Rgb(21, 27, 35);
const FG: Color = Color::Rgb(240, 246, 252);
const MUTED: Color = Color::Rgb(145, 152, 161);
const BLUE: Color = Color::Rgb(68, 147, 248);
const CYAN: Color = Color::Rgb(121, 192, 255);
const PURPLE: Color = Color::Rgb(210, 168, 255);
const GREEN: Color = Color::Rgb(126, 231, 135);
const YELLOW: Color = Color::Rgb(242, 204, 96);
const BORDER: Color = Color::Rgb(61, 68, 77);

pub fn preview() -> Style {
    Style::default().fg(FG).bg(BG)
}

pub fn editor() -> Style {
    Style::default().fg(FG).bg(BG)
}

pub fn pane_border() -> Style {
    Style::default().fg(BORDER).bg(BG)
}

pub fn pane_title() -> Style {
    Style::default()
        .fg(BLUE)
        .bg(BG)
        .add_modifier(Modifier::BOLD)
}

pub fn status() -> Style {
    Style::default().fg(MUTED).bg(BG_MUTED)
}

pub fn shortcut_key() -> Style {
    Style::default()
        .fg(CYAN)
        .bg(BG_MUTED)
        .add_modifier(Modifier::BOLD)
}

pub fn shortcut_label() -> Style {
    Style::default().fg(FG).bg(BG_MUTED)
}

pub fn body() -> Style {
    preview()
}

pub fn heading(level: HeadingLevel) -> Style {
    let color = match level {
        HeadingLevel::H1 => CYAN,
        HeadingLevel::H2 => BLUE,
        HeadingLevel::H3 => PURPLE,
        HeadingLevel::H4 => GREEN,
        HeadingLevel::H5 => YELLOW,
        HeadingLevel::H6 => MUTED,
    };
    let mut style = Style::default()
        .fg(color)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

pub fn heading_marker(level: HeadingLevel) -> Style {
    heading(level).add_modifier(Modifier::DIM)
}

pub fn quote() -> Style {
    Style::default()
        .fg(MUTED)
        .bg(BG)
        .add_modifier(Modifier::ITALIC)
}

pub fn quote_marker() -> Style {
    Style::default().fg(BLUE).bg(BG)
}

pub fn list_marker() -> Style {
    Style::default().fg(YELLOW).bg(BG)
}

pub fn inline_code() -> Style {
    Style::default().fg(CYAN).bg(BG_MUTED)
}

pub fn code_block() -> Style {
    Style::default().fg(FG).bg(BG_MUTED)
}

pub fn code_label() -> Style {
    Style::default()
        .fg(MUTED)
        .bg(BG)
        .add_modifier(Modifier::DIM)
}

pub fn link() -> Style {
    Style::default()
        .fg(BLUE)
        .bg(BG)
        .add_modifier(Modifier::UNDERLINED)
}

pub fn rule() -> Style {
    Style::default().fg(BORDER).bg(BG)
}

pub fn table_border() -> Style {
    Style::default().fg(BLUE).bg(BG)
}

pub fn table_header() -> Style {
    Style::default()
        .fg(CYAN)
        .bg(BG_MUTED)
        .add_modifier(Modifier::BOLD)
}

pub fn table_cell() -> Style {
    Style::default().fg(FG).bg(BG)
}

pub fn task(checked: bool) -> Style {
    Style::default()
        .fg(if checked { GREEN } else { MUTED })
        .bg(BG)
}

pub fn html() -> Style {
    Style::default()
        .fg(MUTED)
        .bg(BG)
        .add_modifier(Modifier::DIM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_are_more_prominent_than_body_text() {
        let heading = heading(HeadingLevel::H1);
        assert_ne!(heading.fg, body().fg);
        assert!(heading.add_modifier.contains(Modifier::BOLD));
        assert!(heading.add_modifier.contains(Modifier::UNDERLINED));
    }
}
