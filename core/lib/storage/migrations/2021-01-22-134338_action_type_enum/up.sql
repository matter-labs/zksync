CREATE TYPE action_type AS ENUM ('COMMIT', 'VERIFY');
ALTER TABLE operations
    ALTER COLUMN action_type TYPE action_type
    USING action_type::action_type;
CREATE INDEX IF NOT EXISTS operations_action_type_idx ON operations(action_type);
