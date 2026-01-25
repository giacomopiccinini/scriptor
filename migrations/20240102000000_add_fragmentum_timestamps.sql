-- Add timestamp columns for tracking fragment position within recording
-- timestamp_start: offset in seconds from the start of the recording
-- timestamp_end: offset in seconds from the start of the recording
-- Existing rows will have NULL values for these columns

ALTER TABLE fragmentum ADD COLUMN timestamp_start REAL;
ALTER TABLE fragmentum ADD COLUMN timestamp_end REAL;
