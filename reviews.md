# Multi-Model Review — v0.3.0

**Verdicts:** APPROVE×2 / infrastructure failure×1

## Consensus issues resolved

- Synchronized scrolling originally used the document's last row rather than the reachable editor viewport range. It now maps viewport endpoints exactly.
- Preview height originally approximated wrapping. It now uses Ratatui's renderer-compatible `line_count` API.
- Mouse-wheel input originally moved only the cursor. It now moves the editor viewport immediately and keeps the cursor visible.
- Mouse capture originally lacked panic-path cleanup. The panic hook now disables capture before terminal restoration.

## Unique catches resolved

- Removed unused pane state that would fail warning-as-error CI.
- Styled trailing blank preview rows are trimmed and regression-tested.
- Wheel events no longer hide an armed destructive-quit warning; a successful scroll explicitly disarms confirmation.
- Theme language and licensing no longer imply copied third-party code.

## Verification

- Two independent final reviewers returned plain `APPROVE` on the current revision.
- The third reviewer failed because its read-only execution stream encountered a provider/tool protocol error; its earlier actionable findings were resolved.
- Local formatting, 30 tests, Clippy with warnings denied, RustSec audit, and diff checks pass.

## Net recommendation

Ship v0.3.0.

---

# Multi-Model Review — v0.4.0

**Final verdicts:** APPROVE×3

## Consensus issues resolved

- Wide tables originally wrapped borders in narrow panes. Rendering now receives the preview width, fits columns to available terminal cells, truncates content with an ellipsis, and shows a compact fallback when even minimum columns cannot fit.
- Table base styles originally overrode semantic inline colors. Header/cell backgrounds now retain link and inline-code colors plus text modifiers.
- Rendered, clicked, and cursor-position horizontal offsets now share the same bounded scroll value.

## Unique catches resolved

- Table truncation now follows Unicode extended grapheme clusters and verifies its final display width.
- Click mapping now follows extended grapheme clusters, including emoji presentation and skin-tone sequences, while preserving source scalar indices for the editor.
- In-table line and paragraph breaks no longer emit output outside the table.
- Preview width changes invalidate the table render cache.

## Verification

- Three independent reviewers approved the final implementation after fixes.
- Local formatting, 41 tests, Clippy with warnings denied, RustSec audit, and diff checks pass.
- Coverage includes scrolled click coordinates, wide/sanitized characters, click clamping, empty and aligned table cells, narrow panes, Unicode table truncation, semantic inline styles, and terminal-control neutralization.

## Net recommendation

Ship v0.4.0.

---

# Multi-Model Review — v0.5.0

**Final verdicts:** APPROVE×2 / infrastructure failure×1

## Consensus issues resolved

- The updater now starts from a minimal environment with a fixed system `PATH`; the installer independently hardens command lookup and curl configuration.
- Downloads have explicit size and time limits, checksums are verified, and archives must contain exactly two regular files.
- Checksummed release metadata prevents downgrades without executing the downloaded binary.
- Release builds fail when the tag and compiled binary version differ.

## Unique catches resolved

- `draftpane -- update` opens a document whose filename collides with the command name.
- Non-Unix update attempts return a clear unsupported-platform error.
- Installer tests cover a poisoned inherited `PATH` and downgrade rejection.
- Documentation accurately names the limited environment passed to the embedded installer.

## Verification

- Two independent final reviewers returned plain `APPROVE` on the current revision.
- The third reviewer failed because its read-only execution stream encountered a provider/tool protocol error; its earlier material findings were independently verified and resolved.
- Local formatting, 45 tests, Clippy with warnings denied, RustSec audit, installer smoke tests, downgrade rejection, shell syntax, and diff checks pass. ShellCheck remains enforced in Linux CI.

## Net recommendation

Ship v0.5.0.

---

# Focused Review — v0.5.1

**Final verdict:** APPROVE

## Issue resolved

- Fenced code blocks previously sanitized their complete parser event before splitting lines, turning embedded newlines into visible `␊` symbols. Flow diagrams consequently collapsed into one wrapped line and misplaced arrows.
- Code-block text now splits on parser-preserved newlines first, then sanitizes each line independently. Terminal controls remain neutralized.

## Verification

- The affected B3 flow was rendered from the full source document at the actual preview width: all five down arrows appeared on separate, correctly indented lines, with no visible newline symbols.
- Added a regression test for line boundaries, arrow placement, and terminal-safe output.
- Independent review approved the fix.
- Formatting, 46 tests, and Clippy with warnings denied pass.

## Net recommendation

Ship v0.5.1.
