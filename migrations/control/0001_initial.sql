CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL UNIQUE COLLATE NOCASE,
    google_sub TEXT UNIQUE,
    name TEXT NOT NULL DEFAULT '',
    avatar TEXT,
    hd TEXT,
    is_super_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_super_admin IN (0, 1)),
    disabled INTEGER NOT NULL DEFAULT 0 CHECK (disabled IN (0, 1)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_seen INTEGER
);

CREATE TABLE orgs (
    id INTEGER PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    storage_limit INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    deleted_at INTEGER
);

CREATE TABLE org_members (
    org_id INTEGER NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'member')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (org_id, user_id)
) WITHOUT ROWID;

CREATE INDEX org_members_user ON org_members (user_id);

CREATE TABLE invites (
    id INTEGER PRIMARY KEY,
    org_id INTEGER NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    email TEXT NOT NULL COLLATE NOCASE,
    company_role TEXT NOT NULL CHECK (company_role IN ('owner', 'member')),
    project_id INTEGER,
    project_role TEXT CHECK (project_role IN ('owner', 'member', 'observer')),
    created_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    accepted_at INTEGER,
    revoked_at INTEGER,
    CHECK ((project_id IS NULL) = (project_role IS NULL))
);

CREATE INDEX invites_email ON invites (email) WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TABLE sessions (
    id_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    csrf_token TEXT NOT NULL,
    user_agent TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX sessions_user ON sessions (user_id);

CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL DEFAULT (unixepoch()),
    actor_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    org_id INTEGER REFERENCES orgs (id) ON DELETE SET NULL,
    target TEXT,
    detail TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX audit_log_at ON audit_log (at);

CREATE TABLE user_settings (
    user_id INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    settings TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE personal_tokens (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    scopes TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER,
    expires_at INTEGER,
    revoked_at INTEGER
);

CREATE TABLE device_codes (
    code_hash TEXT PRIMARY KEY,
    user_code TEXT NOT NULL UNIQUE,
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    scopes TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER NOT NULL,
    approved_at INTEGER
) WITHOUT ROWID;

CREATE TABLE email_queue (
    id INTEGER PRIMARY KEY,
    to_addr TEXT NOT NULL,
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    body_html TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    send_after INTEGER NOT NULL DEFAULT (unixepoch()),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    sent_at INTEGER
);

CREATE INDEX email_queue_pending ON email_queue (send_after) WHERE sent_at IS NULL;
