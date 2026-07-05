CREATE TABLE sessions (
    id                           TEXT PRIMARY KEY,   -- UUID v4
    user_sub                    TEXT NOT NULL,
    email                       TEXT NOT NULL,
    groups                      TEXT NOT NULL,      -- JSON array
    token_id                    TEXT NOT NULL,      -- resolved at login, cached
    -- Not in the original spec's schema sketch (watari.md §6.4), but required to
    -- actually implement the silent refresh described in §6.6.
    oidc_refresh_token           TEXT,
    oidc_access_token_expires_at INTEGER,
    created_at                  INTEGER NOT NULL,
    last_seen_at                INTEGER NOT NULL,
    expires_at                  INTEGER NOT NULL
);

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
