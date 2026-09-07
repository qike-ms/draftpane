//! Terminal-output safety boundary.

/// Preserve logical Markdown layout while neutralizing terminal protocol and
/// deceptive Unicode formatting characters.
pub fn parser_input(input: &str) -> String {
    sanitize(input, true)
}

/// Replace every control/format character with a visible, terminal-safe symbol.
/// Strings passed to Ratatui cells must cross this function first.
pub fn printable(input: &str) -> String {
    sanitize(input, false)
}

fn sanitize(input: &str, preserve_layout: bool) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\n' | '\t' if preserve_layout => output.push(ch),
            '\0'..='\x1f' => output.push(control_picture(ch)),
            '\x7f' => output.push('\u{2421}'),
            '\u{80}'..='\u{9f}' => output.push('\u{fffd}'),
            '\u{061c}' => output.push_str("⟦ALM⟧"),
            '\u{200b}' => output.push_str("⟦ZWSP⟧"),
            '\u{200c}' => output.push_str("⟦ZWNJ⟧"),
            '\u{200d}' => output.push_str("⟦ZWJ⟧"),
            '\u{200e}' => output.push_str("⟦LRM⟧"),
            '\u{200f}' => output.push_str("⟦RLM⟧"),
            '\u{202a}' => output.push_str("⟦LRE⟧"),
            '\u{202b}' => output.push_str("⟦RLE⟧"),
            '\u{202c}' => output.push_str("⟦PDF⟧"),
            '\u{202d}' => output.push_str("⟦LRO⟧"),
            '\u{202e}' => output.push_str("⟦RLO⟧"),
            '\u{2060}' => output.push_str("⟦WJ⟧"),
            '\u{2061}' => output.push_str("⟦FA⟧"),
            '\u{2062}' => output.push_str("⟦IT⟧"),
            '\u{2063}' => output.push_str("⟦IS⟧"),
            '\u{2064}' => output.push_str("⟦IP⟧"),
            '\u{2066}' => output.push_str("⟦LRI⟧"),
            '\u{2067}' => output.push_str("⟦RLI⟧"),
            '\u{2068}' => output.push_str("⟦FSI⟧"),
            '\u{2069}' => output.push_str("⟦PDI⟧"),
            '\u{feff}' => output.push_str("⟦BOM⟧"),
            _ => output.push(ch),
        }
    }
    output
}

fn control_picture(ch: char) -> char {
    char::from_u32(0x2400 + ch as u32).unwrap_or('\u{fffd}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutralizes_terminal_sequences() {
        let payload = "safe\x1b]52;c;SGVsbG8=\x07\n\x1b[2J";
        let rendered = printable(payload);
        assert!(!rendered.chars().any(char::is_control));
        assert!(rendered.contains('␛'));
        assert!(rendered.contains('␇'));
    }

    #[test]
    fn neutralizes_osc8_dcs_apc_and_c1_controls() {
        let payload =
            "\x1b]8;;https://example.invalid\x1b\\x\x1bPq\x1b\\\x1b_payload\x1b\\\u{009b}";
        assert!(!printable(payload).chars().any(char::is_control));
    }

    #[test]
    fn exposes_deceptive_unicode_formatting() {
        let payload = "safe\u{202e}txt\u{2066}x\u{200b}";
        let rendered = printable(payload);
        assert_eq!(rendered, "safe⟦RLO⟧txt⟦LRI⟧x⟦ZWSP⟧");
        assert!(!rendered.contains('\u{202e}'));
    }

    #[test]
    fn parser_input_preserves_only_layout_controls() {
        let rendered = parser_input("a\t\x1bb\nc\u{009b}");
        assert!(rendered.contains('\t'));
        assert!(rendered.contains('\n'));
        assert!(!rendered.contains('\x1b'));
        assert!(!rendered.contains('\u{009b}'));
    }
}
