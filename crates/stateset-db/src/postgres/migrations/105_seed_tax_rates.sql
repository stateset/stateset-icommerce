-- Seed the tax jurisdictions and rates a fresh SQLite store has.
--
-- SQLite seeds US state sales tax, EU VAT and Canadian sales tax (009, fixed
-- by 098 and corrected by 099). Postgres seeded none, so a fresh Postgres
-- store charged zero tax on every sale. This seeds the state SQLite ENDS at
-- after 099, not the intermediate states it passed through:
--   - GST on the non-harmonized provinces and territories, not on the
--     country (HST already includes the federal part);
--   - HST in ON (13%), NB (15%), NL and PE (15%), NS 15% until 2025-03-31
--     and 14% from 2025-04-01;
--   - QST not compounded on GST; NT, NU and YT charge GST only.
-- Every state/province is linked to its country (098 had to repair that on
-- SQLite).
--
-- Only a store with NO tax rates at all is seeded: an operator who already
-- configured tax (under any jurisdiction codes) keeps exactly what they have,
-- rather than gaining seeded rates that would stack on theirs. Every insert is
-- also guarded with NOT EXISTS, so re-running is a no-op. Ids come from the
-- column defaults (UUID v4).
--
-- Rates SQLite dates `date('now')` at migration time are dated the UTC day
-- this migration runs, the same as SQLite. Rates 099 dated explicitly keep
-- those dates. The superseded Nova Scotia 15% rate is dated from 2010-07-01,
-- when that rate began (SQLite's copy starts on its migration day, after its
-- own end date, so it never applies there either).

DO $$
DECLARE
    seed_day DATE := (now() AT TIME ZONE 'UTC')::date;
