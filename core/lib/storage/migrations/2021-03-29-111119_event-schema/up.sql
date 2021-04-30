CREATE TYPE event_type AS ENUM ('Account', 'Block', 'Transaction');

CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    block_number BIGINT NOT NULL,
    event_type event_type NOT NULL,
    event_data jsonb NOT NULL
);

CREATE OR REPLACE FUNCTION notify_event_channel() RETURNS TRIGGER AS $$
BEGIN
    PERFORM (
        SELECT pg_notify('event_channel', NEW.id::text)
    );
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notify_event_listener
AFTER INSERT ON events
FOR EACH ROW EXECUTE PROCEDURE notify_event_channel();
