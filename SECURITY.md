# Security Policy

## Supported Versions

Security fixes are applied to the latest released minor version.

## Reporting a Vulnerability

Do not file public issues for suspected vulnerabilities.

Please report privately with:
- affected version/commit
- reproduction steps
- impact assessment

If private contact is unavailable for your fork/deployment, open a minimal issue requesting a private channel without disclosing exploit details.

## Secrets Handling

`storage_state.json` contains active NotebookLM session cookies and must be treated as a secret.

Minimum practices:
- keep file permissions restricted (`chmod 600`)
- never commit auth material to VCS
- rotate/re-authenticate immediately after suspected exposure

See `docs/security.md` for operational details.
