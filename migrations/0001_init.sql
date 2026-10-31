-- Amigo Oculto v2 initial schema.
-- Timestamps are unix seconds (INTEGER); dates are ISO-8601 TEXT (YYYY-MM-DD).
-- Secrets (reveal links, login links, session ids) are stored only as SHA-256 hex digests.

CREATE TABLE games (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    event_date      TEXT NOT NULL,
    budget_cents    INTEGER,
    message         TEXT,
    organizer_name  TEXT NOT NULL,
    organizer_email TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'drawn')),
    draw_round      INTEGER NOT NULL DEFAULT 0,
    drawn_at        INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX idx_games_organizer ON games(organizer_email);
CREATE INDEX idx_games_event_date ON games(event_date);
CREATE INDEX idx_games_created ON games(created_at);

CREATE TABLE participants (
    id                TEXT PRIMARY KEY,
    game_id           TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    name              TEXT NOT NULL,
    email             TEXT NOT NULL,
    wishlist          TEXT,
    receiver_id       TEXT REFERENCES participants(id) ON DELETE SET NULL,
    reveal_token_hash TEXT NOT NULL UNIQUE,
    revealed_at       INTEGER,
    position          INTEGER NOT NULL,
    created_at        INTEGER NOT NULL,
    UNIQUE (game_id, email)
);
CREATE INDEX idx_participants_game ON participants(game_id, position);
CREATE INDEX idx_participants_email ON participants(email);

CREATE TABLE exclusions (
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    a_id    TEXT NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
    b_id    TEXT NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
    PRIMARY KEY (game_id, a_id, b_id),
    CHECK (a_id < b_id)
);

CREATE TABLE verifications (
    id             TEXT PRIMARY KEY,
    email          TEXT NOT NULL,
    code_hash      TEXT NOT NULL,
    name           TEXT NOT NULL,
    event_date     TEXT NOT NULL,
    budget_cents   INTEGER,
    message        TEXT,
    organizer_name TEXT NOT NULL,
    attempts       INTEGER NOT NULL DEFAULT 0,
    expires_at     INTEGER NOT NULL,
    consumed_at    INTEGER,
    created_at     INTEGER NOT NULL
);
CREATE INDEX idx_verifications_email ON verifications(email, created_at);

CREATE TABLE login_tokens (
    token_hash TEXT PRIMARY KEY,
    email      TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('organizer', 'admin')),
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_login_tokens_expires ON login_tokens(expires_at);

CREATE TABLE sessions (
    id_hash    TEXT PRIMARY KEY,
    email      TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('organizer', 'admin')),
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

CREATE TABLE outbox (
    id              TEXT PRIMARY KEY,
    game_id         TEXT REFERENCES games(id) ON DELETE CASCADE,
    participant_id  TEXT REFERENCES participants(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,
    draw_round      INTEGER,
    to_email        TEXT NOT NULL,
    to_name         TEXT,
    reply_to        TEXT,
    subject         TEXT NOT NULL,
    text_body       TEXT NOT NULL,
    html_body       TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sent', 'failed', 'cancelled')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL,
    last_error      TEXT,
    created_at      INTEGER NOT NULL,
    sent_at         INTEGER
);
CREATE INDEX idx_outbox_queue ON outbox(status, next_attempt_at);
CREATE INDEX idx_outbox_participant ON outbox(participant_id, created_at);
CREATE INDEX idx_outbox_created ON outbox(created_at);

CREATE TABLE rate_events (
    key TEXT NOT NULL,
    at  INTEGER NOT NULL
);
CREATE INDEX idx_rate_events ON rate_events(key, at);
