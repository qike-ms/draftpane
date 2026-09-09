# DraftPane Architecture

**Status:** Accepted for MVP  
**Style:** Layered single-process application

## Context

DraftPane is a local terminal application. Its trust boundary is unusual: the terminal is a privileged output device with control protocols, while the opened document is untrusted. Rendering must therefore produce typed terminal cells through Ratatui and must never interpolate raw document bytes into terminal escape output.

```text
untrusted file / keystrokes
          │
          ▼
  Document + Editor ──► Markdown events
          │                   │
          └────► safety::printable ◄────┘
                              │
                              ▼
                      Ratatui cells/styles
                              │
                              ▼
                    Crossterm ──► terminal
```

## Components

### `main.rs` — process boundary

- Parses the single path argument.
- Initializes/restores terminal state.
- Returns contextual errors to stderr only after terminal restoration.

It owns no editor or rendering logic.

### `document.rs` — filesystem boundary

- Opens bounded, regular UTF-8 files.
- Tracks saved content and whether the target existed at the baseline.
- Writes to a new same-directory file, syncs, and renames.
- Owns dirty-state truth and save conflicts.

No UI code accesses the filesystem directly.

### `editor.rs` — text state machine

- Stores text as Unicode strings split into logical lines.
- Translates key events into deterministic state transitions.
- Exposes cursor and complete text snapshots.

It does not render terminal escapes, parse Markdown, or write files.

### `markdown.rs` — semantic preview

- Parses safe Markdown with `pulldown-cmark`.
- Converts events to Ratatui `Line`/`Span` values.
- Ignores link destinations and treats HTML as inert text.
- Applies semantic styles from `theme.rs`; no document content can choose a color or terminal protocol.

### `theme.rs` — terminal theme

- Defines a high-contrast dark documentation palette with Ratatui colors and modifiers.
- Gives headings, quotes, lists, links, tasks, and code visually distinct styles.
- Does not attempt font selection: terminal emulators such as Ghostty own the font for terminal cells.

It never invokes a browser, shell, or URI handler.

### `safety.rs` — mandatory output policy

- Preserves newline/tab only in parser input, then converts every C0/DEL/C1 character to printable Unicode before terminal-cell output.
- Is intentionally small, pure, and exhaustively unit-tested.
- Is applied before Markdown parsing and to path/status strings.

This is defense in depth: safe input enters the parser, then parser-provided text is sanitized again when spans are produced.

### `app.rs` — orchestration and presentation

- Owns `Document` and `Editor` instances.
- Runs the event/draw loop.
- Chooses responsive pane layout.
- Routes save/quit/keyboard/mouse-scroll commands.
- Tracks rendered pane rectangles so mouse-wheel input affects only the editor pane.
- Derives preview scroll proportionally from editor viewport progress and wrapped preview height.
- Builds widgets exclusively from sanitized strings and typed styles.

Business rules remain in `Document`, `Editor`, and `safety`, which allows tests without a real terminal.

## Dependency direction

```text
main ─► app ─► document
          ├──► editor
          ├──► markdown ─► safety
          │         └────► theme
          ├──────────────► theme
          └──────────────► safety
```

Modules are cohesive and acyclic. Infrastructure (`crossterm`, filesystem) remains at boundaries. There is no plugin interface or speculative abstraction in the MVP.

## Key decisions

### ADR-001: Rust and typed terminal rendering

**Decision:** Rust 2024, Ratatui, and Crossterm.

**Why:** memory-safe application code, one distributable binary, explicit terminal lifecycle, and a typed cell/style representation that avoids constructing ANSI from documents.

**Tradeoff:** the custom MVP editor starts with fewer text-editing features than mature widgets.

### ADR-002: Independent, terminal-portable implementation

**Decision:** support Ghostty through standard terminal capabilities rather than importing Ghostty code or APIs.

**Why:** Ghostty-specific integration is unnecessary for split panes, true-color styles, and keyboard events. Standard behavior also supports Kitty, WezTerm, iTerm2, and common Linux terminals.

**Tradeoff:** no Ghostty-only graphics or shader integration.

### ADR-003: Markdown event rendering, not HTML

**Decision:** use `pulldown-cmark` events and render terminal cells.

**Why:** avoids browser/WebView, HTML execution, CSP, local HTTP serving, and HTML-to-ANSI conversion.

