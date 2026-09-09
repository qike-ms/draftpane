use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    safety::{parser_input, printable},
    theme,
};

#[derive(Default)]
struct InlineStyle {
    heading: Option<HeadingLevel>,
    strong: bool,
    emphasis: bool,
    strikethrough: bool,
    quote_depth: usize,
    link_depth: usize,
}

impl InlineStyle {
    fn current(&self) -> Style {
        let mut style = self.heading.map(theme::heading).unwrap_or_else(theme::body);
        if self.quote_depth > 0 && self.heading.is_none() {
            style = style.patch(theme::quote());
        }
        if self.link_depth > 0 {
            style = style.patch(theme::link());
        }
        if self.strong {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.emphasis {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if self.strikethrough {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }
        style
    }
}

pub fn render(source: &str) -> Vec<Line<'static>> {
    let safe = parser_input(source);
    let parser = Parser::new_ext(
        &safe,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut output = Vec::new();
    let mut spans = Vec::<Span<'static>>::new();
    let mut inline = InlineStyle::default();
    let mut list_depth = 0usize;
    let mut in_code_block = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut output, &mut spans);
                let marker = match level {
                    HeadingLevel::H1 => "# ",
                    HeadingLevel::H2 => "## ",
                    HeadingLevel::H3 => "### ",
                    HeadingLevel::H4 => "#### ",
                    HeadingLevel::H5 => "##### ",
                    HeadingLevel::H6 => "###### ",
                };
                spans.push(Span::styled(marker, theme::heading_marker(level)));
                inline.heading = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(&mut output, &mut spans);
                output.push(Line::styled("", theme::body()));
                inline.heading = None;
            }
            Event::Start(Tag::Strong) => inline.strong = true,
            Event::End(TagEnd::Strong) => inline.strong = false,
            Event::Start(Tag::Emphasis) => inline.emphasis = true,
            Event::End(TagEnd::Emphasis) => inline.emphasis = false,
            Event::Start(Tag::Strikethrough) => inline.strikethrough = true,
            Event::End(TagEnd::Strikethrough) => inline.strikethrough = false,
            Event::Start(Tag::Link { .. }) => inline.link_depth += 1,
            Event::End(TagEnd::Link) => inline.link_depth = inline.link_depth.saturating_sub(1),
            Event::Start(Tag::BlockQuote(_)) => {
                inline.quote_depth += 1;
                spans.push(Span::styled("│ ", theme::quote_marker()));
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush(&mut output, &mut spans);
                inline.quote_depth = inline.quote_depth.saturating_sub(1);
            }
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush(&mut output, &mut spans);
            }
            Event::Start(Tag::Item) => spans.push(Span::styled(
                format!("{}• ", "  ".repeat(list_depth.saturating_sub(1))),
                theme::list_marker(),
            )),
            Event::End(TagEnd::Item) => flush(&mut output, &mut spans),
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut output, &mut spans);
                in_code_block = true;
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    output.push(Line::styled(
                        format!("── {} ──", printable(&language)),
                        theme::code_label(),
                    ));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                flush(&mut output, &mut spans);
                in_code_block = false;
                output.push(Line::styled("", theme::body()));
            }
            Event::Code(text) => spans.push(Span::styled(printable(&text), theme::inline_code())),
            Event::Text(text) => {
                let text = printable(&text);
                if in_code_block {
                    for (index, line) in text.split('\n').enumerate() {
                        if index > 0 {
                            flush(&mut output, &mut spans);
                        }
                        spans.push(Span::styled(line.to_owned(), theme::code_block()));
                    }
                } else {
                    spans.push(Span::styled(text, inline.current()));
                }
            }
            Event::SoftBreak | Event::HardBreak => flush(&mut output, &mut spans),
            Event::Rule => {
                flush(&mut output, &mut spans);
                output.push(Line::styled("────────────────────────", theme::rule()));
            }
            Event::TaskListMarker(checked) => spans.push(Span::styled(
                if checked { "[x] " } else { "[ ] " },
                theme::task(checked),
            )),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush(&mut output, &mut spans);
                output.push(Line::styled("", theme::body()));
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                spans.push(Span::styled(printable(&html), theme::html()))
            }
            _ => {}
        }
    }
    flush(&mut output, &mut spans);
    while output
        .last()
        .is_some_and(|line| line.spans.iter().all(|span| span.content.is_empty()))
    {
        output.pop();
    }
    if output.is_empty() {
        output.push(Line::styled("", theme::body()));
    }
    output
}

fn flush(output: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    output.push(Line::from(std::mem::take(spans)).style(theme::body()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, style::Color, widgets::Paragraph};

    #[test]
    fn renders_basic_markdown() {
        let rendered = render("# Title\n\n- **bold**\n- item");
        let plain = rendered
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("# Title"));
        assert!(plain.contains("• bold"));
        assert!(
            rendered
                .last()
                .is_some_and(|line| { line.spans.iter().any(|span| !span.content.is_empty()) })
        );
    }

    #[test]
    fn trims_styled_trailing_blank_lines() {
        let rendered = render("paragraph\n\n");
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].spans[0].content, "paragraph");
    }

    #[test]
    fn applies_prominent_semantic_colors() {
        let rendered =
            render("# Title\n\n> quote\n\n- item\n\n`code` and [link](https://example.invalid)");
        let styles = rendered
            .iter()
            .flat_map(|line| &line.spans)
            .filter_map(|span| span.style.fg)
            .collect::<Vec<_>>();
        assert!(styles.iter().any(|color| matches!(color, Color::Rgb(..))));
        assert!(
            styles
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                >= 4
        );
    }

    #[test]
    fn never_retains_control_bytes() {
        let rendered = render("[x](https://example.invalid)\t\x1b]52;c;SGk=\x07\u{009b}");
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .all(|span| !span.content.chars().any(char::is_control))
        );
    }

    #[test]
    fn rendered_terminal_cells_never_contain_controls() {
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let payload = "[link\x1b]8;;file:///tmp/x\x1b\\](x)\x1b]52;c;SGk=\x07\x1bPq\x1b\\";
        terminal
            .draw(|frame| frame.render_widget(Paragraph::new(render(payload)), frame.area()))
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| !cell.symbol().chars().any(char::is_control))
        );
    }
}
