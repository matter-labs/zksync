-- Ongoing multiblock proving jobs.
CREATE TABLE prover_multiblock_runs (
    id serial PRIMARY KEY,
    block_number_from BIGINT NOT NULL,
    block_number_to BIGINT NOT NULL,
    worker TEXT,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now(),
    updated_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);
-- Stored proofs for the multiblocks.
CREATE TABLE multiblock_proofs (
    id serial PRIMARY KEY,
    block_from bigserial,
    block_to bigserial,
    proof jsonb NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);
CREATE TABLE verify_multiproof_queue_elements (
    id bigserial PRIMARY KEY,
    verify_multiblock_info jsonb NOT NULL,
    sended_to_eth bool NOT NULL DEFAULT false
);
