# Changelog

## v0.1.0

Initial release. Web GUI frontend for [rustypaste](https://github.com/orhun/rustypaste), adding SSO, per-group access control, and optional client-side encryption on top of rustypaste's bearer-token-only API.

### Features

- 🔐 **OpenID Connect SSO** — login flow (PKCE + state/nonce), silent token refresh, tested against Zitadel and other multi-audience OIDC providers.
- 🗂️ **Per-group token mapping** — `token-bindings.yaml` maps OIDC groups to rustypaste tokens/permissions; first matching rule wins. Admin status derived from the `delete` permission.
- 🔒 **Client-side encryption** — optional password-based AES-GCM (WebCrypto) encryption; plaintext/password never reach the server.
- 📋 Dashboard, upload, paste, and shorten-URL pages (HTMX + Askama).
- 🗑️ Paste management: paginated listing, delete via `/api/pastes/{id}`.
- 🛡️ Admin token-bindings viewer.
- 🌐 Public, SSRF-guarded decrypt endpoint for sharing encrypted links.
- ⚙️ CSRF protection, static CSP headers, and rate limiting on all mutating routes.
- 🗄️ SQLite-backed sessions and upload log, migrations run automatically at startup.
- 🐳 Docker Compose deployment path.
