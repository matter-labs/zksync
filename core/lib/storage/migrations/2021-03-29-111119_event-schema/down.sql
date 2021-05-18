DROP FUNCTION IF EXISTS notify_event_channel();
DROP TRIGGER IF EXISTS notify_event_listener ON events;
DROP TABLE IF EXISTS events;
DROP TYPE IF EXISTS event_type;
