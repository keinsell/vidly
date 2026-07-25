ALTER TABLE movies ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE movies ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

UPDATE movies SET created_at = '2026-07-25T00:00:00+00:00', updated_at = '2026-07-25T00:00:00+00:00' WHERE created_at = '';
