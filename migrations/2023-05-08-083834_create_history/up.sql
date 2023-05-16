-- Your SQL goes here
CREATE TABLE history (
    id INTEGER PRIMARY KEY NOT NULL,
    old_ip TEXT NOT NULL,
    new_ip TEXT NOT NULL,
    version INTEGER NOT NULL,
    updated DATETIME NOT NULL
)