# DraftPane Design

**Status:** Accepted for MVP  
**Target release:** 0.6.0

## Problem

Terminal users need a fast way to edit Markdown while seeing its structure. Existing terminal editors can be heavily configurable, while small split-preview tools may pass document bytes into terminal output, invoke arbitrary URI handlers, parse complex formats, or install mutable/unlocked dependency graphs.

DraftPane provides the smallest useful editing loop with an explicit terminal-safety boundary.

## Users and jobs

1. A developer edits a README in Ghostty and wants immediate structural feedback without leaving the terminal.
2. An operator inspects Markdown from an untrusted issue or repository and expects it not to execute terminal control protocols.
3. A writer makes a short note and needs conflict-safe saving when another tool modifies the file.

## Goals

- Open or create one UTF-8 Markdown document.
- Edit it with familiar terminal keys.
- Show a live preview beside or below the editor.
- Preserve terminal integrity for attacker-controlled text.
- Refuse oversized input and detected conflicting saves.
- Ship as one Rust binary with a locked, audited dependency graph and a Cargo-free end-user installer.
- Work in Ghostty and other Crossterm-supported terminals.

## Non-goals for MVP

- Vim/Emacs compatibility, undo/redo, selections, search, mouse text selection
- HTML preview, images, PDF, plugins, embedded code execution
- Full Mermaid compatibility or arbitrary graph layout
- Syntax highlighting across programming languages
- Opening links, clipboard protocols, or background/implicit network access
- Multi-file navigation and configuration
- Full CommonMark visual fidelity

## Interaction design

Launch with `draftpane <path>`. Existing UTF-8 files are loaded; a missing path starts an empty buffer and is created on save. Run `draftpane update` outside the editor to explicitly fetch and install the latest verified release.

At 80 columns or wider, editor and preview each receive half the screen. Narrow terminals stack them. A one-line status bar shows path, dirty state, and the last action/error.

- `Ctrl+S`: save via same-directory temporary file and rename.
- `Ctrl+Q`: quit immediately if clean; arm discard if dirty; a second press quits.
- Left click in the editor: move the cursor to the clicked logical row and displayed character, accounting for scrolling and wide/sanitized characters.
- Mouse wheel over the editor: move the editor cursor/viewport by three logical rows; preview follows proportionally.
- `Ctrl+D` / `Ctrl+U`: move the editor cursor/viewport by six logical rows; preview follows proportionally.
- Standard cursor and text-editing keys edit the buffer; preview scroll follows the editor viewport.

The preview uses a high-contrast dark documentation palette: bright semantic heading colors, muted quote text, yellow list markers, blue links, and a distinct code background. DraftPane cannot change font family inside a terminal application; Ghostty or the user's terminal owns font selection.

## Functional requirements

### FR1 — Open safely

- Reject symbolic links, non-regular files, non-UTF-8 input, and files larger than 1 MiB.
- Return an actionable error and restore terminal state.

### FR2 — Edit

- Insert Unicode scalar values without corrupting UTF-8.
- Insert/split/join/delete lines and move by character rather than byte.
- Reflect each edit in preview and dirty status on the next frame.
- Keep the cursor visible through vertical and horizontal editor scrolling.

### FR3 — Preview

- Render common Markdown structure as prominent, semantically colored terminal cells.
- Render GFM tables with Unicode borders, content-derived column widths, header emphasis, and source alignment markers.
- Recognize fenced `mermaid` blocks and render a deliberately bounded subset—top-down linear flows, rectangular nodes, and `-->` edges—as terminal-native boxes and arrows. Unsupported syntax falls back to visible source code.
- Synchronize preview position proportionally to the editor viewport while accounting for wrapped preview rows.
- Treat inline HTML as text, not executable markup.
- Ensure every document-derived terminal cell contains only printable text; expose common deceptive Unicode formatting controls visibly.

### FR4 — Save safely

- Before replacing a target, compare its bytes with the opening/saving baseline; detect replacement or creation by another process.
- Refuse a save when another process changed the file.
- Write a securely created same-directory temporary file, flush it, preserve existing permission bits, and atomically persist it.
- Remove temporary output on failure.

### FR5 — Quit safely

- Restore raw mode and alternate screen even when startup or the event loop returns an error.
- Require two explicit quit commands to discard dirty content.

## Security requirements

1. **Terminal injection:** parser input crosses `safety::parser_input`, which preserves newline/tab only for logical layout; every string reaching a terminal cell crosses `safety::printable`, where all C0 controls become Unicode control pictures, DEL becomes `␡`, and C1 controls become `�`.
2. **No link activation:** links render as label text only. The MVP never dispatches URI handlers.
3. **Resource bounds:** 1 MiB input cap; Mermaid blocks are capped at 64 KiB, 64 nodes, 64-byte identifiers, 512-character labels, and 4,096 output lines; no PDF/decompression/plugin inputs; one in-memory document; preview is cached and recomputed only after edits.
4. **Filesystem integrity:** regular files only; conflict check; same-directory atomic replacement; no shell commands.
5. **Supply chain:** committed `Cargo.lock`; minimal features; CI formatting/lint/tests/audit; immutable release assets; checksummed Cargo-free installs rather than mutable branch installation.
6. **Privacy:** no telemetry, background network calls, history, or recovery files; only the explicit update command accesses the network.
7. **Updater integrity:** update logic is embedded in the trusted binary, rebuilds the subprocess environment from a fixed system `PATH` plus required home, temporary-directory, and TLS certificate variables and size/time-bounded HTTPS downloads from fixed GitHub Release URLs, verifies `SHA256SUMS`, validates archive contents, refuses downgrades, and atomically replaces only the running executable path.

