-- Revert tier 3 geographic pricing changes
DROP TABLE country_availability;

-- SQLite doesn't support DROP COLUMN in ALTER TABLE, so we need to recreate tables
-- Note: This is a simplified down migration. In production, you may want to preserve data.

-- For user_settings, we'll note that the columns were added but can't be easily removed in SQLite
-- Subaccounts table same issue

-- If you need to truly revert, you would need to:
-- 1. Create new table without the columns
-- 2. Copy data from old table
-- 3. Drop old table
-- 4. Rename new table

-- For now, this down migration only removes the country_availability table
-- The added columns in user_settings and subaccounts will remain but be unused
