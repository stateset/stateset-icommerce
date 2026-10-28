-- Preserve the approvable/payment state while a supplier bill is disputed.
ALTER TABLE ap_bills ADD COLUMN disputed_from_status TEXT;
