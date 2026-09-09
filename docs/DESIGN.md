# DraftPane Design

**Status:** Accepted for MVP  
**Target release:** 0.3.0

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

- Vim/Emacs compatibility, undo/redo, selections, search, mouse cursor placement/selection
- HTML preview, images, PDF, plugins, embedded code execution
- Syntax highlighting across programming languages
- Opening links, clipboard protocols, or network access
- Multi-file navigation and configuration
- Full CommonMark visual fidelity

## Interaction design

Launch with `draftpane <path>`. Existing UTF-8 files are loaded; a missing path starts an empty buffer and is created on save.

At 80 columns or wider, editor and preview each receive half the screen. Narrow terminals stack them. A one-line status bar shows path, dirty state, and the last action/error.

- `Ctrl+S`: save via same-directory temporary file and rename.
- `Ctrl+Q`: quit immediately if clean; arm discard if dirty; a second press quits.
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
3. **Resource bounds:** 1 MiB input cap; no PDF/decompression/plugin inputs; one in-memory document; preview is cached and recomputed only after edits.
4. **Filesystem integrity:** regular files only; conflict check; same-directory atomic replacement; no shell commands.
5. **Supply chain:** committed `Cargo.lock`; minimal features; CI formatting/lint/tests/audit; immutable release assets; checksummed Cargo-free installs rather than mutable branch installation.
6. **Privacy:** no telemetry, network calls, history, or recovery files in MVP.

## Acceptance criteria

- `cargo test --locked` passes.
- Tests prove OSC 52, OSC 8/CSI-class ESC input, BEL, and C1 bytes cannot survive into renderer spans or Ratatui test-backend cells.
- A Unicode character can be inserted and deleted without panic/corruption.
- Saving updates the target and clears dirty state.
- A changed-on-disk target is not overwritten.
- A file over 1 MiB, a symbolic link, and invalid UTF-8 are rejected.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo audit` pass.
- Release CI builds and tests macOS/Linux arm64/x86-64 archives, audits dependencies, publishes immutable assets plus `SHA256SUMS`, and the portable `/bin/sh` installer verifies checksums before replacement.
- Tests prove mouse-wheel input only scrolls when the pointer is over the editor and that preview scroll follows editor progress.
- A manual Ghostty smoke test can open, edit, mouse-scroll both panes in sync, preview, save, and visibly neutralize an OSC 52 payload.

## Success metrics

For the MVP, success is quality-gated rather than growth-gated:

- Zero known critical/high dependency advisories at release.
- Zero raw control bytes from document input in rendered spans under tests/fuzz corpus.
- No data loss in tested normal-save, detected-conflict, and new-target-conflict paths; injected persist-failure testing remains release work.
- Editing remains usable across documents longer and wider than the visible pane.

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
4. **Editing workflow:** undo/redo, selection, search, grapheme-aware movement, file watching, and explicit reload/merge prompt.
5. **Release hardening:** fuzzing, signed binaries/checksums, provenance attestations, documented compatibility matrix.
