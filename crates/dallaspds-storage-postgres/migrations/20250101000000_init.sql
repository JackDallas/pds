-- Actors (identity layer)
CREATE TABLE IF NOT EXISTS actor (
    did TEXT PRIMARY KEY NOT NULL,
    handle TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    takedown_ref TEXT,
    deactivated_at TIMESTAMPTZ,
    delete_after TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_actor_handle ON actor(handle);

-- Accounts (auth layer)
CREATE TABLE IF NOT EXISTS account (
    did TEXT PRIMARY KEY NOT NULL REFERENCES actor(did) ON DELETE CASCADE,
    email TEXT UNIQUE,
    email_confirmed_at TIMESTAMPTZ,
    invites_disabled INTEGER NOT NULL DEFAULT 0,
    password_hash TEXT NOT NULL,
    signing_key BYTEA NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_account_email ON account(email);

-- Repo root tracking
CREATE TABLE IF NOT EXISTS repo_root (
    did TEXT PRIMARY KEY NOT NULL REFERENCES actor(did) ON DELETE CASCADE,
    cid BYTEA NOT NULL,
    rev TEXT NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Repo blocks (MST nodes, commits, records)
CREATE TABLE IF NOT EXISTS repo_block (
    did TEXT NOT NULL,
    cid BYTEA NOT NULL,
    block BYTEA NOT NULL,
    PRIMARY KEY (did, cid)
);

-- Refresh tokens
CREATE TABLE IF NOT EXISTS refresh_token (
    id TEXT PRIMARY KEY NOT NULL,
    did TEXT NOT NULL REFERENCES actor(did) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    next_id TEXT,
    app_password_name TEXT
);
CREATE INDEX IF NOT EXISTS idx_refresh_token_did ON refresh_token(did);

-- App passwords
CREATE TABLE IF NOT EXISTS app_password (
    did TEXT NOT NULL REFERENCES actor(did) ON DELETE CASCADE,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    privileged INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (did, name)
);

-- Invite codes
CREATE TABLE IF NOT EXISTS invite_code (
    code TEXT PRIMARY KEY NOT NULL,
    available_uses INTEGER NOT NULL DEFAULT 1,
    disabled INTEGER NOT NULL DEFAULT 0,
    for_account TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS invite_code_use (
    code TEXT NOT NULL REFERENCES invite_code(code),
    used_by TEXT NOT NULL,
    used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (code, used_by)
);

-- Email tokens
CREATE TABLE IF NOT EXISTS email_token (
    purpose TEXT NOT NULL,
    did TEXT NOT NULL REFERENCES actor(did) ON DELETE CASCADE,
    token TEXT NOT NULL,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (purpose, did)
);

-- Firehose events (for sync/subscribeRepos backfill)
CREATE TABLE IF NOT EXISTS firehose_event (
    seq BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    did TEXT NOT NULL,
    payload BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_firehose_created ON firehose_event(created_at);

-- Blob metadata
CREATE TABLE IF NOT EXISTS blob_meta (
    did TEXT NOT NULL,
    cid TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (did, cid)
);
