use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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

#[derive(Default)]
struct TableRow {
    cells: Vec<Vec<Span<'static>>>,
    header: bool,
}

struct TableState {
    alignments: Vec<Alignment>,
    rows: Vec<TableRow>,
    current: TableRow,
}

impl TableState {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            rows: Vec::new(),
            current: TableRow::default(),
        }
    }

    fn begin_row(&mut self, header: bool) {
        self.finish_row();
        self.current.header = header;
    }

    fn finish_cell(&mut self, spans: &mut Vec<Span<'static>>) {
        self.current.cells.push(std::mem::take(spans));
    }

    fn finish_row(&mut self) {
        if !self.current.cells.is_empty() {
            self.rows.push(std::mem::take(&mut self.current));
        }
    }

    fn render(mut self, output: &mut Vec<Line<'static>>, max_width: Option<usize>) {
        self.finish_row();
        let columns = self
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0)
            .max(self.alignments.len());
        if columns == 0 {
            return;
        }
        let mut widths = vec![0usize; columns];
        for row in &self.rows {
            for (column, cell) in row.cells.iter().enumerate() {
                widths[column] = widths[column].max(cell_width(cell));
            }
        }
        widths.iter_mut().for_each(|width| *width = (*width).max(1));
        if let Some(max_width) = max_width {
            let Some(fitted) = fit_table_widths(&widths, max_width) else {
                output.push(table_too_wide_line(columns, max_width));
                return;
            };
            widths = fitted;
        }
        push_table_rule(output, &widths, '┌', '┬', '┐');
        for (index, row) in self.rows.iter().enumerate() {
            output.push(render_table_row(row, &widths, &self.alignments));
            if row.header && index + 1 < self.rows.len() {
                push_table_rule(output, &widths, '├', '┼', '┤');
            }
        }
        push_table_rule(output, &widths, '└', '┴', '┘');
    }
}

