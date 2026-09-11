use std::collections::{HashMap, HashSet};

use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{safety::printable, theme};

const MAX_NODES: usize = 64;
const MAX_ID_CHARS: usize = 64;
const MAX_LABEL_CHARS: usize = 512;
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_LINES: usize = 4_096;

#[derive(Debug, PartialEq, Eq)]
struct Node {
    id: String,
    label_lines: Vec<String>,
}

/// Render the deliberately small, safe Mermaid subset DraftPane supports.
///
/// This accepts top-down linear flowcharts containing rectangular nodes and
/// `-->` edges. Anything else returns `None` so the caller can show the source
/// as an ordinary fenced code block instead of guessing at diagram semantics.
pub fn render_mermaid(source: &str, max_width: Option<usize>) -> Option<Vec<Line<'static>>> {
    if source.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let (nodes, order) = parse_linear_flowchart(source)?;
    let available = max_width.unwrap_or(80);
    if available < 8 {
        return None;
    }

    let mut output = Vec::new();
    for (index, id) in order.iter().enumerate() {
        let node = nodes.get(id)?;
        render_node(node, available, &mut output);
        if output.len() > MAX_OUTPUT_LINES {
            return None;
        }
        if index + 1 < order.len() {
            let padding = available.saturating_sub(1) / 2;
            output.push(Line::from(vec![
                Span::raw(" ".repeat(padding)),
                Span::styled("↓", theme::diagram_arrow()),
            ]));
        }
    }
    Some(output)
}

fn parse_linear_flowchart(source: &str) -> Option<(HashMap<String, Node>, Vec<String>)> {
    let mut lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let header = lines.next()?.trim_end_matches(';');
    if !matches!(
        header,
        "flowchart TD" | "flowchart TB" | "graph TD" | "graph TB"
    ) {
        return None;
    }

    let mut nodes = HashMap::new();
    let mut edge_order: Option<Vec<String>> = None;

    for raw in lines {
        let line = raw.trim_end_matches(';').trim();
        if line.starts_with("%%{") {
            // Mermaid directives can alter rendering and are outside our subset.
            return None;
        }
        if line.starts_with("%%") {
            continue;
        }
        if line.contains("-->") {
            if edge_order.is_some() {
                return None;
            }
            let mut chain = Vec::new();
            for id in line.split("-->").map(str::trim) {
                if chain.len() >= MAX_NODES || !valid_id(id) {
                    return None;
                }
                chain.push(id.to_owned());
            }
            if chain.len() < 2 {
                return None;
            }
            edge_order = Some(chain);
            continue;
        }

        let node = parse_node(line)?;
        if nodes.contains_key(&node.id) || nodes.len() >= MAX_NODES {
            return None;
        }
        nodes.insert(node.id.clone(), node);
    }

    if nodes.is_empty() {
        return None;
    }
    let order = edge_order?;
    if order.len() != nodes.len()
        || order.iter().collect::<HashSet<_>>().len() != order.len()
        || order.iter().any(|id| !nodes.contains_key(id))
    {
        return None;
    }
    Some((nodes, order))
}

