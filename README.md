# DraftPane

A small, security-first terminal Markdown editor with a live side-by-side preview. It is designed for modern terminals, including Ghostty, without depending on terminal-specific APIs.

> **Status:** MVP. Back up important files and review the documented limitations.

## Why

DraftPane keeps the useful editing model of split-pane Markdown tools while treating every document as untrusted input. Document control characters are neutralized before rendering, saves detect external conflicts, and the MVP has no network, link-launching, PDF, plugin, or broad syntax-parser surface.

This is an independent implementation. It does not copy source from SDF or Ghostty.

## Install from source

Requires Rust 1.88 or newer:

```bash
cargo install --locked --git https://github.com/qike-ms/draftpane --tag v0.1.0
```

Until `v0.1.0` is published, clone the repository and run:

```bash
cargo run -- README.md
```

## Use

```bash
draftpane README.md
```

| Key | Action |
|---|---|
| `Ctrl+S` | Atomically save |
| `Ctrl+Q` | Quit; press twice to discard unsaved changes |
| `Ctrl+D` / `Ctrl+U` | Scroll preview down/up |
| Arrow keys, Home, End | Move cursor |
| Enter, Backspace, Delete, Tab | Edit |

The layout is horizontal at 80 columns or wider and stacked in narrower terminals.

## MVP boundaries

- Opens one regular, non-symlink UTF-8 Markdown file, up to 1 MiB.
- Renders headings, emphasis, lists, block quotes, code, rules, and task markers.
- Does not open links or make network requests.
- Does not support mouse input, selection, undo, search, clipboard integration, PDF, syntax highlighting, configuration, or automatic file reload yet.
- Saves compare the current file with the opened/saved baseline and refuse known conflicts. A concurrent writer can still race the final replacement; keep backups.
- Atomic replacement preserves existing permission bits and CRLF style, but extended attributes and ownership behavior remain platform-dependent.
- Cursor movement operates on Unicode scalar values rather than grapheme clusters, so combining characters and multi-code-point emoji may require multiple keypresses.

See [DESIGN.md](docs/DESIGN.md), [ARCHITECTURE.md](docs/ARCHITECTURE.md), and [SAMPLE_COMMANDS.md](SAMPLE_COMMANDS.md).

## Security

See [SECURITY.md](SECURITY.md). Please report vulnerabilities privately rather than opening a public issue.

## License

Licensed under either Apache-2.0 or MIT, at your option.
