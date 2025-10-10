CREATE TABLE refresh_tokens (
    selector TEXT PRIMARY KEY NOT NULL,
    verifier_hash TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_refresh_tokens_expires_at
    ON refresh_tokens (expires_at);
