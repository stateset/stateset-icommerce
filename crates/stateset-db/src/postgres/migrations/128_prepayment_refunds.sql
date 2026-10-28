ALTER TABLE prepayments ADD COLUMN refunded_amount NUMERIC NOT NULL DEFAULT 0;
ALTER TABLE prepayments ADD COLUMN refunded_at TIMESTAMPTZ;
UPDATE prepayments p SET refunded_amount=p.amount-COALESCE((SELECT SUM(a.amount) FROM prepayment_applications a WHERE a.prepayment_id=p.id AND NOT a.reversed),0),refunded_at=p.updated_at WHERE p.status='refunded';
