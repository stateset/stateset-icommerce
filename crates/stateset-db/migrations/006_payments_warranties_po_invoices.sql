-- Migration 006: Payments, Warranties, Purchase Orders, and Invoices
-- Adds comprehensive financial and warranty management tables

-- ====================
-- PAYMENTS
-- ====================

-- Payment methods stored for customers
CREATE TABLE IF NOT EXISTS payment_methods (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    method_type TEXT NOT NULL DEFAULT 'credit_card',
    is_default INTEGER NOT NULL DEFAULT 0,
    card_brand TEXT,
    card_last4 TEXT,
    card_exp_month INTEGER,
    card_exp_year INTEGER,
    cardholder_name TEXT,
    bank_name TEXT,
    account_last4 TEXT,
    external_id TEXT,
    billing_address TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_payment_methods_customer ON payment_methods(customer_id);
CREATE INDEX IF NOT EXISTS idx_payment_methods_external ON payment_methods(external_id);

-- Payments table
CREATE TABLE IF NOT EXISTS payments (
    id TEXT PRIMARY KEY,
    payment_number TEXT UNIQUE NOT NULL,
    order_id TEXT,
    invoice_id TEXT,
    customer_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    payment_method TEXT NOT NULL DEFAULT 'credit_card',
    amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    amount_refunded TEXT NOT NULL DEFAULT '0',
    external_id TEXT,
    processor TEXT,
    card_brand TEXT,
    card_last4 TEXT,
    card_exp_month INTEGER,
    card_exp_year INTEGER,
    billing_email TEXT,
    billing_name TEXT,
    billing_address TEXT,
    description TEXT,
    failure_reason TEXT,
    failure_code TEXT,
    metadata TEXT,
    paid_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_payments_number ON payments(payment_number);
CREATE INDEX IF NOT EXISTS idx_payments_order ON payments(order_id);
CREATE INDEX IF NOT EXISTS idx_payments_invoice ON payments(invoice_id);
CREATE INDEX IF NOT EXISTS idx_payments_customer ON payments(customer_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);
CREATE INDEX IF NOT EXISTS idx_payments_external ON payments(external_id);

-- Refunds table
CREATE TABLE IF NOT EXISTS refunds (
    id TEXT PRIMARY KEY,
    refund_number TEXT UNIQUE NOT NULL,
    payment_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    amount TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    reason TEXT,
    external_id TEXT,
    failure_reason TEXT,
    notes TEXT,
    refunded_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (payment_id) REFERENCES payments(id)
);

CREATE INDEX IF NOT EXISTS idx_refunds_number ON refunds(refund_number);
CREATE INDEX IF NOT EXISTS idx_refunds_payment ON refunds(payment_id);
CREATE INDEX IF NOT EXISTS idx_refunds_status ON refunds(status);

-- ====================
-- WARRANTIES
-- ====================

-- Warranties table
CREATE TABLE IF NOT EXISTS warranties (
    id TEXT PRIMARY KEY,
    warranty_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    order_item_id TEXT,
    product_id TEXT,
    sku TEXT,
    serial_number TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    warranty_type TEXT NOT NULL DEFAULT 'standard',
    provider TEXT,
    coverage_description TEXT,
    purchase_date TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT,
    duration_months INTEGER,
    max_coverage_amount TEXT,
    deductible TEXT,
    max_claims INTEGER,
    claims_used INTEGER NOT NULL DEFAULT 0,
    terms TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_warranties_number ON warranties(warranty_number);
CREATE INDEX IF NOT EXISTS idx_warranties_customer ON warranties(customer_id);
CREATE INDEX IF NOT EXISTS idx_warranties_order ON warranties(order_id);
CREATE INDEX IF NOT EXISTS idx_warranties_serial ON warranties(serial_number);
CREATE INDEX IF NOT EXISTS idx_warranties_status ON warranties(status);
CREATE INDEX IF NOT EXISTS idx_warranties_end_date ON warranties(end_date);

-- Warranty claims table
CREATE TABLE IF NOT EXISTS warranty_claims (
    id TEXT PRIMARY KEY,
    claim_number TEXT UNIQUE NOT NULL,
    warranty_id TEXT NOT NULL,
    customer_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'submitted',
    resolution TEXT NOT NULL DEFAULT 'none',
    issue_description TEXT NOT NULL,
    issue_category TEXT,
    issue_date TEXT,
    contact_phone TEXT,
    contact_email TEXT,
    shipping_address TEXT,
    repair_cost TEXT,
    replacement_product_id TEXT,
    refund_amount TEXT,
    denial_reason TEXT,
    internal_notes TEXT,
    customer_notes TEXT,
    submitted_at TEXT NOT NULL,
    approved_at TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (warranty_id) REFERENCES warranties(id)
);

CREATE INDEX IF NOT EXISTS idx_warranty_claims_number ON warranty_claims(claim_number);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_warranty ON warranty_claims(warranty_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_customer ON warranty_claims(customer_id);
CREATE INDEX IF NOT EXISTS idx_warranty_claims_status ON warranty_claims(status);

-- ====================
-- PURCHASE ORDERS
-- ====================

-- Suppliers table
CREATE TABLE IF NOT EXISTS suppliers (
    id TEXT PRIMARY KEY,
    supplier_code TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    contact_name TEXT,
    email TEXT,
    phone TEXT,
    website TEXT,
    address TEXT,
    city TEXT,
    state TEXT,
    postal_code TEXT,
    country TEXT,
    tax_id TEXT,
    payment_terms TEXT NOT NULL DEFAULT 'net_30',
    currency TEXT NOT NULL DEFAULT 'USD',
    lead_time_days INTEGER,
    minimum_order TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_suppliers_code ON suppliers(supplier_code);
CREATE INDEX IF NOT EXISTS idx_suppliers_name ON suppliers(name);
CREATE INDEX IF NOT EXISTS idx_suppliers_active ON suppliers(is_active);

-- Purchase orders table
CREATE TABLE IF NOT EXISTS purchase_orders (
    id TEXT PRIMARY KEY,
    po_number TEXT UNIQUE NOT NULL,
    supplier_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    order_date TEXT NOT NULL,
    expected_date TEXT,
    delivered_date TEXT,
    ship_to_address TEXT,
    ship_to_city TEXT,
    ship_to_state TEXT,
    ship_to_postal_code TEXT,
    ship_to_country TEXT,
    payment_terms TEXT NOT NULL DEFAULT 'net_30',
    currency TEXT NOT NULL DEFAULT 'USD',
    subtotal TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    shipping_cost TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    amount_paid TEXT NOT NULL DEFAULT '0',
    supplier_reference TEXT,
    notes TEXT,
    supplier_notes TEXT,
    approved_by TEXT,
    approved_at TEXT,
    sent_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id)
);

CREATE INDEX IF NOT EXISTS idx_purchase_orders_number ON purchase_orders(po_number);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_supplier ON purchase_orders(supplier_id);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_status ON purchase_orders(status);
CREATE INDEX IF NOT EXISTS idx_purchase_orders_date ON purchase_orders(order_date);

-- Purchase order items table
CREATE TABLE IF NOT EXISTS purchase_order_items (
    id TEXT PRIMARY KEY,
    purchase_order_id TEXT NOT NULL,
    product_id TEXT,
    sku TEXT NOT NULL,
    name TEXT NOT NULL,
    supplier_sku TEXT,
    quantity_ordered TEXT NOT NULL,
    quantity_received TEXT NOT NULL DEFAULT '0',
    unit_of_measure TEXT,
    unit_cost TEXT NOT NULL,
    line_total TEXT NOT NULL,
    tax_amount TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    expected_date TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (purchase_order_id) REFERENCES purchase_orders(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_purchase_order_items_po ON purchase_order_items(purchase_order_id);
CREATE INDEX IF NOT EXISTS idx_purchase_order_items_sku ON purchase_order_items(sku);

-- ====================
-- INVOICES
-- ====================

-- Invoices table
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    invoice_number TEXT UNIQUE NOT NULL,
    customer_id TEXT NOT NULL,
    order_id TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    invoice_type TEXT NOT NULL DEFAULT 'standard',
    invoice_date TEXT NOT NULL,
    due_date TEXT NOT NULL,
    payment_terms TEXT,
    currency TEXT NOT NULL DEFAULT 'USD',
    billing_name TEXT,
    billing_email TEXT,
    billing_address TEXT,
    billing_city TEXT,
    billing_state TEXT,
    billing_postal_code TEXT,
    billing_country TEXT,
    subtotal TEXT NOT NULL DEFAULT '0',
    discount_amount TEXT NOT NULL DEFAULT '0',
    discount_percent TEXT,
    tax_amount TEXT NOT NULL DEFAULT '0',
    tax_rate TEXT,
    shipping_amount TEXT NOT NULL DEFAULT '0',
    total TEXT NOT NULL DEFAULT '0',
    amount_paid TEXT NOT NULL DEFAULT '0',
    balance_due TEXT NOT NULL DEFAULT '0',
    po_number TEXT,
    notes TEXT,
    terms TEXT,
    footer TEXT,
    sent_at TEXT,
    viewed_at TEXT,
    paid_at TEXT,
    voided_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_invoices_number ON invoices(invoice_number);
CREATE INDEX IF NOT EXISTS idx_invoices_customer ON invoices(customer_id);
CREATE INDEX IF NOT EXISTS idx_invoices_order ON invoices(order_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_due_date ON invoices(due_date);
CREATE INDEX IF NOT EXISTS idx_invoices_date ON invoices(invoice_date);

-- Invoice items table
CREATE TABLE IF NOT EXISTS invoice_items (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL,
    order_item_id TEXT,
    product_id TEXT,
    sku TEXT,
    description TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_of_measure TEXT,
    unit_price TEXT NOT NULL,
    discount_amount TEXT NOT NULL DEFAULT '0',
    tax_amount TEXT NOT NULL DEFAULT '0',
    line_total TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice ON invoice_items(invoice_id);
CREATE INDEX IF NOT EXISTS idx_invoice_items_order_item ON invoice_items(order_item_id);
