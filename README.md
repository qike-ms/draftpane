# DraftPane

A small, security-first terminal Markdown editor with a live side-by-side preview. It is designed for modern terminals, including Ghostty, without depending on terminal-specific APIs. The high-contrast preview theme brings familiar dark documentation styling to terminal cells.

> **Status:** MVP. Back up important files and review the documented limitations.

## Why

DraftPane keeps the useful editing model of split-pane Markdown tools while treating every document as untrusted input. Document control characters are neutralized before rendering, saves detect external conflicts, and the MVP has no network, link-launching, PDF, plugin, or broad syntax-parser surface.

This is an independent implementation. It does not copy source from SDF or Ghostty.

## Install

Recommended: install a prebuilt, checksummed release binary. Cargo and Rust are not required.

```bash
curl --proto '=https' --tlsv1.2 -fsSLO https://github.com/qike-ms/draftpane/releases/latest/download/install.sh
sh install.sh
rm install.sh
```

The installer supports macOS and Linux on arm64 and x86-64, verifies the release archive against `SHA256SUMS`, and installs to `~/.local/bin`. To pin an immutable release:

```bash
sh install.sh --version v0.3.0
```

To build or contribute, install Rust 1.88 or newer and use the locked source build documented in [SAMPLE_COMMANDS.md](SAMPLE_COMMANDS.md).

## Use

```bash
draftpane README.md
```

| Key | Action |
|---|---|
| `Ctrl+S` | Atomically save |
| `Ctrl+Q` | Quit; press twice to discard unsaved changes |
| Mouse wheel over editor | Scroll editor and synchronized preview |
| `Ctrl+D` / `Ctrl+U` | Scroll editor and synchronized preview down/up |
| Arrow keys, Home, End | Move cursor; preview follows editor position |
| Enter, Backspace, Delete, Tab | Edit |

The layout is horizontal at 80 columns or wider and stacked in narrower terminals.

## MVP boundaries

- Opens one regular, non-symlink UTF-8 Markdown file, up to 1 MiB.
- Renders headings, emphasis, links, lists, block quotes, code, rules, and task markers with a high-contrast dark palette. The terminal controls the font; DraftPane can select color and text attributes only.
- Scrolls the editor with the mouse wheel and synchronizes preview progress proportionally with editor scrolling/cursor movement.
- Does not open links or make network requests.
- Does not support mouse cursor placement, selection, undo, search, clipboard integration, PDF, syntax highlighting, configuration, or automatic file reload yet.
- Saves compare the current file with the opened/saved baseline and refuse known conflicts. A concurrent writer can still race the final replacement; keep backups.
- Atomic replacement preserves existing permission bits and CRLF style, but extended attributes and ownership behavior remain platform-dependent.
- Cursor movement operates on Unicode scalar values rather than grapheme clusters, so combining characters and multi-code-point emoji may require multiple keypresses.
- Release checksums detect corrupted or mismatched downloads. GitHub release hosting remains the distribution trust root; signed artifacts and provenance attestations are release-hardening work.

See [DESIGN.md](docs/DESIGN.md), [ARCHITECTURE.md](docs/ARCHITECTURE.md), and [SAMPLE_COMMANDS.md](SAMPLE_COMMANDS.md).

## Security

See [SECURITY.md](SECURITY.md). Please report vulnerabilities privately rather than opening a public issue.

## License

Licensed under either Apache-2.0 or MIT, at your option.
