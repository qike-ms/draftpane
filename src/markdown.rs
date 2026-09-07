use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::safety::{parser_input, printable};

pub fn render(source: &str) -> Vec<Line<'static>> {
    let safe = parser_input(source);
    let parser = Parser::new_ext(
        &safe,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut output = Vec::new();
    let mut spans = Vec::<Span<'static>>::new();
    let mut style = Style::default();
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
                spans.push(Span::styled(
                    marker,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                style = Style::default().add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(&mut output, &mut spans);
                output.push(Line::default());
                style = Style::default();
            }
            Event::Start(Tag::Strong) => style = style.add_modifier(Modifier::BOLD),
            Event::End(TagEnd::Strong) => style = style.remove_modifier(Modifier::BOLD),
            Event::Start(Tag::Emphasis) => style = style.add_modifier(Modifier::ITALIC),
            Event::End(TagEnd::Emphasis) => style = style.remove_modifier(Modifier::ITALIC),
            Event::Start(Tag::Strikethrough) => style = style.add_modifier(Modifier::CROSSED_OUT),
            Event::End(TagEnd::Strikethrough) => {
                style = style.remove_modifier(Modifier::CROSSED_OUT)
            }
            Event::Start(Tag::BlockQuote(_)) => spans.push(Span::raw("│ ")),
            Event::End(TagEnd::BlockQuote(_)) => flush(&mut output, &mut spans),
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush(&mut output, &mut spans);
            }
            Event::Start(Tag::Item) => spans.push(Span::raw(format!(
                "{}• ",
                "  ".repeat(list_depth.saturating_sub(1))
            ))),
            Event::End(TagEnd::Item) => flush(&mut output, &mut spans),
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut output, &mut spans);
                in_code_block = true;
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    output.push(Line::styled(
                        format!("── {} ──", printable(&language)),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                flush(&mut output, &mut spans);
                in_code_block = false;
                output.push(Line::default());
            }
            Event::Code(text) => spans.push(Span::styled(
                printable(&text),
                style.add_modifier(Modifier::REVERSED),
            )),
            Event::Text(text) => {
                let text = printable(&text);
                if in_code_block {
                    for (index, line) in text.split('\n').enumerate() {
                        if index > 0 {
                            flush(&mut output, &mut spans);
                        }
                        spans.push(Span::styled(
                            line.to_owned(),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                } else {
                    spans.push(Span::styled(text, style));
                }
            }
            Event::SoftBreak | Event::HardBreak => flush(&mut output, &mut spans),
            Event::Rule => {
                flush(&mut output, &mut spans);
                output.push(Line::raw("────────────────────────"));
            }
            Event::TaskListMarker(checked) => {
                spans.push(Span::raw(if checked { "[x] " } else { "[ ] " }))
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush(&mut output, &mut spans);
                output.push(Line::default());
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                spans.push(Span::styled(printable(&html), style))
            }
            _ => {}
        }
    }
    flush(&mut output, &mut spans);
    while output.last().is_some_and(|line| line.spans.is_empty()) {
        output.pop();
    }
    if output.is_empty() {
        output.push(Line::default());
    }
    output
}

fn flush(output: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>) {
    output.push(Line::from(std::mem::take(spans)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

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