fn parse_node(line: &str) -> Option<Node> {
    let open = line.find('[')?;
    let id = line[..open].trim();
    if !valid_id(id) || !line.ends_with(']') {
        return None;
    }
    let raw_label = line[open + 1..line.len() - 1].trim();
    let label = if let Some(quoted) = raw_label.strip_prefix('"') {
        let quoted = quoted.strip_suffix('"')?;
        if quoted.contains('"') {
            return None;
        }
        quoted
    } else {
        if raw_label.contains('"') {
            return None;
        }
        raw_label
    };
    if label.chars().count() > MAX_LABEL_CHARS
        || label.contains('[')
        || label.contains(']')
        || (label.starts_with('(') && label.ends_with(')'))
        || (label.starts_with('/') && matches!(label.chars().last(), Some('/' | '\\')))
        || (label.starts_with('\\') && matches!(label.chars().last(), Some('/' | '\\')))
        || (label.contains('<')
            && !label.contains("<br/>")
            && !label.contains("<br>")
            && !label.contains("<br />"))
    {
        return None;
    }

    let normalized = label
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n");
    if normalized.contains('<') || normalized.contains('>') {
        return None;
    }
    let label_lines = normalized
        .lines()
        .map(printable)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if label_lines.is_empty() {
        return None;
    }

    Some(Node {
        id: id.to_owned(),
        label_lines,
    })
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_CHARS
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn render_node(node: &Node, available: usize, output: &mut Vec<Line<'static>>) {
    let content_width = available.saturating_sub(4).max(1);
    let wrapped = node
        .label_lines
        .iter()
        .flat_map(|line| wrap_display(line, content_width))
        .collect::<Vec<_>>();
    let box_content_width = wrapped
        .iter()
        .map(|line| line.width())
        .max()
        .unwrap_or(1)
        .max(1);
    let box_width = box_content_width.saturating_add(4).min(available);
    let left = available.saturating_sub(box_width) / 2;
    let margin = " ".repeat(left);
    let horizontal = "─".repeat(box_width.saturating_sub(2));

    output.push(Line::from(vec![
        Span::raw(margin.clone()),
        Span::styled(format!("┌{horizontal}┐"), theme::diagram_border()),
    ]));
    for (index, text) in wrapped.iter().enumerate() {
        let padding = box_width.saturating_sub(4).saturating_sub(text.width());
        output.push(Line::from(vec![
            Span::raw(margin.clone()),
            Span::styled("│ ", theme::diagram_border()),
            Span::styled(
                text.clone(),
                if index == 0 {
                    theme::diagram_title()
                } else {
                    theme::diagram_body()
                },
            ),
            Span::styled(
                format!("{} │", " ".repeat(padding)),
                theme::diagram_border(),
            ),
        ]));
    }
    output.push(Line::from(vec![
        Span::raw(margin),
        Span::styled(format!("└{horizontal}┘"), theme::diagram_border()),
    ]));
}

fn wrap_display(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut used = 0usize;
    for grapheme in text.graphemes(true) {
        let grapheme_width = grapheme.width();
        if used > 0 && used.saturating_add(grapheme_width) > width {
            lines.push(current);
            current = String::new();
            used = 0;
        }
        if grapheme_width > width {
            continue;
        }
        current.push_str(grapheme);
        used = used.saturating_add(grapheme_width);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn renders_linear_top_down_flowchart_as_boxes() {
        let source = r#"flowchart TD
            connect["CONNECT<br/>cloud API · SSH · PXE"]
            qualify["QUALIFY INFRASTRUCTURE<br/>GPU · drivers · health"]
            connect --> qualify"#;
        let rendered = render_mermaid(source, Some(50)).unwrap();
        let text = plain(&rendered);
        assert!(text.iter().any(|line| line.contains("┌")));
        assert!(text.iter().any(|line| line.contains("│ CONNECT")));
        assert!(
            text.iter()
                .any(|line| line.contains("cloud API · SSH · PXE"))
        );
        assert_eq!(text.iter().filter(|line| line.contains('↓')).count(), 1);
        assert!(text.iter().all(|line| line.width() <= 50));
    }

    #[test]
    fn wraps_long_labels_to_the_preview_width() {
        let source = "flowchart TB\na[\"TITLE<br/>a very long description that cannot fit\"]\nb[END]\na --> b";
        let rendered = render_mermaid(source, Some(20)).unwrap();
        assert!(plain(&rendered).iter().all(|line| line.width() <= 20));
    }

    #[test]
    fn renders_the_six_node_b3_chain() {
        let source = r#"flowchart TD
connect["CONNECT<br/>cloud API · SSH · PXE"]
qualify["QUALIFY INFRASTRUCTURE<br/>GPU/CPU/memory · drivers · network fabric · storage · health"]
foundation["FORM THE KUBERNETES FOUNDATION<br/>networking · device plugins · scheduling · isolation · lifecycle"]
stack["DELIVER THE AI STACK<br/>peer-to-peer images · KubeRay · vLLM · SGLang · data/model paths"]
validate["VALIDATE THE WORKLOAD<br/>functional result · performance envelope · failure and recovery checks"]
operations["HAND OFF OPERATIONS<br/>metrics/log sinks · audit · alerting · ownership · support"]
connect --> qualify --> foundation --> stack --> validate --> operations"#;
        let rendered = render_mermaid(source, Some(50)).unwrap();
        let text = plain(&rendered);
        assert_eq!(text.iter().filter(|line| line.contains('↓')).count(), 5);
        assert!(text.iter().any(|line| line.contains("HAND OFF OPERATIONS")));
        assert!(text.iter().all(|line| line.width() <= 50));
    }

    #[test]
    fn rejects_unsupported_or_ambiguous_mermaid() {
        assert!(render_mermaid("flowchart LR\na[A] --> b[B]", Some(80)).is_none());
        assert!(render_mermaid("flowchart TD\na[A]\nb[B]", Some(80)).is_none());
        assert!(render_mermaid("sequenceDiagram\nA->>B: hi", Some(80)).is_none());
        assert!(render_mermaid("flowchart TD\na[A]\na --> missing", Some(80)).is_none());
        assert!(render_mermaid("flowchart TD\na[<script>x</script>]", Some(80)).is_none());
        assert!(render_mermaid("flowchart TD\na[(Database)]\nb[B]\na --> b", Some(80)).is_none());
        for unsupported_shape in ["a[/Lean/]", "a[\\Wide\\]", "a[/Trap\\]", "a[\\Trap/]"] {
            let source = format!("flowchart TD\n{unsupported_shape}\nb[B]\na --> b");
            assert!(render_mermaid(&source, Some(80)).is_none());
        }
        assert!(
            render_mermaid("flowchart TD\na[\"unterminated]\nb[B]\na --> b", Some(80)).is_none()
        );
        assert!(
            render_mermaid(
                "flowchart TD\n%%{init: {\"theme\":\"dark\"}}%%\na[A]\nb[B]\na --> b",
                Some(80)
            )
            .is_none()
        );
        assert!(render_mermaid("flowchart TD\na[A]\na --> a", Some(80)).is_none());
        assert!(
            render_mermaid(
                &format!(
                    "flowchart TD\n{}[A]\nb[B]\n{} --> b",
                    "a".repeat(MAX_ID_CHARS + 1),
                    "a".repeat(MAX_ID_CHARS + 1)
                ),
                Some(80)
            )
            .is_none()
        );
        assert!(render_mermaid(&"x".repeat(MAX_SOURCE_BYTES + 1), Some(80)).is_none());
    }

    #[test]
    fn neutralizes_terminal_controls_in_labels() {
        let source = "flowchart TD\na[\"SAFE\u{1b}52;c;eA==\u{7}\"]\nb[END]\na --> b";
        let rendered = render_mermaid(source, Some(80)).unwrap();
        assert!(rendered.iter().flat_map(|line| &line.spans).all(|span| {
            !span.content.chars().any(char::is_control) && !span.content.contains("\u{1b}")
        }));
    }
}
