DROP INDEX IF EXISTS operations_action_type_idx;
ALTER TABLE operations
    ALTER COLUMN action_type TYPE text;
DROP TYPE IF EXISTS action_type;
