-- Your SQL goes here
CREATE TABLE leader_election (
    id         bool PRIMARY KEY NOT NULL DEFAULT true,
    name       text not null,
    voted_at   timestamp not null default now()
);
