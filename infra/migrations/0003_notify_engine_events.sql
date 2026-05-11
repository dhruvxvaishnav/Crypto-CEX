CREATE OR REPLACE FUNCTION notify_engine_event_insert()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
  PERFORM pg_notify('engine_events', NEW.seq::TEXT);
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_engine_events_notify_insert ON engine_events;

CREATE TRIGGER trg_engine_events_notify_insert
AFTER INSERT ON engine_events
FOR EACH ROW
EXECUTE FUNCTION notify_engine_event_insert();
