-- Add the 'last_attempt' column
ALTER TABLE cars ADD COLUMN last_attempt DATETIME;

-- Add the 'dead_until' column
ALTER TABLE cars ADD COLUMN dead_until DATETIME;
