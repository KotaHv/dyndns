CREATE TABLE auth_secrets (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    secret TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
