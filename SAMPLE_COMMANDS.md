# Sample Commands

Run these from the repository root.

1. Start DraftPane on its README.

```bash
cargo run --locked -- README.md
# Expect: an editor pane and a live preview pane
```

2. Run tests.

```bash
cargo test --locked
# Expect: `test result: ok`
```

3. Check formatting and lint rules.

```bash
cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings
# Expect: exit status 0 with no warnings
```

4. Build the release binary.

```bash
cargo build --locked --release
# Expect: `target/release/draftpane` exists
```

5. Audit locked dependencies after installing `cargo-audit` once.

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
| Save reports an external change | Preserve the buffer, reopen/compare the disk file, then retry. |
| Terminal display remains altered after a crash | Run `reset`; then report the failing path and terminal version. |
