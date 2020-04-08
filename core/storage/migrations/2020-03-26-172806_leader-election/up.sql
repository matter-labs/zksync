-- Your SQL goes here
CREATE TABLE leader_election (
    id         serial primary key,
    name       text not null,
    created_at timestamp not null default now(),
    retired_at timestamp null
);
