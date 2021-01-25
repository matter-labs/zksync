CREATE TYPE action_type_enum AS ENUM ('COMMIT', 'VERIFY');
ALTER TABLE operations
    ALTER COLUMN action_type TYPE action_type_enum
    USING action_type::action_type_enum;