fn cell_width(cell: &[Span<'static>]) -> usize {
    cell.iter().map(|span| span.content.width()).sum()
}

fn fit_table_widths(natural: &[usize], max_width: usize) -> Option<Vec<usize>> {
    let content_budget =
        max_width.checked_sub(natural.len().saturating_mul(3).saturating_add(1))?;
    if content_budget < natural.len() {
        return None;
    }
    if natural.iter().sum::<usize>() <= content_budget {
        return Some(natural.to_vec());
    }

    let mut low = 1usize;
    let mut high = natural.iter().copied().max().unwrap_or(1);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if natural
            .iter()
            .map(|width| (*width).min(middle))
            .sum::<usize>()
            <= content_budget
        {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let mut fitted = natural
        .iter()
        .map(|width| (*width).min(low))
        .collect::<Vec<_>>();
    let mut spare = content_budget.saturating_sub(fitted.iter().sum::<usize>());
    for (width, natural_width) in fitted.iter_mut().zip(natural) {
        if spare == 0 {
            break;
        }
        if *width < *natural_width {
            *width += 1;
            spare -= 1;
        }
    }
    Some(fitted)
}

fn table_too_wide_line(columns: usize, max_width: usize) -> Line<'static> {
    let message = format!("[table: {columns} columns; widen pane]");
    let visible = message.chars().take(max_width).collect::<String>();
    Line::styled(visible, theme::table_border())
}

fn truncate_cell(cell: &[Span<'static>], max_width: usize) -> Vec<Span<'static>> {
    if cell_width(cell) <= max_width {
        return cell.to_vec();
    }
    let content_limit = max_width.saturating_sub(1);
    let mut used = 0usize;
    let mut truncated = Vec::new();
    let mut ellipsis_style = theme::table_cell();
    'spans: for span in cell {
        ellipsis_style = span.style;
        let mut text = String::new();
        for grapheme in span.content.graphemes(true) {
            let width = grapheme.width();
            if used.saturating_add(width) > content_limit {
                if !text.is_empty() {
                    truncated.push(Span::styled(text, span.style));
                }
                break 'spans;
            }
            text.push_str(grapheme);
            used = used.saturating_add(width);
        }
        if !text.is_empty() {
            truncated.push(Span::styled(text, span.style));
        }
    }
    truncated.push(Span::styled("…", ellipsis_style));
    debug_assert!(cell_width(&truncated) <= max_width);
    truncated
}

fn push_table_rule(
    output: &mut Vec<Line<'static>>,
    widths: &[usize],
    left: char,
    join: char,
    right: char,
) {
    let mut rule = String::new();
    rule.push(left);
    for (index, width) in widths.iter().enumerate() {
        rule.push_str(&"─".repeat(width.saturating_add(2)));
        rule.push(if index + 1 == widths.len() {
            right
        } else {
            join
        });
    }
    output.push(Line::styled(rule, theme::table_border()));
}

fn render_table_row(row: &TableRow, widths: &[usize], alignments: &[Alignment]) -> Line<'static> {
    let mut rendered = vec![Span::styled("│", theme::table_border())];
    for (column, width) in widths.iter().enumerate() {
        let cell = row.cells.get(column).map(Vec::as_slice).unwrap_or(&[]);
        let cell = truncate_cell(cell, *width);
        let content_width = cell_width(&cell);
        let remaining = width.saturating_sub(content_width);
        let alignment = alignments.get(column).copied().unwrap_or(Alignment::None);
        let (left, right) = match alignment {
            Alignment::Right => (remaining, 0),
            Alignment::Center => (remaining / 2, remaining - remaining / 2),
            Alignment::None | Alignment::Left => (0, remaining),
        };
        let base = if row.header {
            theme::table_header()
        } else {
            theme::table_cell()
        };
        rendered.push(Span::styled(format!(" {}", " ".repeat(left)), base));
        rendered.extend(cell.iter().cloned().map(|mut span| {
            span.style = table_inline_style(base, span.style);
            span
        }));
        rendered.push(Span::styled(format!("{} ", " ".repeat(right)), base));
        rendered.push(Span::styled("│", theme::table_border()));
    }
    Line::from(rendered).style(theme::body())
}

fn table_inline_style(base: Style, inline: Style) -> Style {
    let body = theme::body();
    let mut overlay = Style::default()
        .add_modifier(inline.add_modifier)
        .remove_modifier(inline.sub_modifier);
    if inline.fg != body.fg
        && let Some(color) = inline.fg
    {
        overlay = overlay.fg(color);
    }
    if inline.bg != body.bg
        && let Some(color) = inline.bg
    {
        overlay = overlay.bg(color);
    }
    base.patch(overlay)
}

pub fn render(source: &str) -> Vec<Line<'static>> {
    render_with_width(source, None)
}