BEGIN
    IF EXISTS (SELECT 1 FROM tax_rates) THEN
        RETURN;
    END IF;

    -- Countries.
    INSERT INTO tax_jurisdictions (parent_id, name, code, level, country_code, state_code)
    SELECT NULL, seed.name, seed.code, 'country', seed.code, NULL
    FROM (VALUES
        ('United States', 'US'),
        ('Germany', 'DE'),
        ('France', 'FR'),
        ('United Kingdom', 'GB'),
        ('Italy', 'IT'),
        ('Spain', 'ES'),
        ('Netherlands', 'NL'),
        ('Ireland', 'IE'),
        ('Sweden', 'SE'),
        ('Canada', 'CA')
    ) AS seed(name, code)
    WHERE NOT EXISTS (SELECT 1 FROM tax_jurisdictions j WHERE j.code = seed.code);

    -- States, provinces and territories, linked to their country.
    INSERT INTO tax_jurisdictions (parent_id, name, code, level, country_code, state_code)
    SELECT country.id, seed.name, seed.country_code || '-' || seed.state_code, 'state',
           seed.country_code, seed.state_code
    FROM (VALUES
        ('Alaska', 'US', 'AK'),
        ('Delaware', 'US', 'DE'),
        ('Montana', 'US', 'MT'),
        ('New Hampshire', 'US', 'NH'),
        ('Oregon', 'US', 'OR'),
        ('California', 'US', 'CA'),
        ('Texas', 'US', 'TX'),
        ('Florida', 'US', 'FL'),
        ('New York', 'US', 'NY'),
        ('Washington', 'US', 'WA'),
        ('Arizona', 'US', 'AZ'),
        ('Colorado', 'US', 'CO'),
        ('Illinois', 'US', 'IL'),
        ('Pennsylvania', 'US', 'PA'),
        ('Ohio', 'US', 'OH'),
        ('Ontario', 'CA', 'ON'),
        ('Quebec', 'CA', 'QC'),
        ('British Columbia', 'CA', 'BC'),
        ('Alberta', 'CA', 'AB'),
        ('Nova Scotia', 'CA', 'NS'),
        ('New Brunswick', 'CA', 'NB'),
        ('Manitoba', 'CA', 'MB'),
        ('Saskatchewan', 'CA', 'SK'),
        ('Newfoundland and Labrador', 'CA', 'NL'),
        ('Prince Edward Island', 'CA', 'PE'),
        ('Northwest Territories', 'CA', 'NT'),
        ('Nunavut', 'CA', 'NU'),
        ('Yukon', 'CA', 'YT')
    ) AS seed(name, country_code, state_code)
    JOIN tax_jurisdictions AS country
      ON country.code = seed.country_code AND country.level = 'country'
    WHERE NOT EXISTS (
        SELECT 1 FROM tax_jurisdictions j WHERE j.code = seed.country_code || '-' || seed.state_code
    );

    -- Rates. `name` NULL means "<jurisdiction name> <suffix>", as SQLite names
    -- them; `effective_from` NULL means the seed day.
    INSERT INTO tax_rates (
        jurisdiction_id, tax_type, product_category, rate, name, is_compound,
        effective_from, effective_to
    )
    SELECT j.id, seed.tax_type, seed.product_category, seed.rate,
           COALESCE(seed.name, j.name || ' ' || seed.suffix), FALSE,
           COALESCE(seed.effective_from, seed_day), seed.effective_to
    FROM (VALUES
        -- US state sales tax.
        ('US-CA', 'sales_tax', 'standard', 0.0725::numeric, NULL, 'State Tax', NULL::date, NULL::date),
        ('US-TX', 'sales_tax', 'standard', 0.0625, NULL, 'State Tax', NULL, NULL),
        ('US-FL', 'sales_tax', 'standard', 0.0600, NULL, 'State Tax', NULL, NULL),
        ('US-NY', 'sales_tax', 'standard', 0.0400, NULL, 'State Tax', NULL, NULL),
        ('US-WA', 'sales_tax', 'standard', 0.0650, NULL, 'State Tax', NULL, NULL),
        ('US-AZ', 'sales_tax', 'standard', 0.0560, NULL, 'State Tax', NULL, NULL),
        ('US-CO', 'sales_tax', 'standard', 0.0290, NULL, 'State Tax', NULL, NULL),
        ('US-IL', 'sales_tax', 'standard', 0.0625, NULL, 'State Tax', NULL, NULL),
        ('US-PA', 'sales_tax', 'standard', 0.0600, NULL, 'State Tax', NULL, NULL),
        ('US-OH', 'sales_tax', 'standard', 0.0575, NULL, 'State Tax', NULL, NULL),
        -- EU / UK VAT, standard and reduced.
        ('DE', 'vat', 'standard', 0.19, NULL, 'Standard VAT', NULL, NULL),
        ('FR', 'vat', 'standard', 0.20, NULL, 'Standard VAT', NULL, NULL),
        ('GB', 'vat', 'standard', 0.20, NULL, 'Standard VAT', NULL, NULL),
        ('IT', 'vat', 'standard', 0.22, NULL, 'Standard VAT', NULL, NULL),
        ('ES', 'vat', 'standard', 0.21, NULL, 'Standard VAT', NULL, NULL),
        ('NL', 'vat', 'standard', 0.21, NULL, 'Standard VAT', NULL, NULL),
        ('IE', 'vat', 'standard', 0.23, NULL, 'Standard VAT', NULL, NULL),
        ('SE', 'vat', 'standard', 0.25, NULL, 'Standard VAT', NULL, NULL),
        ('DE', 'vat', 'reduced', 0.07, NULL, 'Reduced VAT', NULL, NULL),
        ('FR', 'vat', 'reduced', 0.10, NULL, 'Reduced VAT', NULL, NULL),
        ('GB', 'vat', 'reduced', 0.05, NULL, 'Reduced VAT', NULL, NULL),
        ('IT', 'vat', 'reduced', 0.10, NULL, 'Reduced VAT', NULL, NULL),
        ('ES', 'vat', 'reduced', 0.10, NULL, 'Reduced VAT', NULL, NULL),
        ('NL', 'vat', 'reduced', 0.09, NULL, 'Reduced VAT', NULL, NULL),
        ('IE', 'vat', 'reduced', 0.135, NULL, 'Reduced VAT', NULL, NULL),
        ('SE', 'vat', 'reduced', 0.12, NULL, 'Reduced VAT', NULL, NULL),
        -- Canada: HST in the harmonized provinces (it includes GST).
        ('CA-ON', 'hst', 'standard', 0.13, NULL, 'HST', NULL, NULL),
        ('CA-NB', 'hst', 'standard', 0.15, NULL, 'HST', NULL, NULL),
        ('CA-NS', 'hst', 'standard', 0.15, NULL, 'HST', DATE '2010-07-01', DATE '2025-03-31'),
        ('CA-NS', 'hst', 'standard', 0.14, 'Nova Scotia HST', NULL, DATE '2025-04-01', NULL),
        ('CA-NL', 'hst', 'standard', 0.15, NULL, 'HST', DATE '2016-07-01', NULL),
        ('CA-PE', 'hst', 'standard', 0.15, NULL, 'HST', DATE '2016-07-01', NULL),
        -- Canada: federal GST everywhere that is not harmonized.
        ('CA-AB', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-BC', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-MB', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-SK', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-QC', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-NT', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-NU', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        ('CA-YT', 'gst', 'standard', 0.05, 'Federal GST', NULL, DATE '2008-01-01', NULL),
        -- Canada: provincial sales tax, on top of GST.
        ('CA-BC', 'pst', 'standard', 0.07, NULL, 'PST', NULL, NULL),
        ('CA-MB', 'pst', 'standard', 0.07, NULL, 'PST', NULL, NULL),
        ('CA-SK', 'pst', 'standard', 0.06, NULL, 'PST', NULL, NULL),
        -- Quebec QST: 9.975% of the pre-GST price, not compounded.
        ('CA-QC', 'qst', 'standard', 0.09975, 'Quebec QST', NULL, NULL, NULL)
    ) AS seed(code, tax_type, product_category, rate, name, suffix, effective_from, effective_to)
    JOIN tax_jurisdictions AS j ON j.code = seed.code
    WHERE NOT EXISTS (
        SELECT 1 FROM tax_rates r
        WHERE r.jurisdiction_id = j.id
          AND r.tax_type = seed.tax_type
          AND r.product_category = seed.product_category
          AND r.rate = seed.rate
    );
END
$$;
