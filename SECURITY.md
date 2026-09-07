# Security Policy

## Supported versions

DraftPane is pre-release. Only the latest tagged release receives security fixes.

## Reporting

Report suspected vulnerabilities privately through GitHub's **Security → Report a vulnerability** flow. Do not include exploit details in a public issue.

Include the DraftPane commit/version, operating system, terminal and version, minimal reproduction, and observed impact. Do not send secrets or unrelated document content.

## Security model

DraftPane treats opened documents as untrusted. It neutralizes terminal control characters before rendering and does not open links, invoke shells, load plugins, parse PDFs, access the network, or emit telemetry in the MVP.

Security controls reduce risk but do not make arbitrary files trustworthy. Keep backups and install only immutable tagged releases with a committed lockfile.
