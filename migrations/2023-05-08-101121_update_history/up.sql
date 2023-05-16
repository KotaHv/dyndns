-- Your SQL goes here
ALTER TABLE history RENAME TO oldhistory;


CREATE TABLE history (
    id INTEGER PRIMARY KEY NOT NULL,
    old_ip TEXT,
    new_ip TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version in (1, 2)),
    updated DATETIME NOT NULL
);

INSERT INTO history (old_ip, new_ip, version, updated) 
SELECT old_ip, new_ip, version, updated FROM oldhistory;

DROP TABLE oldhistory;
