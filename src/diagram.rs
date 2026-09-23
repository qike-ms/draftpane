use std::collections::{HashMap, HashSet};

use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{safety::printable, theme};

const MAX_NODES: usize = 64;
const MAX_EDGES: usize = 128;
const MAX_SUBGRAPHS: usize = 32;
const MAX_ID_CHARS: usize = 64;
const MAX_LABEL_CHARS: usize = 512;
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_LINES: usize = 4_096;

#[derive(Debug, PartialEq, Eq)]
struct Node {
    id: String,
    label_lines: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct Edge {
    from: String,
    to: String,
    label: Option<String>,
    dotted: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Subgraph {
    id: String,
    title: String,
    nodes: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct Flowchart {
    nodes: HashMap<String, Node>,
    node_order: Vec<String>,
    subgraphs: Vec<Subgraph>,
    memberships: HashMap<String, usize>,
    edges: Vec<Edge>,
}

/// Render the deliberately small, safe Mermaid subset DraftPane supports.
///
/// This accepts top-down flowcharts containing rectangular nodes, bounded
/// subgraphs, and solid or dotted directed edges. Unsupported syntax returns
/// `None` so the caller can show the source instead of guessing at semantics.
pub fn render_mermaid(source: &str, max_width: Option<usize>) -> Option<Vec<Line<'static>>> {
    if source.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let source = strip_frontmatter(source)?;
    let available = max_width.unwrap_or(80);
    if available < 8 {
        return None;
    }

    if let Some((nodes, order)) = parse_linear_flowchart(&source) {
        return render_linear_flowchart(&nodes, &order, available);
    }
    let graph = parse_flowchart(&source)?;
    render_flowchart(&graph, available)
}

fn strip_frontmatter(source: &str) -> Option<String> {
    let mut lines = source.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Some(source.to_owned());
    }
    let mut closed = false;
    let mut body = Vec::new();
    for line in lines {
        if !closed {
            if line.trim() == "---" {
                closed = true;
            }
        } else {
            body.push(line);
        }
    }
    if !closed || body.is_empty() {
        return None;
    }
    Some(body.join("\n"))
}

fn render_linear_flowchart(
    nodes: &HashMap<String, Node>,
    order: &[String],
    available: usize,
) -> Option<Vec<Line<'static>>> {
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

fn parse_flowchart(source: &str) -> Option<Flowchart> {
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

    let mut graph = Flowchart {
        nodes: HashMap::new(),
        node_order: Vec::new(),
        subgraphs: Vec::new(),
        memberships: HashMap::new(),
        edges: Vec::new(),
    };
    let mut current_subgraph = None;
    let mut known_ids = HashSet::new();

    for raw in lines {
        let line = raw.trim_end_matches(';').trim();
        if line.starts_with("%%{") || line.starts_with("click ") {
            return None;
        }
        if line.starts_with("%%") {
            continue;
        }
        if let Some(declaration) = line.strip_prefix("subgraph ") {
            if current_subgraph.is_some() || graph.subgraphs.len() >= MAX_SUBGRAPHS {
                return None;
            }
            let declaration = parse_node(declaration)?;
            if !known_ids.insert(declaration.id.clone()) {
                return None;
            }
            let title = declaration.label_lines.join(" ");
            graph.subgraphs.push(Subgraph {
                id: declaration.id,
                title,
                nodes: Vec::new(),
            });
            current_subgraph = Some(graph.subgraphs.len() - 1);
            continue;
        }
        if line == "end" {
            current_subgraph.take()?;
            continue;
        }
        if line.contains("-->") || line.contains("-.->") {
            let mut edges = parse_edges(line)?;
            if graph.edges.len().saturating_add(edges.len()) > MAX_EDGES {
                return None;
            }
            graph.edges.append(&mut edges);
            continue;
        }

        let node = parse_node(line)?;
        if graph.nodes.len() >= MAX_NODES || !known_ids.insert(node.id.clone()) {
            return None;
        }
        if let Some(group) = current_subgraph {
            graph.memberships.insert(node.id.clone(), group);
            graph.subgraphs[group].nodes.push(node.id.clone());
        }
        graph.node_order.push(node.id.clone());
        graph.nodes.insert(node.id.clone(), node);
    }

    if current_subgraph.is_some() || graph.nodes.is_empty() || graph.edges.is_empty() {
        return None;
    }
    if graph
        .edges
        .iter()
        .any(|edge| !graph.nodes.contains_key(&edge.from) || !graph.nodes.contains_key(&edge.to))
    {
        return None;
    }
    let unique_edges = graph
        .edges
        .iter()
        .map(|edge| (&edge.from, &edge.to, edge.dotted))
        .collect::<HashSet<_>>();
    if unique_edges.len() != graph.edges.len() || !is_acyclic(&graph) {
        return None;
    }
    Some(graph)
}

fn parse_edges(line: &str) -> Option<Vec<Edge>> {
    let (first, mut rest) = take_id(line)?;
    let mut from = first.to_owned();
    let mut edges = Vec::new();

    while !rest.trim().is_empty() {
        rest = rest.trim_start();
        let (dotted, after_arrow) = if let Some(after) = rest.strip_prefix("-->") {
            (false, after)
        } else if let Some(after) = rest.strip_prefix("-.->") {
            (true, after)
        } else {
            return None;
        };
        rest = after_arrow.trim_start();

        let mut label = None;
        if let Some(after_open) = rest.strip_prefix('|') {
            let close = after_open.find('|')?;
            let raw = after_open[..close].trim();
            let raw = if let Some(quoted) = raw.strip_prefix('"') {
                quoted.strip_suffix('"')?
            } else {
                raw
            };
            if raw.is_empty()
                || raw.chars().count() > MAX_LABEL_CHARS
                || raw.contains(['"', '<', '>', '[', ']'])
            {
                return None;
            }
            label = Some(printable(raw));
            rest = after_open[close + 1..].trim_start();
        }

        let (to, after_id) = take_id(rest)?;
        edges.push(Edge {
            from: from.clone(),
            to: to.to_owned(),
            label,
            dotted,
        });
        from = to.to_owned();
        rest = after_id;
    }
    (!edges.is_empty()).then_some(edges)
}

fn take_id(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    let id = input[..end].trim_end_matches(';');
    valid_id(id).then_some((id, &input[end..]))
}

fn is_acyclic(graph: &Flowchart) -> bool {
    let mut indegree = graph
        .nodes
        .keys()
        .map(|id| (id.as_str(), 0usize))
        .collect::<HashMap<_, _>>();
    for edge in &graph.edges {
        if let Some(value) = indegree.get_mut(edge.to.as_str()) {
            *value += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(id) = ready.pop() {
        visited += 1;
        for edge in graph.edges.iter().filter(|edge| edge.from == id) {
            let Some(degree) = indegree.get_mut(edge.to.as_str()) else {
                return false;
            };
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                ready.push(edge.to.as_str());
            }
        }
    }
    visited == graph.nodes.len()
}

fn render_flowchart(graph: &Flowchart, available: usize) -> Option<Vec<Line<'static>>> {
    let mut output = Vec::new();
    let ungrouped = graph
        .node_order
        .iter()
        .filter(|id| !graph.memberships.contains_key(*id))
        .collect::<Vec<_>>();
    if !ungrouped.is_empty() {
        push_section_title("Top level", available, &mut output);
        for id in ungrouped {
            render_node(graph.nodes.get(id)?, available, &mut output);
        }
    }
    for group in &graph.subgraphs {
        push_section_title(&group.title, available, &mut output);
        for id in &group.nodes {
            render_node(graph.nodes.get(id)?, available, &mut output);
        }
    }

    push_section_title("Connections", available, &mut output);
    for edge in &graph.edges {
        let (line, connector) = if edge.dotted {
            ("┄┄", "┄┄▶")
        } else {
            ("──", "──▶")
        };
        let text = if let Some(label) = &edge.label {
            format!("[{}] {line}[{label}]{connector} [{}]", edge.from, edge.to)
        } else {
            format!("[{}] {connector} [{}]", edge.from, edge.to)
        };
        for line in wrap_display(&text, available) {
            output.push(Line::styled(line, theme::diagram_arrow()));
        }
    }
    (output.len() <= MAX_OUTPUT_LINES).then_some(output)
}

fn push_section_title(title: &str, available: usize, output: &mut Vec<Line<'static>>) {
    for (index, line) in wrap_display(&printable(title), available.saturating_sub(2).max(1))
        .into_iter()
        .enumerate()
    {
        output.push(Line::from(vec![
            Span::styled(if index == 0 { "◆ " } else { "  " }, theme::diagram_arrow()),
            Span::styled(line, theme::diagram_title()),
        ]));
    }
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
    fn renders_frontmatter_subgraphs_branches_and_labeled_edges() {
        let source = r#"---
config:
  layout: elk
  themeVariables:
    fontSize: 26px
  flowchart:
    wrappingWidth: 460
  themeCSS: |
    .cluster-label text, .cluster-label span { font-size: 36px !important; font-weight: 700 !important; }
---
flowchart TB
  FOUNDRY["Foundry<br/>customer contract, durable operation record"]

  subgraph CONTROL["Control plane"]
    KSERVE["KServe LLMInferenceService"]
    ROUTE["Routing"]
    WORK["Workloads"]
    MODELCFG["Model config"]
    LWS["LeaderWorkerSet / Kubernetes"]
    KSERVE --> ROUTE
    KSERVE --> WORK
    KSERVE --> MODELCFG
    WORK --> LWS
  end

  subgraph DATA["Data plane"]
    CLIENT["Client"]
    GATEWAY["Gateway / inference router<br/>Envoy · Gateway API<br/>Inference Extension · llm-d router"]
    ENGINE["Model engine<br/>vLLM · SGLang · TensorRT-LLM"]
    GPU["GPU"]
    CLIENT --> GATEWAY --> ENGINE --> GPU
  end

  subgraph SCALING["Scaling plane"]
    WLAUTO["Workload autoscaling<br/>KEDA · HPA"]
    NODEAUTO["Node autoscaling<br/>Karpenter · cloud NAP"]
    KUBE["Kubernetes"]
    WLAUTO --> KUBE
    NODEAUTO --> KUBE
  end

  subgraph RESOURCE["Resource plane"]
    DEVICES["NVIDIA GPU Operator<br/>DRA · device drivers"]
  end

  FOUNDRY -->|"deploys model CR"| KSERVE
  ROUTE -.->|"programs"| GATEWAY
  LWS -.->|"runs"| ENGINE
  WORK -.->|"scaling policy"| WLAUTO
  WORK -.->|"GPU requests"| DEVICES"#;

        let rendered = render_mermaid(source, Some(58)).unwrap();
        let text = plain(&rendered);
        let joined = text.join("\n");
        for expected in [
            "Control plane",
            "Data plane",
            "Scaling plane",
            "Resource plane",
            "KServe LLMInferenceService",
            "Gateway / inference router",
            "NVIDIA GPU Operator",
            "[FOUNDRY]",
            "deploys model CR",
            "[KSERVE]",
            "[ROUTE]",
            "[GATEWAY]",
        ] {
            assert!(joined.contains(expected), "missing {expected:?}");
        }
        assert_eq!(joined.matches('▶').count(), 14);
        assert!(joined.contains('┄'));
        assert!(text.iter().all(|line| line.width() <= 58));
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
        assert!(render_mermaid("---\nconfig: {}\nflowchart TD\na[A]", Some(80)).is_none());
        assert!(
            render_mermaid(
                "flowchart TD\nsubgraph G[Group]\na[A]\nsubgraph H[Nested]\nb[B]\nend\nend\na --> b",
                Some(80)
            )
            .is_none()
        );
        assert!(render_mermaid("flowchart TD\na[A]\nb[B]\na --> b\nb --> a", Some(80)).is_none());
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
