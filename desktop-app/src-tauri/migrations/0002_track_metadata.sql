ALTER TABLE tracks ADD COLUMN genre TEXT;
ALTER TABLE tracks ADD COLUMN year INTEGER CHECK (year IS NULL OR year BETWEEN 1 AND 9999);
ALTER TABLE tracks ADD COLUMN track_number INTEGER CHECK (track_number IS NULL OR track_number > 0);