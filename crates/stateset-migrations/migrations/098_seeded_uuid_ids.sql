-- Hyphenate the ids the early migrations seeded as bare hex.
--
-- 008, 009 and 011 seeded rows with `lower(hex(randomblob(16)))`: 32 hex
-- digits with no hyphens. Every row the engine writes itself stores a UUID in
-- its hyphenated form, and every read parses the stored id and hands out the
-- hyphenated form. A seeded row therefore came back under an id that matched
-- nothing in its own table: the seeded tax rates never matched their
-- jurisdictions (tax on a fresh store was zero for every address, with tax
-- enabled and rates seeded), and `get_plan` on a listed seeded plan was None.
--
-- Also links each seeded state to its seeded country (see below).
--
-- Rewrites every seeded id, and every column that references one, to
-- 8-4-4-4-12. Only values that are exactly 32 hex digits are touched, so an id
-- the engine wrote is never changed and the migration is idempotent. Foreign
-- keys are checked at commit, after parents and children agree again.

PRAGMA defer_foreign_keys = ON;

-- 009 inserted each country and its states in ONE multi-row INSERT whose
-- parent subquery ran before the country row existed, so every seeded state
-- has a NULL parent. Link them to their seeded country. Only seeded rows are
-- touched: they are the ones whose id is still bare hex at this point.
UPDATE tax_jurisdictions
SET parent_id = (
    SELECT country.id FROM tax_jurisdictions AS country
    WHERE country.level = 'country'
      AND country.country_code = tax_jurisdictions.country_code
      AND length(country.id) = 32
)
WHERE level = 'state'
  AND parent_id IS NULL
  AND length(id) = 32 AND id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE tax_jurisdictions
SET id = lower(substr(id, 1, 8) || '-' || substr(id, 9, 4) || '-' || substr(id, 13, 4) || '-'
        || substr(id, 17, 4) || '-' || substr(id, 21, 12))
WHERE length(id) = 32 AND id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE tax_jurisdictions
SET parent_id = lower(substr(parent_id, 1, 8) || '-' || substr(parent_id, 9, 4) || '-'
        || substr(parent_id, 13, 4) || '-' || substr(parent_id, 17, 4) || '-'
        || substr(parent_id, 21, 12))
WHERE parent_id IS NOT NULL AND length(parent_id) = 32 AND parent_id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE tax_rates
SET id = lower(substr(id, 1, 8) || '-' || substr(id, 9, 4) || '-' || substr(id, 13, 4) || '-'
        || substr(id, 17, 4) || '-' || substr(id, 21, 12))
WHERE length(id) = 32 AND id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE tax_rates
SET jurisdiction_id = lower(substr(jurisdiction_id, 1, 8) || '-' || substr(jurisdiction_id, 9, 4)
        || '-' || substr(jurisdiction_id, 13, 4) || '-' || substr(jurisdiction_id, 17, 4) || '-'
        || substr(jurisdiction_id, 21, 12))
WHERE length(jurisdiction_id) = 32 AND jurisdiction_id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE subscription_plans
SET id = lower(substr(id, 1, 8) || '-' || substr(id, 9, 4) || '-' || substr(id, 13, 4) || '-'
        || substr(id, 17, 4) || '-' || substr(id, 21, 12))
WHERE length(id) = 32 AND id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE subscription_plan_items
SET plan_id = lower(substr(plan_id, 1, 8) || '-' || substr(plan_id, 9, 4) || '-'
        || substr(plan_id, 13, 4) || '-' || substr(plan_id, 17, 4) || '-' || substr(plan_id, 21, 12))
WHERE length(plan_id) = 32 AND plan_id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE subscriptions
SET plan_id = lower(substr(plan_id, 1, 8) || '-' || substr(plan_id, 9, 4) || '-'
        || substr(plan_id, 13, 4) || '-' || substr(plan_id, 17, 4) || '-' || substr(plan_id, 21, 12))
WHERE length(plan_id) = 32 AND plan_id NOT GLOB '*[^0-9a-fA-F]*';

UPDATE exchange_rates
SET id = lower(substr(id, 1, 8) || '-' || substr(id, 9, 4) || '-' || substr(id, 13, 4) || '-'
        || substr(id, 17, 4) || '-' || substr(id, 21, 12))
WHERE length(id) = 32 AND id NOT GLOB '*[^0-9a-fA-F]*';
