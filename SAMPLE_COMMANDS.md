# Sample Commands

## Install and use

1. Download the installer.

```bash
curl --proto '=https' --tlsv1.2 -fsSLO https://github.com/qike-ms/draftpane/releases/latest/download/install.sh
# Expect: `install.sh` exists in the current directory
```

2. Install the latest checksummed release without Cargo.

```bash
sh install.sh && rm install.sh
# Expect: `Installed DraftPane at .../.local/bin/draftpane`
```

3. Start DraftPane on a Markdown file.

```bash
$HOME/.local/bin/draftpane README.md
# Expect: an editor pane and a high-contrast live preview; left click moves the cursor, mouse-wheel scrolling moves both panes, and Markdown tables have borders
```

4. Uninstall DraftPane.

```bash
installer=$(mktemp "${TMPDIR:-/tmp}/draftpane-install.XXXXXX") && curl --proto '=https' --tlsv1.2 -fsSL https://github.com/qike-ms/draftpane/releases/latest/download/install.sh -o "$installer" && sh "$installer" --uninstall; rc=$?; rm -f "$installer"; exit $rc
# Expect: `Removed .../.local/bin/draftpane`
```

## Develop from source

Run these from the repository root with Rust 1.88 or newer.

5. Start DraftPane on its README.

```bash
cargo run --locked -- README.md
# Expect: an editor pane and a live preview pane
```

6. Run tests.

```bash
cargo test --locked
# Expect: `test result: ok`
```

7. Check formatting and lint rules.

```bash
cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings
# Expect: exit status 0 with no warnings
```

8. Build the release binary.

```bash
cargo build --locked --release
# Expect: `target/release/draftpane` exists
```

9. Audit locked dependencies after installing `cargo-audit` once.

```bash
cargo audit
# Expect: no known vulnerability errors
```

## Common failures

| Failure | Fix |
|---|---|
| `usage: draftpane <markdown-file>` | Supply exactly one file path. |
| File is not valid UTF-8 | Convert it to UTF-8 before opening. |
| File exceeds 1 MiB | Use another editor or reduce the file size. |
| Installer reports `unsupported platform` | Use macOS/Linux on arm64/x86-64 or build from source. |
| `draftpane: command not found` after install | Add `$HOME/.local/bin` to `PATH` or use the full path. |
| Installer reports checksum failure | Delete the download and retry; do not bypass verification. |
| Save reports an external change | Preserve the buffer, reopen/compare the disk file, then retry. |
| Terminal display remains altered after a crash | Run `reset`; then report the failing path and terminal version. |
