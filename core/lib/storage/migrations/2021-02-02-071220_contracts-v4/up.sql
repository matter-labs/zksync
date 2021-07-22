ALTER TABLE blocks
    ADD timestamp bigint,
    ADD commitment BYTEA NOT NULL default '\x0000000000000000000000000000000000000000000000000000000000000000';
ALTER TABLE pending_block
    ADD timestamp bigint,
    ADD previous_root_hash BYTEA NOT NULL default '\x0000000000000000000000000000000000000000000000000000000000000000';

-- commit blocks, execute blocks, proof blocks, verify blocks on ethereum
CREATE TABLE aggregate_operations
(
    id          bigserial PRIMARY KEY,
    action_type TEXT                     NOT NULL,
    arguments   jsonb                    NOT NULL,
    from_block  bigint                   not null,
    to_block    bigint                   not null,
    created_at  TIMESTAMP with time zone NOT NULL DEFAULT now(),
    confirmed   bool                     NOT NULL DEFAULT false
);

CREATE TABLE eth_aggregated_ops_binding
(
    id        bigserial PRIMARY KEY,
    op_id     bigserial NOT NULL REFERENCES aggregate_operations (id),
    eth_op_id bigserial NOT NULL REFERENCES eth_operations (id)
);

CREATE TABLE eth_unprocessed_aggregated_ops
(
    op_id bigserial NOT NULL REFERENCES aggregate_operations (id),
    PRIMARY KEY (op_id)
);

CREATE TABLE prover_job_queue
(
    id           serial primary key,
    job_status   int                      not null,
    job_priority int                      not null,
    job_type     text                     not null,

    created_at   timestamp with time zone not null default now(),
    updated_by   text                     not null,
    updated_at   timestamp with time zone not null default now(),

    first_block  bigint                   not null,
    last_block   bigint                   not null,
    job_data     jsonb                    not null
);

CREATE TABLE aggregated_proofs
(
    first_block bigint                   not null,
    last_block  bigint                   not null,
    created_at  timestamp with time zone not null default now(),
    proof       jsonb                    not null,
    PRIMARY KEY (first_block, last_block)
);

ALTER TABLE data_restore_events_state
    ADD contract_version INT DEFAULT (0);

ALTER TABLE data_restore_events_state
    ALTER COLUMN contract_version SET NOT NULL;

CREATE TYPE eth_account_type AS ENUM ('Owned', 'CREATE2');

CREATE TABLE eth_account_types
(
    account_id   BIGINT PRIMARY KEY,
    account_type eth_account_type NOT NULL
);

ALTER TABLE txs_batches_signatures
    DROP CONSTRAINT txs_batches_signatures_pkey;
ALTER TABLE txs_batches_signatures
    ADD COLUMN id SERIAL PRIMARY KEY;

ALTER TABLE eth_parameters
    RENAME COLUMN "commit_ops" TO "last_committed_block";
ALTER TABLE eth_parameters
    RENAME COLUMN "verify_ops" TO "last_verified_block";
ALTER TABLE eth_parameters
    RENAME COLUMN "withdraw_ops" TO "last_executed_block";

UPDATE eth_parameters SET last_executed_block = (SELECT last_verified_block FROM eth_parameters WHERE id = true);
