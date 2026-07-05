# Technical Specification: Rustypaste Web GUI

### Rust (Axum) · HTMX · OIDC SSO · Docker Compose

---

## 1. Overview

This document specifies a web-based graphical frontend for [rustypaste](https://github.com/orhun/rustypaste), a minimal self-hosted pastebin/file-upload server. Rustypaste exposes only an HTTP API secured by a static bearer token; it has no concept of individual users, sessions, or per-user permissions.

This application is a Rust binary built with **Axum** that sits in front of rustypaste. It authenticates individual users via **OIDC** (Authorization Code flow with PKCE), maps authenticated identities to rustypaste bearer tokens, proxies all API calls server-side, and serves an **HTMX**-driven UI rendered with **Askama** templates. Client-side JavaScript is intentionally minimal — HTMX handles all dynamic interactions; a small amount of vanilla JS handles the WebCrypto-based client-side encryption feature only.

The design is intentionally OIDC-provider-agnostic. The reference deployment uses self-hosted Zitadel, but no Zitadel-specific APIs are used; any standards-compliant OIDC provider must work through environment variable configuration alone.

---

## 2. Goals and Non-Goals

### Goals

- Friendly web UI covering file upload, plain-text paste, and URL shortening.
- SSO login via OIDC Authorization Code + PKCE before any page or API is accessible.
- Per-user/per-group rustypaste token mapping, without modifying rustypaste.
- Optional password-based encryption of any upload — implemented entirely client-side using the WebCrypto API; the server and rustypaste never receive plaintext or the password.
- Single deployable Rust binary in a Docker container, alongside the existing rustypaste container.
- Provider-agnostic: switching from Zitadel to Keycloak/Authentik/Auth0/etc. requires only environment variable changes.

### Non-Goals

- Modifying or forking rustypaste. It is treated as an unmodified upstream container.
- Fine-grained per-paste ACLs (not a rustypaste concept).
- An admin UI for the OIDC provider itself.
- JavaScript-heavy SPA behaviour.

---

## 3. Architecture

```
Browser
  │  HTTPS
  ▼
┌─────────────────────────────────────────────────┐
│  Axum application (Rust binary, in Docker)       │
│                                                  │
│  ┌─ Tower middleware stack ──────────────────┐   │
│  │  - OIDC session guard (all routes)        │   │
│  │  - CSRF token validation (mutating routes)│   │
│  │  - Rate limiter                           │   │
│  └───────────────────────────────────────────┘   │
│                                                  │
│  ┌─ Route handlers ──────────────────────────┐   │
│  │  GET  /              → dashboard           │   │
│  │  GET  /upload        → file upload page   │   │
│  │  GET  /paste         → text paste page    │   │
│  │  GET  /shorten       → URL shorten page   │   │
│  │  GET  /decrypt       → decrypt page       │   │
│  │  POST /api/upload    → proxy to rustypaste│   │
│  │  POST /api/paste     → proxy to rustypaste│   │
│  │  POST /api/shorten   → proxy to rustypaste│   │
│  │  DELETE /api/[id]    → proxy to rustypaste│   │
│  │  GET  /auth/login    → OIDC redirect      │   │
│  │  GET  /auth/callback → OIDC code exchange │   │
│  │  POST /auth/logout   → session teardown   │   │
│  └───────────────────────────────────────────┘   │
│                                                  │
│  ┌─ Askama templates ────────────────────────┐   │
│  │  base.html, dashboard.html, upload.html,  │   │
│  │  paste.html, shorten.html, decrypt.html,  │   │
│  │  partials/paste_row.html, error.html      │   │
│  └───────────────────────────────────────────┘   │
│                                                  │
│  ┌─ SQLite (via sqlx) ───────────────────────┐   │
│  │  sessions  │  upload_log                  │   │
│  └───────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
  │  HTTP (internal Docker network only)
  ▼
Rustypaste container
  │  Authorization: Bearer <token resolved from session>
  ▼
Local filesystem volume (rustypaste storage)
```

The browser never communicates with rustypaste directly and never sees a rustypaste bearer token. All upload/proxy calls go through Axum route handlers that resolve the token server-side from the authenticated session.

---

## 4. Crate Dependencies

| Crate | Purpose |
|---|---|
| `axum` | Web framework, routing, extractors |
| `tokio` | Async runtime |
| `tower` / `tower-http` | Middleware (tracing, compression, timeouts) |
| `askama` | Compile-time Jinja-like HTML templates |
| `openidconnect` | OIDC/OAuth2 code flow, PKCE, JWKS validation |
| `reqwest` | HTTP client for proxying calls to rustypaste and OIDC token endpoint |
| `sqlx` | Async SQLite driver (sessions + upload log) |
| `serde` / `serde_json` | Config and JSON deserialization |
| `uuid` | Session IDs |
| `rand` | Cryptographically secure random values |
| `time` | Timestamp handling |
| `tower_governor` | Rate limiting |
| `axum-csrf` | CSRF token middleware |
| `tracing` / `tracing-subscriber` | Structured logging |
| `config` | Layered configuration (env vars + optional YAML) |

No JavaScript bundler or npm dependency is required; HTMX is loaded from a CDN or vendored as a single static file.

---

## 5. Configuration

All configuration is read at startup from environment variables (with optional override via a mounted `config.yaml`). The application fails fast with a clear error if required values are absent.

```
# OIDC
OIDC_ISSUER_URL          # e.g. https://zitadel.example.com/o/...  (required)
OIDC_CLIENT_ID           # (required)
OIDC_CLIENT_SECRET       # (required)
OIDC_REDIRECT_URI        # e.g. https://paste.example.com/auth/callback (required)
OIDC_GROUPS_CLAIM        # claim name containing user groups, default: "groups"

# Session
SESSION_SECRET           # 32+ byte hex secret for signing session IDs (required)
SESSION_TTL_SECONDS      # default: 28800 (8 hours)

# Rustypaste
RUSTYPASTE_INTERNAL_URL  # e.g. http://rustypaste:8000 (required)
RUSTYPASTE_PUBLIC_URL    # public base URL, used to validate /decrypt?url= (required)

# Token env var references (values referenced by token-bindings.yaml)
RUSTYPASTE_TOKEN_READONLY
RUSTYPASTE_TOKEN_ADMIN
# ... additional tokens as needed

# App
APP_BASE_URL             # e.g. https://paste.example.com
APP_PORT                 # default: 3000
DATABASE_PATH            # default: /data/app.db
PBKDF2_ITERATIONS        # default: 310000
```

Token bindings are defined in a YAML file mounted into the container (see section 7.1). Actual token values are read from the environment variables above and referenced by name in the YAML — never stored as plaintext in config files.

---

## 6. Authentication & OIDC Flow

### 6.1 Protocol

OpenID Connect, Authorization Code flow with PKCE, using the `openidconnect` crate. The issuer's `.well-known/openid-configuration` is fetched at startup to obtain the authorization endpoint, token endpoint, JWKS URI, and optional `end_session_endpoint`. No provider-specific code paths exist.

### 6.2 Login flow

1. `GET /auth/login` — the handler generates a PKCE challenge, state token, and nonce; stores them in a short-lived signed cookie (`__oidc_state`); and issues a `302` redirect to the IdP's authorization endpoint with `scope=openid profile email <groups_scope>`.
2. IdP authenticates the user and redirects to `GET /auth/callback?code=...&state=...`.
3. The handler validates the `state` cookie, exchanges the code for tokens via the token endpoint, validates the ID token (signature via JWKS, `aud`, `exp`, `nonce`), extracts the user's `sub`, email, and groups claim.
4. A server-side session record is written to SQLite (`sessions` table). A random `session_id` (UUID v4) is set as an `HttpOnly; Secure; SameSite=Lax` cookie — no token material ever reaches the browser.
5. Browser is redirected to `/`.

### 6.3 Session middleware (Tower layer)

A custom Tower middleware layer runs on every protected route. It reads the `session_id` cookie, looks up the session in SQLite, validates expiry, and injects a `UserSession` extension into the request. Routes extract this via Axum's `Extension<UserSession>` extractor. Unauthenticated requests are redirected to `/auth/login`.

### 6.4 Session table schema

```sql
CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,   -- UUID v4
    user_sub     TEXT NOT NULL,
    email        TEXT NOT NULL,
    groups       TEXT NOT NULL,      -- JSON array
    token_id     TEXT NOT NULL,      -- resolved at login, cached
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);
```

### 6.5 Logout

`POST /auth/logout` deletes the session row, clears the cookie, and optionally performs RP-initiated logout by redirecting to the IdP's `end_session_endpoint` (if present in the discovery document). The POST verb + CSRF token prevents logout CSRF.

### 6.6 Token refresh

If a refresh token was returned (`offline_access` scope), the session middleware silently refreshes the OIDC access token when it nears expiry, updating the session row. If refresh fails (e.g. user revoked at IdP), the session is invalidated and the user is sent to `/auth/login`.

---

## 7. User → Rustypaste Token Mapping

### 7.1 Mapping model

A `token-bindings.yaml` file is mounted read-only into the container:

```yaml
tokens:
  - id: readonly
    env_var: RUSTYPASTE_TOKEN_READONLY
    permissions: [upload, paste, shorten, view]
  - id: admin
    env_var: RUSTYPASTE_TOKEN_ADMIN
    permissions: [upload, paste, shorten, view, delete]

bindings:
  - match:
      groups: ["pastebin-admins"]
    token_id: admin
  - match:
      groups: ["pastebin-users"]
    token_id: readonly
  - default: deny   # or "readonly" to allow all authenticated users
```

At startup the application reads this file, resolves each `env_var` to its actual token value, and holds the mapping in memory. The resolved token values are never logged or returned in any HTTP response.

At login time the user's groups are matched against `bindings` (first match wins) and the resulting `token_id` is stored in the session row. Proxy route handlers read `token_id` from the session and look up the live token from the in-memory map.

### 7.2 Permission enforcement

Permissions listed in the token config are enforced at the proxy route handler level — e.g. `DELETE /api/[id]` returns `403 Forbidden` if the session's token does not include `delete`. This is a soft boundary (direct rustypaste API access bypasses it); for hard separation, provision distinct rustypaste tokens with different capabilities in rustypaste's own config.

---

## 8. Frontend: HTMX + Askama

### 8.1 Rendering model

Axum handlers render Askama templates and return `text/html` responses. HTMX attributes on HTML elements trigger partial re-renders by swapping only the relevant `<div>` or `<section>` in the page, avoiding full-page reloads for actions like submitting an upload or deleting a paste. No client-side routing; no virtual DOM; no build step.

HTMX is the only JavaScript dependency for the core UI. It is either:

- Loaded from a CDN (`https://unpkg.com/htmx.org`), or
- Vendored as a single static file served from `GET /static/htmx.min.js` (recommended for a fully self-contained deployment).

### 8.2 Template structure

```
templates/
  base.html           # <html>, <head>, nav, flash messages, CSRF meta tag
  dashboard.html      # extends base; recent uploads table
  upload.html         # extends base; file drag-drop form + options
  paste.html          # extends base; <textarea> + options
  shorten.html        # extends base; URL input + options
  decrypt.html        # extends base; password prompt + in-browser decrypt
  admin/
    tokens.html       # extends base; read-only token binding viewer
  partials/
    paste_row.html    # single <tr> for the upload log table (HTMX swap target)
    flash.html        # success/error flash message fragment
  error.html          # 4xx/5xx error page
```

All templates are compiled into the binary by Askama at build time; no template files ship in the Docker image.

### 8.3 Pages

**Dashboard (`GET /`)** — shows a paginated table of the current user's recent uploads pulled from the `upload_log` table. Each row has a copy-URL button and, for users with `delete` permission, a delete button that fires `hx-delete="/api/[id]"` and swaps the row out on success.

**File upload (`GET /upload`)** — drag-and-drop area plus file picker. Options: custom filename, expiry (dropdown mapped to rustypaste's `Expire` header), one-shot (delete after first read), password protection toggle (reveals the password/confirm fields; see section 9). Submits via `hx-post="/api/upload"` with `hx-encoding="multipart/form-data"`. On success, HTMX swaps in a flash partial containing the paste URL with a one-click copy button.

**Text paste (`GET /paste`)** — a `<textarea>` with a filename/extension field and the same expiry/one-shot/password options as file upload. Submits via `hx-post="/api/paste"`. On the server, the text content is wrapped in a `multipart/form-data` body and forwarded to rustypaste's `POST /` endpoint as a `text/plain` file.

**URL shortening (`GET /shorten`)** — a URL input with the same options. Submits via `hx-post="/api/shorten"`, proxied to rustypaste's URL-shortening endpoint. Password protection encrypts the target URL string before submission (see section 9).

**Decrypt (`GET /decrypt?url=...`)** — unauthenticated (no session required). Fetches the ciphertext from rustypaste via the server proxy, streams it to the browser, then prompts for a password. All decryption happens in the browser via vanilla JS + WebCrypto (see section 9.3). This is the only page that requires non-HTMX JavaScript.

**Admin / token bindings (`GET /admin/tokens`)** — accessible only to sessions with `admin`-mapped tokens. Renders the in-memory token binding config as a read-only HTML table for operational transparency. Token values are masked (`***`); only IDs, groups, and permissions are shown.

### 8.4 HTMX interaction patterns

- **Upload success:** server returns a `200` with a `partials/flash.html` fragment (the paste URL); HTMX swaps it into a `#result` div.
- **Upload error:** server returns `422` with a `partials/flash.html` error fragment; HTMX swaps the same target.
- **Delete row:** `hx-delete` on each row; on `200` the row is removed from the DOM via `hx-swap="outerHTML"` with an empty response.
- **Dashboard pagination:** `hx-get="/api/pastes?page=N"` on a "load more" button; server returns additional `paste_row.html` partials that are appended to the table body.
- **CSRF:** the CSRF token is embedded in the `<meta name="csrf-token">` tag in `base.html`; HTMX's `hx-headers` config (set once in a `<script>` tag in the base template) attaches it as `X-CSRF-Token` on every non-GET HTMX request.

---

## 9. Client-Side Password Encryption

Password protection is entirely opt-in and entirely client-side. The Axum server receives and forwards only opaque encrypted bytes; it never has access to the plaintext content or the password.

### 9.1 Encryption (at upload time)

The password fields are revealed by a checkbox toggle in the upload/paste/shorten forms. When enabled, a small inline `<script>` block (no framework, no bundler) intercepts the HTMX `htmx:configRequest` event to encrypt the payload before it leaves the browser:

1. Read file bytes (or UTF-8 encode the text/URL string).
2. Generate 16 random bytes as `salt` and 12 random bytes as `iv` via `crypto.getRandomValues()`.
3. Derive a 256-bit AES-GCM key using `SubtleCrypto.importKey` + `SubtleCrypto.deriveKey` with PBKDF2 (SHA-256, `PBKDF2_ITERATIONS` iterations — read from a `<meta>` tag injected by the server so it matches the server-side configured value).
4. Encrypt: `SubtleCrypto.encrypt({ name: "AES-GCM", iv }, key, plaintext)`.
5. Assemble the binary envelope:

   ```
   Offset  Length  Field
   0       4       Magic: 0x52 0x50 0x45 0x4E  ("RPEN")
   4       1       Version: 0x01
   5       16      Salt (random, per upload)
   21      12      IV (random, per upload)
   33      var     AES-GCM ciphertext + 16-byte auth tag
   ```

6. Replace the form payload with the encrypted `Blob` before HTMX sends the request. Append `.enc` to the filename.
7. The server receives this opaque blob, proxies it to rustypaste, and records `encrypted: true` in the upload log. No key material is stored server-side.

### 9.2 What the server records

The `upload_log` entry stores: original display filename (pre-`.enc`), the rustypaste-returned URL, timestamp, uploader's `user_sub`, `encrypted` boolean. The password and derived key are never transmitted to or stored by the server.

### 9.3 Decryption (at access time)

The `/decrypt` page is intentionally unauthenticated so recipients without an SSO account can decrypt shared content.

1. The server's `/decrypt` handler validates that the `url` query parameter starts with `RUSTYPASTE_PUBLIC_URL` (SSRF prevention), then proxies a GET to rustypaste and streams the raw bytes back to the browser inside an inline `<script>`-accessible variable (or as a Blob URL via a hidden `<a>`).
2. Browser JS reads the magic bytes to confirm this is an encrypted payload, then prompts for the password.
3. Re-derive the key using salt from the envelope header + entered password + same PBKDF2 parameters.
4. `SubtleCrypto.decrypt({ name: "AES-GCM", iv }, key, ciphertext)` — AES-GCM's authentication tag means a wrong password produces a hard decryption failure with no partial output.
5. If the decrypted content is text (detected by the stored filename extension before `.enc`), render it in a `<pre>` block. Otherwise offer it as a `<a download>` Blob URL.
6. Plaintext never leaves the browser.

### 9.4 Scope and caveats

- **Protected:** content at rest in rustypaste storage; the Axum server and operator cannot read encrypted pastes.
- **Not protected:** metadata (`.enc` filename, upload time, uploader identity in the app's own DB); the paste URL itself. If the URL is leaked, the ciphertext is accessible but unreadable without the password.
- Password and URL should be shared over separate channels (e.g. encrypted messaging app) for meaningful security.
- Expiry and password protection are orthogonal — both should be set for sensitive time-limited shares.

---

## 10. Data Layer (SQLite via sqlx)

### 10.1 Schema

```sql
-- Sessions (see section 6.4)
CREATE TABLE sessions ( ... );

-- Upload log
CREATE TABLE upload_log (
    id           TEXT PRIMARY KEY,    -- UUID v4
    user_sub     TEXT NOT NULL,
    email        TEXT NOT NULL,
    display_name TEXT NOT NULL,       -- original filename, text snippet, or URL
    paste_url    TEXT NOT NULL,       -- URL returned by rustypaste
    kind         TEXT NOT NULL,       -- 'file' | 'paste' | 'url'
    encrypted    INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER              -- NULL if no expiry set
);

CREATE INDEX upload_log_user ON upload_log(user_sub, created_at DESC);
```

### 10.2 Migrations

Managed with `sqlx`'s compile-time migration embedding (`sqlx::migrate!`). Migrations run automatically at startup before the server begins accepting connections.

---

## 11. Proxy Route Handlers

All rustypaste API calls are made from Axum route handlers over an internal `reqwest::Client` (connection-pooled, not created per request). The handlers:

1. Extract and validate the session (via Tower middleware).
2. Resolve the rustypaste bearer token from the in-memory token map.
3. Check that the resolved token's permissions include the required action (returning `403` otherwise).
4. Forward the request to rustypaste with `Authorization: Bearer <token>`.
5. On success, write a row to `upload_log` and return the appropriate HTMX-compatible HTML fragment.
6. On error, return an HTMX-compatible error fragment with the appropriate HTTP status code.

The `reqwest` client has a configured timeout (`RUSTYPASTE_TIMEOUT_SECS`, default 30) and a max body size (`RUSTYPASTE_MAX_BODY_BYTES`, default 100 MB, should match rustypaste's own config).

---

## 12. Security Considerations

- **PKCE** is used on every OIDC flow, even with a confidential client.
- **Session cookie** flags: `HttpOnly`, `Secure`, `SameSite=Lax`.
- **CSRF:** `axum-csrf` middleware issues a per-session token; HTMX attaches it via `X-CSRF-Token` on all non-GET requests; the middleware validates it server-side.
- **SSRF:** the `/decrypt` handler validates the `url` parameter strictly against `RUSTYPASTE_PUBLIC_URL` prefix before proxying.
- **No token leakage:** rustypaste bearer tokens are resolved from environment variables at startup, held in memory, and never written to logs, HTTP responses, or the database.
- **Audit log:** the `upload_log` table provides a per-user record of all uploads and deletions, including encrypted uploads (content unknown; the fact of the upload is logged).
- **Rate limiting:** `tower_governor` applied to `/auth/callback`, `POST /api/upload`, `POST /api/paste`, and `POST /api/shorten`.
- **Content-Security-Policy:** a strict CSP header is set on all responses; the only `script-src` values permitted are `'self'` and (if using CDN HTMX) the CDN host. Inline scripts in templates use a nonce injected per-request.
- **Encryption iteration count versioning:** the `version` byte in the encrypted payload envelope allows the PBKDF2 iteration count to be increased in future releases without breaking previously encrypted pastes (old envelopes carry their own version, decryption logic branches on it).
- **Token rotation runbook:** rotating a rustypaste token requires updating the environment variable and restarting both the Axum container and the rustypaste container; no live rotation API exists. Document as an ops runbook step.

---

## 13. Deployment (Docker Compose)

```yaml
services:
  rustypaste:
    image: ghcr.io/orhun/rustypaste:latest
    volumes:
      - rustypaste-data:/app/upload
      - ./rustypaste-config.toml:/app/config.toml:ro
    networks: [internal]
    # No published ports — unreachable from outside Docker network

  webgui:
    build: ./webgui          # Rust multi-stage build
    environment:
      OIDC_ISSUER_URL:         "https://zitadel.example.com/..."
      OIDC_CLIENT_ID:          "..."
      OIDC_CLIENT_SECRET:      "..."
      OIDC_REDIRECT_URI:       "https://paste.example.com/auth/callback"
      OIDC_GROUPS_CLAIM:       "groups"
      SESSION_SECRET:          "..."
      RUSTYPASTE_INTERNAL_URL: "http://rustypaste:8000"
      RUSTYPASTE_PUBLIC_URL:   "https://paste.example.com/r"
      RUSTYPASTE_TOKEN_READONLY: "..."
      RUSTYPASTE_TOKEN_ADMIN:    "..."
      APP_BASE_URL:            "https://paste.example.com"
      DATABASE_PATH:           "/data/app.db"
      PBKDF2_ITERATIONS:       "310000"
    volumes:
      - webgui-data:/data
      - ./token-bindings.yaml:/app/token-bindings.yaml:ro
    networks: [internal, public]
    ports:
      - "3000:3000"

networks:
  internal:
  public:

volumes:
  rustypaste-data:
  webgui-data:
```

**Docker build (multi-stage):**

```dockerfile
FROM rust:1.78 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/webgui /usr/local/bin/webgui
# Static assets (htmx.min.js etc.) if vendored
COPY --from=builder /app/static /app/static
ENTRYPOINT ["webgui"]
```

Since Askama compiles templates into the binary, the final image contains only the binary, `ca-certificates` (for outbound TLS to the IdP and rustypaste), and optionally the vendored HTMX file. Image size is typically under 30 MB.

---

## 14. Open Questions for Implementation Phase

- Exact rustypaste version to target — header names for expiry and one-shot have changed across releases; pin a version in the Compose file and test.
- Whether rustypaste requires a bearer token for GET (read) requests, or only for POST/DELETE — affects whether the `/decrypt` proxy needs a token.
- Shape of the groups/roles claim in Zitadel (flat string array vs nested object) — the `OIDC_GROUPS_CLAIM` parser needs to handle both or document its expected format.
- Whether a syntax-highlighted text viewer for decrypted pastes is in scope for v1.
- Desired retention/pruning policy for `upload_log` rows whose corresponding rustypaste files have expired.
- Whether multi-replica deployment is ever needed — if so, SQLite must be replaced with Postgres (swap the `sqlx` feature flag and connection string; no logic changes required).

---

## 15. Summary

The application is a single Rust binary (Axum + Askama + HTMX) that acts as a secure, user-aware gateway to an unmodified rustypaste instance. Server-side OIDC handles authentication; a YAML-driven token binding layer maps OIDC groups to rustypaste tokens without any changes to rustypaste's config model; all proxy calls attach the resolved token server-side so it never reaches the browser. Three upload modes (file, text paste, URL shortening) share a common optional client-side encryption path built on the WebCrypto API — the server is a zero-knowledge participant for password-protected uploads. The entire stack runs as two containers in a Docker Compose file with no external database, message broker, or JS build toolchain required.
