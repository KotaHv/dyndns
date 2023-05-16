-- Your SQL goes here
ALTER TABLE dyndns RENAME TO olddyndns;


CREATE TABLE dyndns (
    id INTEGER PRIMARY KEY NOT NULL,
    server TEXT NOT NULL,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    hostname TEXT NOT NULL,
    ip INTEGER NOT NULL CHECK(ip in (1, 2, 3)),
    interface TEXT NOT NULL

);

INSERT INTO dyndns (server, username, password, hostname, ip, interface) 
SELECT server, username, password, hostname, ip, interface FROM olddyndns;

DROP TABLE olddyndns;