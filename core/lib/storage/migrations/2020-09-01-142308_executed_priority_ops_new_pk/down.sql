ALTER TABLE executed_priority_operations DROP CONSTRAINT executed_priority_operations_pkey;
ALTER TABLE executed_priority_operations ADD PRIMARY KEY (eth_hash);
