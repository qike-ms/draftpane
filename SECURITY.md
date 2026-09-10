# Security Policy

## Supported versions

DraftPane is pre-release. Only the latest tagged release receives security fixes.

## Reporting

Report suspected vulnerabilities privately through GitHub's **Security → Report a vulnerability** flow. Do not include exploit details in a public issue.

Include the DraftPane commit/version, operating system, terminal and version, minimal reproduction, and observed impact. Do not send secrets or unrelated document content.

## Security model

DraftPane treats opened documents as untrusted. It neutralizes terminal control characters before rendering and does not open links, invoke shells from document content, load plugins, parse PDFs, access the network while editing, or emit telemetry.

The explicit `draftpane update` command is the sole runtime network boundary. It passes an installer embedded at build time to `/bin/sh` with an environment rebuilt from a fixed system `PATH` plus required `HOME`, `TMPDIR`, and TLS certificate variables, downloads only size-bounded fixed-name assets from this repository's GitHub Releases over HTTPS with bounded timeouts, verifies the selected archive against the release's `SHA256SUMS`, validates archive contents, refuses downgrades, and atomically replaces the running executable. It does not download and execute a remote script. Updater output is neutralized before being written to the terminal.

Security controls reduce risk but do not make arbitrary files or updates independently trustworthy. Keep backups and install only tagged release binaries whose archives pass the published SHA-256 check. Checksums detect corruption or mismatched assets but do not independently authenticate GitHub; GitHub release hosting remains the trust root, and signed artifacts and provenance attestations are not available yet.
