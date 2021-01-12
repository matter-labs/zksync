ALTER TABLE data_restore_events_state ADD contract_version INT DEFAULT(0);
UPDATE data_restore_events_state SET contract_version = 0;
ALTER TABLE data_restore_events_state ALTER COLUMN contract_version SET NOT NULL;
