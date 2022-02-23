create or replace function unique_users(after timestamp, before timestamp)
    returns table
            (
                total bigint
            )
    language plpgsql
as
$$
begin
    return query (
        select count(distinct address) from tx_filters where tx_hash in (
            select tx_hash
            from executed_transactions
            where success = true
              and created_at BETWEEN after AND before
            union
            select tx_hash
            from executed_priority_operations
            where created_at BETWEEN after AND before
        )    );
end;
$$;