## Acceptance criteria

- `cargo test --locked` passes.
- Tests prove OSC 52, OSC 8/CSI-class ESC input, BEL, and C1 bytes cannot survive into renderer spans or Ratatui test-backend cells.
- A Unicode character can be inserted and deleted without panic/corruption.
- Saving updates the target and clears dirty state.
- A changed-on-disk target is not overwritten.
- A file over 1 MiB, a symbolic link, and invalid UTF-8 are rejected.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo audit` pass.
- Release CI builds and tests macOS/Linux arm64/x86-64 archives, audits dependencies, publishes immutable assets plus `SHA256SUMS`, and both the portable `/bin/sh` installer and `draftpane update` verify checksums before replacement.
- Tests prove the embedded updater passes install paths as arguments without shell interpolation and neutralizes subprocess output controls.
- Tests prove left-click cursor placement accounts for viewport offsets, wide characters, and sanitization expansion.
- Tests prove mouse-wheel input only scrolls when the pointer is over the editor and that preview scroll follows editor progress.
- Tests prove GFM tables render bordered, aligned, padded, terminal-safe cells while retaining inline emphasis.
- Tests prove supported Mermaid flows render as bounded, terminal-safe boxes and arrows, and unsupported Mermaid falls back to source without semantic guessing.
- A manual Ghostty smoke test can open, edit, mouse-scroll both panes in sync, preview, save, and visibly neutralize an OSC 52 payload.

## Success metrics

For the MVP, success is quality-gated rather than growth-gated:

- Zero known critical/high dependency advisories at release.
- Zero raw control bytes from document input in rendered spans under tests/fuzz corpus.
- No data loss in tested normal-save, detected-conflict, and new-target-conflict paths; injected persist-failure testing remains release work.
- Editing remains usable across documents longer and wider than the visible pane.

## Rendering compatibility research

DraftPane follows explicit Markdown extensions rather than inferring semantics from ordinary text:

| Convention | Established behavior | DraftPane behavior |
|---|---|---|
| CommonMark fenced code | Preserve literal code; arrows such as `↓` have no diagram semantics | Styled, line-preserving code block |
| GFM tables, task lists, and strikethrough | Parse opt-in GFM extensions | Semantic terminal rendering; link destinations remain inert |
| Fenced ```` ```mermaid ```` | MarkEdit recognizes the `mermaid` info string and delegates preview to Mermaid; Mermaid defines flowchart nodes and edges | Parse a safe linear subset locally into boxes/arrows; show source for unsupported syntax |
| Mermaid rectangle node | `id["label"]` is a process/rectangle node | Unicode bordered box |
| Mermaid top-down edge | `flowchart TD`/`TB` plus `a --> b` means a directed top-down connection | Centered `↓` between boxes |

A standalone `↓` is therefore rendered as a glyph by Chrome-adjacent web renderers, CommonMark renderers such as Glamour/Glow, and MarkEdit's normal Markdown path. It becomes part of a diagram only inside an explicit diagram convention such as Mermaid. DraftPane does not reinterpret ordinary prose or `text` fences heuristically.

Primary implementation references, reviewed at pinned revisions:

- [Mermaid flowchart syntax](https://github.com/mermaid-js/mermaid/blob/94ea01f/docs/syntax/flowchart.md)
- [MarkEdit Mermaid preview detection](https://github.com/MarkEdit-app/MarkEdit/blob/45247e0/CoreEditor/src/styling/nodes/code.ts)
- [MarkEdit GFM table preview](https://github.com/MarkEdit-app/MarkEdit/blob/45247e0/CoreEditor/src/styling/nodes/table.ts)
- [Glow's Glamour rendering boundary](https://github.com/charmbracelet/glow/blob/7b2431d/ui/pager.go)
- [Glamour's AST element mapping](https://github.com/charmbracelet/glamour/blob/49df656/ansi/elements.go)

The implementation is independent Rust code using existing DraftPane parser events and Ratatui cells; no source from those projects is copied or linked into the binary.

## Alternatives considered

### Python/Textual

Rejected for MVP because the audited reference implementation showed that sophisticated rendering stacks do not automatically establish a trusted terminal-output boundary, and broad syntax extras substantially expand dependencies.

### Embed or extend Ghostty

Rejected. Ghostty is a terminal emulator, not an editor framework. Coupling to its internals would reduce portability and add complexity without improving the core editing job.

### Ratatui plus a full text-area widget

Deferred. A third-party widget would accelerate selection/undo but increases API and dependency surface. The MVP editor is intentionally small; replace it after requirements for richer editing stabilize.

### Browser/WebView preview

Rejected because HTML sanitization, local servers, browser invocation, and CSP add a larger attack surface than terminal-cell rendering.

## Delivery phases

1. **MVP 0.1:** one file, basic editing, Markdown preview, safe output, conflict-aware atomic save.
2. **Editing 0.2:** release binaries and installer; early viewport polish.
3. **Presentation 0.3:** prominent terminal theme, mouse-wheel editor scrolling, and synchronized preview.
4. **Interaction 0.4:** click-to-position cursor and bordered GFM tables.
5. **Distribution 0.5:** explicit `draftpane update` with embedded, checksummed installer logic.
6. **Diagrams 0.6:** safe, bounded terminal rendering for linear top-down Mermaid flowcharts.
7. **Editing workflow:** undo/redo, selection, search, grapheme-aware movement, file watching, and explicit reload/merge prompt.
8. **Release hardening:** fuzzing, signed binaries/checksums, provenance attestations, documented compatibility matrix.
