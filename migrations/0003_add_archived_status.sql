-- Add 'archived' to the event_status enum so past events can be kept
-- as a record instead of being deleted. Apply with psql (autocommit);
-- ALTER TYPE ... ADD VALUE cannot run inside a transaction block.
ALTER TYPE event_status ADD VALUE 'archived';
