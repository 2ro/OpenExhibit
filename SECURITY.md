# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security-sensitive reports.

Use [GitHub Security Advisories](https://github.com/2ro/OpenExhibit/security/advisories/new)
— it's a private channel between you and the maintainers. You'll get a reply
within 7 days; a fix and coordinated disclosure normally follow within 30.

If you can't use GitHub Advisories, email **2ro [at] users.noreply.github.com**
with the word `SECURITY` in the subject.

## Scope

In scope: the OpenExhibit binary, its templates, its migrations, the
documented install paths in `install.sh`.

Out of scope: third-party crates (report upstream), social engineering of
operators, denial-of-service through resource consumption on a single-tenant
install, vulnerabilities in PostgreSQL / Actix-Web / Caddy itself.

## Trust model in one paragraph

An authenticated admin can write raw HTML in exhibit content, captions, and
sidebar blocks (rendered with `|safe`). The CSP forbids inline scripts and
event handlers, so the worst an admin XSS can do is restyle the page. If you
expose authoring to untrusted users, sanitize on save (`ammonia` crate).
Untrusted visitors are sandboxed: parameterised SQL throughout, argon2id
passwords, CSRF on every mutation, in-process rate limits on auth, encrypted
SMTP secrets at rest, trusted-proxy XFF allowlist.

## Supported versions

| Version | Security fixes |
|---|---|
| 0.1.x   | ✓ |

Pre-1.0 — breaking changes possible. Pin a tag in production.