**Tradeoff:** preview fidelity is intentionally lower than a browser.

### ADR-004: No links in MVP

**Decision:** discard Markdown destination URLs and render labels only.

**Why:** URI handlers are an external execution boundary. Scheme allowlists and informed confirmation need their own design and tests.

**Tradeoff:** links cannot be followed yet.

### ADR-005: Bounded complete-document model

**Decision:** hold at most one 1 MiB UTF-8 file in memory, cache the preview, and reparse only after edits.

**Why:** simplest correct MVP and easy to reason about. The hard cap makes worst-case memory and parsing behavior finite.

**Tradeoff:** not suitable for huge documents; incremental parsing is deferred until measurement proves it necessary.

### ADR-006: Optimistic conflict detection plus atomic replacement

**Decision:** compare current bytes with the saved baseline and replace through a securely created same-directory temporary file while preserving existing permission bits.

**Why:** detects changes even when timestamps collide and prevents partial writes without lock files or daemons.

**Tradeoff:** comparison is O(file size), concurrent changes between verification and rename remain possible, and replacement may not preserve ownership or extended attributes on every platform. Stronger platform-aware atomic APIs are required before stable release.

## Threat model

| Asset | Threat | Control | Residual risk |
|---|---|---|---|
| Terminal session/clipboard | OSC/CSI/DCS/APC injection | centralized control sanitization; typed cells; regression tests | terminal/library-generated escapes remain trusted dependencies |
| User document | crash/partial save | secure same-directory tempfile, sync, atomic persist, preserve permission bits | ownership/xattrs and power-loss directory durability vary |
| External editor changes | silent overwrite | byte-for-byte baseline comparison | a change racing after verification can still be overwritten |
| Availability | oversized/pathological input | bounded single-handle reads; 1 MiB cap; cached preview; no PDF/decompression/plugins | pathological Markdown within limit may still use CPU while editing |
| Host account | arbitrary link/shell execution | no link launcher, shell, plugin, or subprocess | dependency compromise remains |
| Privacy | telemetry/history leakage | no network/history/recovery in MVP | OS and terminal may retain normal process/file metadata |
| Supply chain | malicious/vulnerable crate | lockfile, explicit features, automated advisory audit | registry/account compromise, policy gaps, and unknown vulnerabilities |

## Error handling

- Domain and boundary errors return `anyhow::Result` with path/action context.
- Save conflicts are shown in the status bar and keep the dirty buffer.
- Startup/event errors restore terminal state before printing.
- No document content is logged.

## Testing strategy

- Unit tests: sanitizer, editor Unicode transitions, Markdown rendering/theme, mouse hit-testing, wrapped-height estimation, and synchronized-scroll mapping.
- Filesystem tests: UTF-8/size checks, atomic save, external conflict.
- App test: command routing and save integration.
- CI: formatting, Clippy with warnings denied, locked tests/build, dependency audit.
- Pre-release manual test in Ghostty: open/edit/preview/save/quit and malicious controls.
- Post-MVP: property tests and fuzzing for sanitizer, Markdown event conversion, and edit state machine.

## Operations and release

- `Cargo.lock` is committed and `--locked` is used in CI and release builds.
- Tagged GitHub Actions releases produce four native archives (macOS/Linux × arm64/x86-64) and `SHA256SUMS`; Linux binaries use static musl targets to avoid host glibc-version coupling.
- A portable `/bin/sh` installer for supported macOS/Linux hosts selects the native archive, verifies its checksum, and atomically installs it to `~/.local/bin`; users do not need Cargo or Rust.
- Checksums detect corruption and asset mismatches but do not independently authenticate GitHub. Signed artifacts and provenance attestations remain release-hardening work.
- No service, daemon, configuration, database, telemetry, or secret management is required.
- Security reports use the private channel in `SECURITY.md`.

## Evolution constraints

New features must preserve these invariants:

1. Untrusted strings cannot bypass `safety::printable` before reaching terminal cells.
2. Document content cannot select styles containing raw terminal protocols.
3. External processes, URLs, plugins, and parsers require explicit threat-model updates.
4. Resource-consuming formats require enforceable size/time/depth bounds.
5. Filesystem changes remain conflict-aware and use atomic replacement; race and metadata limitations stay documented.
