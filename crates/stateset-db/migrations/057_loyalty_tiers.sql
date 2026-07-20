-- 057_loyalty_tiers: persist loyalty program tiers.
--
-- LoyaltyProgram.tiers is a modeled field, but neither backend stored it:
-- loyalty_programs had no tiers column, create_program ignored the input, and
-- row_to_program always returned an empty vec. A program created with tiers
-- silently lost them. Store the tier list as a JSON array in a TEXT column.

ALTER TABLE loyalty_programs ADD COLUMN tiers TEXT;
