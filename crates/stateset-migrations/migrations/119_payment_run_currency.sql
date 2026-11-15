-- A run has one denomination. Legacy mixed runs require explicit reconciliation.
ALTER TABLE ap_payment_runs ADD COLUMN currency TEXT;
UPDATE ap_payment_runs SET currency=(SELECT MIN(b.currency) FROM ap_payment_run_bills rb JOIN ap_bills b ON b.id=rb.bill_id WHERE rb.run_id=ap_payment_runs.id)
WHERE (SELECT COUNT(DISTINCT b.currency) FROM ap_payment_run_bills rb JOIN ap_bills b ON b.id=rb.bill_id WHERE rb.run_id=ap_payment_runs.id)=1;
