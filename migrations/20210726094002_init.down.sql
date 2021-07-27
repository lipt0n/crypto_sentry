-- Add down migration script here
-- Add up migration script here
DROP TABLE IF EXISTS assets CASCADE;
DROP TABLE IF EXISTS pairs CASCADE;
DROP TABLE IF EXISTS ticks CASCADE;
DROP INDEX IF EXISTS pairs_idx CASCADE;