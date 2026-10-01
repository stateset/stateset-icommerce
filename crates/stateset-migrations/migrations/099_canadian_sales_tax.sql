-- Canadian sales tax, corrected and completed.
--
-- 009 seeded federal GST (5%) on the Canada country jurisdiction, which every
-- Canadian address matches, and HST on Ontario, Nova Scotia and New Brunswick.
-- HST *replaces* GST -- it includes the federal part -- so the two stacked:
-- an Ontario sale was taxed 5% + 13% = 18%. (Until 098 every seeded rate was
-- unreachable, so this never showed.) It also:
--   - compounded Quebec's QST on top of GST, which Quebec stopped doing in
--     2013: QST is 9.975% of the price before GST;
--   - kept Nova Scotia's HST at 15%; it is 14% from 2025-04-01;
--   - left out Newfoundland and Labrador and Prince Edward Island (15% HST)
--     and the three territories (GST only).
--
-- Only rows 009 seeded are changed, found by the jurisdiction code and the
-- seeded name. Every step is guarded so re-running is a no-op. New ids are
-- built per row in the hyphenated 8-4-4-4-12 form the engine reads (see 098).

-- 1. GST moves from the country to the provinces and territories that charge
--    it, so harmonized provinces charge HST alone.
UPDATE tax_rates
SET active = 0, updated_at = datetime('now')
WHERE tax_type = 'gst'
  AND name = 'Federal GST'
  AND active = 1
  AND jurisdiction_id IN (
      SELECT id FROM tax_jurisdictions WHERE code = 'CA' AND level = 'country'
  );

-- 2. The provinces and territories 009 did not seed.
INSERT INTO tax_jurisdictions (id, parent_id, name, code, level, country_code, state_code, postal_codes)
SELECT
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-'
        || hex(randomblob(2)) || '-' || hex(randomblob(6))),
    country.id, missing.name, missing.code, 'state', 'CA', missing.state_code, '[]'
FROM (
    SELECT 'Newfoundland and Labrador' AS name, 'CA-NL' AS code, 'NL' AS state_code
    UNION ALL SELECT 'Prince Edward Island', 'CA-PE', 'PE'
    UNION ALL SELECT 'Northwest Territories', 'CA-NT', 'NT'
    UNION ALL SELECT 'Nunavut', 'CA-NU', 'NU'
    UNION ALL SELECT 'Yukon', 'CA-YT', 'YT'
) AS missing
JOIN tax_jurisdictions AS country ON country.code = 'CA' AND country.level = 'country'
WHERE NOT EXISTS (SELECT 1 FROM tax_jurisdictions j WHERE j.code = missing.code);

-- 3. GST on every province and territory that is not harmonized.
INSERT INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-'
        || hex(randomblob(2)) || '-' || hex(randomblob(6))),
    j.id, 'gst', 'standard', '0.05', 'Federal GST', '2008-01-01'
FROM tax_jurisdictions AS j
WHERE j.code IN ('CA-AB', 'CA-BC', 'CA-MB', 'CA-SK', 'CA-QC', 'CA-NT', 'CA-NU', 'CA-YT')
  AND NOT EXISTS (
      SELECT 1 FROM tax_rates r
      WHERE r.jurisdiction_id = j.id AND r.tax_type = 'gst' AND r.active = 1
  );

-- 4. HST in Newfoundland and Labrador and Prince Edward Island (15%).
INSERT INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-'
        || hex(randomblob(2)) || '-' || hex(randomblob(6))),
    j.id, 'hst', 'standard', '0.15', j.name || ' HST', '2016-07-01'
FROM tax_jurisdictions AS j
WHERE j.code IN ('CA-NL', 'CA-PE')
  AND NOT EXISTS (
      SELECT 1 FROM tax_rates r
      WHERE r.jurisdiction_id = j.id AND r.tax_type = 'hst' AND r.active = 1
  );

-- 5. Nova Scotia: 15% until 2025-03-31, 14% from 2025-04-01.
UPDATE tax_rates
SET effective_to = '2025-03-31', updated_at = datetime('now')
WHERE tax_type = 'hst'
  AND rate = '0.15'
  AND effective_to IS NULL
  AND jurisdiction_id IN (SELECT id FROM tax_jurisdictions WHERE code = 'CA-NS');

INSERT INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, effective_from)
SELECT
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-'
        || hex(randomblob(2)) || '-' || hex(randomblob(6))),
    j.id, 'hst', 'standard', '0.14', 'Nova Scotia HST', '2025-04-01'
FROM tax_jurisdictions AS j
WHERE j.code = 'CA-NS'
  AND NOT EXISTS (
      SELECT 1 FROM tax_rates r
      WHERE r.jurisdiction_id = j.id AND r.tax_type = 'hst' AND r.rate = '0.14'
  );

-- 6. QST is charged on the price before GST, not compounded on it.
UPDATE tax_rates
SET is_compound = 0, updated_at = datetime('now')
WHERE tax_type = 'qst'
  AND name = 'Quebec QST'
  AND is_compound = 1
  AND jurisdiction_id IN (SELECT id FROM tax_jurisdictions WHERE code = 'CA-QC');
