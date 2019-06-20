CREATE TABLE active_provers (
    id              serial primary key,
    worker          text not null,
    created_at      timestamp not null default now(),
    stopped_at      timestamp
);
