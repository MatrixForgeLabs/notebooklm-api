# Security

## Credential Handling

`storage_state.json` contains active authenticated session cookies.
Treat it like a password.

Recommendations:
- Store outside shared directories.
- Restrict permissions (`chmod 600`).
- Never commit to git.
- Rotate by re-login if exposure is suspected.

## Logging

Do not log:
- cookie headers
- CSRF tokens
- full raw response payloads with sensitive session metadata

## Network

This project targets NotebookLM endpoints over HTTPS.
Use trusted environments and avoid MITM proxies unless explicitly required.

## Threat Model Notes

This is an unofficial reverse-engineered client.
Upstream behavior can change without notice, which may impact security assumptions.
Re-validate after upstream changes.
