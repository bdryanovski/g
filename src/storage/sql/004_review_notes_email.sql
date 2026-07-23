-- Migration 004: add author_email to review_notes
-- Allows displaying author email alongside name in review notes.
-- Never edit existing migration files — append a new migration for changes.

ALTER TABLE review_notes ADD COLUMN author_email TEXT;