pub fn render_with_width(source: &str, max_width: Option<usize>) -> Vec<Line<'static>> {
    let safe = parser_input(source);
    let parser = Parser::new_ext(
        &safe,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES,
    );
    let mut output = Vec::new();
    let mut spans = Vec::<Span<'static>>::new();
    let mut inline = InlineStyle::default();
    let mut list_depth = 0usize;
    let mut in_code_block = false;
    let mut table: Option<TableState> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Table(alignments)) => {
                if !spans.is_empty() {
                    flush(&mut output, &mut spans);
                }
                table = Some(TableState::new(alignments));
            }
            Event::End(TagEnd::Table) => {
                if let Some(table) = table.take() {
                    table.render(&mut output, max_width);
                    output.push(Line::styled("", theme::body()));
                }
            }
            Event::Start(Tag::TableHead) => {
                if let Some(table) = table.as_mut() {
                    table.begin_row(true);
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(table) = table.as_mut() {
                    table.finish_row();
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(table) = table.as_mut() {
                    table.begin_row(false);
                }
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(table) = table.as_mut() {
                    table.finish_row();
                }
            }
            Event::Start(Tag::TableCell) => {}
            Event::End(TagEnd::TableCell) => {
                if let Some(table) = table.as_mut() {
                    table.finish_cell(&mut spans);
                }
            }
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
                if in_code_block {
                    // Split parser-preserved newlines before `printable`; otherwise
                    // they become visible ␊ symbols and the whole block wraps as one line.
                    for (index, line) in text.split('\n').enumerate() {
                        if index > 0 {
                            flush(&mut output, &mut spans);
                        }
                        spans.push(Span::styled(printable(line), theme::code_block()));
                    }
                } else {
                    spans.push(Span::styled(printable(&text), inline.current()));
                }
            }
            Event::SoftBreak | Event::HardBreak if table.is_some() => {
                spans.push(Span::styled(" ", inline.current()));
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
            Event::End(TagEnd::Paragraph) if table.is_some() => {}
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
    fn renders_gfm_table_with_borders_padding_and_alignment() {
        let rendered = render("| Name | Count |\n| :--- | ---: |\n| 界 | 7 |\n| longer | 42 |");
        let plain = rendered
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert_eq!(plain[0], "┌────────┬───────┐");
        assert_eq!(plain[1], "│ Name   │ Count │");
        assert_eq!(plain[2], "├────────┼───────┤");
        assert_eq!(plain[3], "│ 界     │     7 │");
        assert_eq!(plain[4], "│ longer │    42 │");
        assert_eq!(plain[5], "└────────┴───────┘");
        assert_eq!(plain.len(), 6);
    }

    #[test]
    fn fits_wide_tables_without_wrapping_borders() {
        let rendered = render_with_width(
            "| First heading 👨‍👩‍👧‍👦 | Second heading |\n| - | - |\n| lengthy content | more lengthy content |",
            Some(24),
        );
        assert!(rendered.iter().all(|line| line.width() <= 24));
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content.contains('…'))
        );
        assert!(rendered.iter().any(|line| {
            let text = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            text.starts_with('┌') && text.ends_with('┐')
        }));
    }

    #[test]
    fn gives_clear_fallback_when_columns_cannot_fit() {
        let rendered = render_with_width("| A | B | C |\n| - | - | - |\n| 1 | 2 | 3 |", Some(8));
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].width() <= 8);
        assert!(rendered[0].spans[0].content.starts_with("[table:"));
    }

    #[test]
    fn renders_empty_cells_and_center_alignment() {
        let rendered = render("| A | B | C |\n| - | :-: | - |\n| x | y | |");
        let plain = rendered
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(plain.iter().any(|line| line == "│ x │ y │   │"));
    }

    #[test]
    fn table_cells_keep_inline_styles_and_safe_text() {
        let rendered = render(
            "| A | B | C |\n| - | - | - |\n| **bold** | `code` | [link](https://example.invalid) bad\x1b]52;c;x\x07 |",
        );
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.content == "bold"
                    && span.style.add_modifier.contains(Modifier::BOLD)
                    && span.style.bg == theme::table_cell().bg)
        );
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| { span.content == "code" && span.style.fg == theme::inline_code().fg })
        );
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| { span.content == "link" && span.style.fg == theme::link().fg })
        );
        assert!(
            rendered
                .iter()
                .flat_map(|line| &line.spans)
                .all(|span| !span.content.chars().any(char::is_control))
        );
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
    fn fenced_flow_diagram_preserves_lines_and_down_arrows() {
        let source = "```text\nCONNECT\ncloud API · SSH · PXE\n        ↓\nQUALIFY INFRASTRUCTURE\nGPU/CPU/memory · drivers · network fabric · storage · health\n        ↓\nFORM THE KUBERNETES FOUNDATION\n```";
        let plain = render_with_width(source, Some(78))
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(plain.iter().any(|line| line == "CONNECT"));
        assert!(plain.iter().any(|line| line == "        ↓"));
        assert!(plain.iter().any(|line| line == "QUALIFY INFRASTRUCTURE"));
        assert_eq!(plain.iter().filter(|line| line.contains('↓')).count(), 2);
        assert!(plain.iter().all(|line| !line.contains('␊')));
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
