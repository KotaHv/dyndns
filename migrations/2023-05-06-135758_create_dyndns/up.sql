-- Your SQL goes here
CREATE TABLE dyndns (
    id INTEGER PRIMARY KEY NOT NULL,
    server TEXT NOT NULL,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    hostname TEXT NOT NULL,
    ip INTEGER NOT NULL,
    interface TEXT NOT NULL
)