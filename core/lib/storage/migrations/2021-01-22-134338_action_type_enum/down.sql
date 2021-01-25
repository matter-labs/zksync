ALTER TABLE operations
    ALTER COLUMN action_type TYPE text;
DROP TYPE IF EXISTS action_type_enum;
