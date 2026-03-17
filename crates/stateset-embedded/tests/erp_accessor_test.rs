//! Integration tests for ERP accessor modules that previously lacked dedicated test files.
//!
//! Covers: `accounts_payable`, `accounts_receivable`, `general_ledger`, `credit`,
//! `promotions`, and `warranties`.

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_embedded::{
    AccountSubType,
    AccountType,
    ApplyPromotionsRequest,
    // Accounts Receivable
    BillFilter,
    BillStatus,
    ClaimResolution,
    CollectionActivityFilter,
    CollectionActivityType,
    Commerce,
    CouponFilter,
    // Accounts Payable
    CreateBill,
    CreateBillItem,
    CreateCollectionActivity,
    CreateCreditAccount,
    CreateCreditMemo,
    CreateCouponCode,
    CreateCustomer,
    CreateGlAccount,
    CreateGlPeriod,
    CreateJournalEntry,
    CreateJournalEntryLine,
    CreatePromotion,
    CreateWarranty,
    CreateWarrantyClaim,
    CreateWriteOff,
    CreditAccountFilter,
    CreditMemoFilter,
    CreditMemoReason,
    CurrencyCode,
    GlAccountFilter,
    GlPeriodFilter,
    JournalEntryFilter,
    PromotionFilter,
    PromotionLineItem,
    PromotionTrigger,
    PromotionType,
    RiskRating,
    UpdateBill,
    UpdateCreditAccount,
    UpdatePromotion,
    UpdateWarranty,
    WarrantyFilter,
    WarrantyType,
    WriteOffFilter,
    WriteOffReason,
};
use uuid::Uuid;

// ============================================================================
// Helper functions
// ============================================================================

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn create_test_customer(commerce: &Commerce) -> stateset_embedded::CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}

// ============================================================================
// Accounts Payable Tests
// ============================================================================

#[test]
fn ap_create_bill_returns_draft_status() {
    let commerce = new_commerce();
    let supplier_id = Uuid::new_v4();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id,
            items: vec![CreateBillItem {
                description: "Raw materials".into(),
                quantity: dec!(50),
                unit_price: dec!(20.00),
                account_code: Some("5010".into()),
                ..Default::default()
            }],
            payment_terms: Some("Net 30".into()),
            ..Default::default()
        })
        .expect("Failed to create bill");

    assert_eq!(bill.status, BillStatus::Draft);
    assert_eq!(bill.total_amount, dec!(1000.00));
    assert_eq!(bill.supplier_id, supplier_id);
}

#[test]
fn ap_get_bill_by_id_round_trips() {
    let commerce = new_commerce();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: Uuid::new_v4(),
            items: vec![CreateBillItem {
                description: "Office supplies".into(),
                quantity: dec!(1),
                unit_price: dec!(75.50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill");

    let fetched = commerce
        .accounts_payable()
        .get_bill(bill.id)
        .expect("get_bill failed")
        .expect("Bill not found");

    assert_eq!(fetched.id, bill.id);
    assert_eq!(fetched.total_amount, dec!(75.50));
}

#[test]
fn ap_get_bill_by_number_round_trips() {
    let commerce = new_commerce();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: Uuid::new_v4(),
            items: vec![CreateBillItem {
                description: "Service fee".into(),
                quantity: dec!(1),
                unit_price: dec!(250.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill");

    let by_number = commerce
        .accounts_payable()
        .get_bill_by_number(&bill.bill_number)
        .expect("get_bill_by_number failed")
        .expect("Bill not found by number");

    assert_eq!(by_number.id, bill.id);
}

#[test]
fn ap_list_bills_includes_created_bill() {
    let commerce = new_commerce();
    let supplier_id = Uuid::new_v4();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id,
            items: vec![CreateBillItem {
                description: "Shipping fee".into(),
                quantity: dec!(1),
                unit_price: dec!(40.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill");

    let bills = commerce
        .accounts_payable()
        .list_bills(BillFilter { ..Default::default() })
        .expect("Failed to list bills");

    assert!(bills.iter().any(|b| b.id == bill.id));
}

#[test]
fn ap_list_bills_filtered_by_supplier() {
    let commerce = new_commerce();
    let supplier_a = Uuid::new_v4();
    let supplier_b = Uuid::new_v4();

    let bill_a = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier_a,
            items: vec![CreateBillItem {
                description: "Item A".into(),
                quantity: dec!(1),
                unit_price: dec!(100.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill A");

    // Bill for a different supplier
    commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier_b,
            items: vec![CreateBillItem {
                description: "Item B".into(),
                quantity: dec!(1),
                unit_price: dec!(200.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill B");

    let bills = commerce
        .accounts_payable()
        .list_bills(BillFilter { supplier_id: Some(supplier_a), ..Default::default() })
        .expect("Failed to list bills");

    assert!(bills.iter().all(|b| b.supplier_id == supplier_a));
    assert!(bills.iter().any(|b| b.id == bill_a.id));
}

#[test]
fn ap_update_bill_changes_payment_terms() {
    let commerce = new_commerce();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: Uuid::new_v4(),
            items: vec![CreateBillItem {
                description: "Consulting".into(),
                quantity: dec!(1),
                unit_price: dec!(500.00),
                ..Default::default()
            }],
            payment_terms: Some("Net 30".into()),
            ..Default::default()
        })
        .expect("Failed to create bill");

    let updated = commerce
        .accounts_payable()
        .update_bill(bill.id, UpdateBill {
            payment_terms: Some("Net 45".into()),
            ..Default::default()
        })
        .expect("Failed to update bill");

    assert_eq!(updated.payment_terms.as_deref(), Some("Net 45"));
}

#[test]
fn ap_approve_bill_changes_status() {
    let commerce = new_commerce();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: Uuid::new_v4(),
            items: vec![CreateBillItem {
                description: "Parts".into(),
                quantity: dec!(10),
                unit_price: dec!(30.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill");

    assert_eq!(bill.status, BillStatus::Draft);

    let approved = commerce
        .accounts_payable()
        .approve_bill(bill.id)
        .expect("Failed to approve bill");

    assert_eq!(approved.status, BillStatus::Approved);
}

#[test]
fn ap_aging_summary_is_zero_on_empty_db() {
    let commerce = new_commerce();

    let aging = commerce
        .accounts_payable()
        .get_aging_summary()
        .expect("Failed to get AP aging summary");

    assert_eq!(aging.total, dec!(0));
    assert_eq!(aging.current, dec!(0));
}

#[test]
fn ap_delete_draft_bill() {
    let commerce = new_commerce();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: Uuid::new_v4(),
            items: vec![CreateBillItem {
                description: "Delete me".into(),
                quantity: dec!(1),
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create bill");

    commerce
        .accounts_payable()
        .delete_bill(bill.id)
        .expect("Failed to delete bill");

    let result = commerce
        .accounts_payable()
        .get_bill(bill.id)
        .expect("get_bill after delete failed");

    assert!(result.is_none());
}

// ============================================================================
// Accounts Receivable Tests
// ============================================================================

#[test]
fn ar_aging_summary_starts_zero() {
    let commerce = new_commerce();

    let summary = commerce
        .accounts_receivable()
        .get_aging_summary()
        .expect("Failed to get AR aging summary");

    assert_eq!(summary.total, dec!(0));
    assert_eq!(summary.current, dec!(0));
    assert_eq!(summary.days_1_30, dec!(0));
    assert_eq!(summary.days_31_60, dec!(0));
    assert_eq!(summary.days_61_90, dec!(0));
    assert_eq!(summary.days_over_90, dec!(0));
}

#[test]
fn ar_get_total_outstanding_is_zero_initially() {
    let commerce = new_commerce();

    let total = commerce
        .accounts_receivable()
        .get_total_outstanding()
        .expect("Failed to get total outstanding");

    assert_eq!(total, dec!(0));
}

#[test]
fn ar_log_collection_activity_and_list() {
    use stateset_embedded::{CreateInvoice, CreateInvoiceItem};

    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    // Collection activity requires a real invoice to look up the customer ID
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Professional services".into(),
                quantity: dec!(1),
                unit_price: dec!(500.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create invoice");

    let invoice_id: Uuid = invoice.id.into();

    let activity = commerce
        .accounts_receivable()
        .log_collection_activity(CreateCollectionActivity {
            invoice_id,
            activity_type: CollectionActivityType::PhoneCall,
            notes: Some("Customer promised to pay by Friday".into()),
            contact_method: Some("Phone".into()),
            contact_result: Some("Promise to pay".into()),
            performed_by: Some("Alice Collector".into()),
            ..Default::default()
        })
        .expect("Failed to log collection activity");

    assert_eq!(activity.invoice_id, invoice_id);

    let activities = commerce
        .accounts_receivable()
        .list_collection_activities(CollectionActivityFilter {
            invoice_id: Some(invoice_id),
            ..Default::default()
        })
        .expect("Failed to list collection activities");

    assert!(activities.iter().any(|a| a.id == activity.id));
}

#[test]
fn ar_create_credit_memo_and_get() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let memo = commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_id.into(),
            amount: dec!(150.00),
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: Some("Compensation for service outage".into()),
        })
        .expect("Failed to create credit memo");

    assert_eq!(memo.amount, dec!(150.00));

    let fetched = commerce
        .accounts_receivable()
        .get_credit_memo(memo.id)
        .expect("get_credit_memo failed")
        .expect("Credit memo not found");

    assert_eq!(fetched.id, memo.id);
    assert_eq!(fetched.customer_id, Uuid::from(customer_id));
}

#[test]
fn ar_list_credit_memos_filtered_by_customer() {
    let commerce = new_commerce();
    let customer_a = create_test_customer(&commerce);
    let customer_b = create_test_customer(&commerce);

    let memo_a = commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_a.into(),
            amount: dec!(50.00),
            reason: CreditMemoReason::ReturnedGoods,
            original_invoice_id: None,
            notes: None,
        })
        .expect("Failed to create credit memo A");

    // Credit memo for another customer
    commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_b.into(),
            amount: dec!(75.00),
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: None,
        })
        .expect("Failed to create credit memo B");

    let memos = commerce
        .accounts_receivable()
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer_a.into()),
            ..Default::default()
        })
        .expect("Failed to list credit memos");

    assert!(memos.iter().all(|m| m.customer_id == Uuid::from(customer_a)));
    assert!(memos.iter().any(|m| m.id == memo_a.id));
}

#[test]
fn ar_create_write_off_and_list() {
    use stateset_embedded::{CreateInvoice, CreateInvoiceItem};

    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    // Write-off requires a real invoice (used to look up customer ID in DB)
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Uncollected service fee".into(),
                quantity: dec!(1),
                unit_price: dec!(300.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create invoice for write-off");

    let write_off = commerce
        .accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id: invoice.id.into(),
            amount: dec!(300.00),
            reason: WriteOffReason::Uncollectible,
            notes: Some("Customer bankrupt".into()),
            approved_by: Some("Finance Manager".into()),
        })
        .expect("Failed to create write-off");

    assert_eq!(write_off.amount, dec!(300.00));

    let write_offs = commerce
        .accounts_receivable()
        .list_write_offs(WriteOffFilter { ..Default::default() })
        .expect("Failed to list write-offs");

    assert!(write_offs.iter().any(|w| w.id == write_off.id));
}

#[test]
fn ar_get_customer_summary_is_none_for_unknown_customer() {
    let commerce = new_commerce();

    let summary = commerce
        .accounts_receivable()
        .get_customer_summary(Uuid::new_v4())
        .expect("get_customer_summary failed");

    assert!(summary.is_none());
}

// ============================================================================
// General Ledger Tests
// ============================================================================

#[test]
fn gl_create_account_and_get_by_id() {
    let commerce = new_commerce();

    let account = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1010".into(),
            name: "Checking Account".into(),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::Cash),
            description: Some("Primary operating checking account".into()),
            is_posting: Some(true),
            currency: Some(CurrencyCode::USD),
            parent_account_id: None,
            is_header: Some(false),
        })
        .expect("Failed to create GL account");

    assert_eq!(account.account_number, "1010");
    assert_eq!(account.name, "Checking Account");
    assert_eq!(account.account_type, AccountType::Asset);

    let fetched = commerce
        .general_ledger()
        .get_account(account.id)
        .expect("get_account failed")
        .expect("Account not found");

    assert_eq!(fetched.id, account.id);
    assert_eq!(fetched.account_number, "1010");
}

#[test]
fn gl_get_account_by_number() {
    let commerce = new_commerce();

    let account = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "4000".into(),
            name: "Sales Revenue".into(),
            account_type: AccountType::Revenue,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create GL account");

    let by_number = commerce
        .general_ledger()
        .get_account_by_number("4000")
        .expect("get_account_by_number failed")
        .expect("Account not found by number");

    assert_eq!(by_number.id, account.id);
}

#[test]
fn gl_list_accounts_includes_created_accounts() {
    let commerce = new_commerce();

    let account1 = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1100".into(),
            name: "Accounts Receivable".into(),
            account_type: AccountType::Asset,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create account 1");

    let account2 = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "2000".into(),
            name: "Accounts Payable".into(),
            account_type: AccountType::Liability,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create account 2");

    let accounts = commerce
        .general_ledger()
        .list_accounts(GlAccountFilter { ..Default::default() })
        .expect("Failed to list accounts");

    let ids: Vec<_> = accounts.iter().map(|a| a.id).collect();
    assert!(ids.contains(&account1.id));
    assert!(ids.contains(&account2.id));
}

#[test]
fn gl_list_accounts_filtered_by_type() {
    let commerce = new_commerce();

    commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1200".into(),
            name: "Inventory".into(),
            account_type: AccountType::Asset,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create asset account");

    commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "5000".into(),
            name: "Cost of Goods Sold".into(),
            account_type: AccountType::Expense,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create expense account");

    let assets = commerce
        .general_ledger()
        .list_accounts(GlAccountFilter {
            account_type: Some(AccountType::Asset),
            ..Default::default()
        })
        .expect("Failed to list asset accounts");

    assert!(assets.iter().all(|a| a.account_type == AccountType::Asset));
}

#[test]
fn gl_get_account_hierarchy_returns_accounts() {
    let commerce = new_commerce();

    // Create a parent account
    let parent = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1000".into(),
            name: "Current Assets".into(),
            account_type: AccountType::Asset,
            description: Some("Current assets header".into()),
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: Some(CurrencyCode::USD),
        })
        .expect("Failed to create parent account");

    // Create a child account
    let child = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1010".into(),
            name: "Cash in Bank".into(),
            account_type: AccountType::Asset,
            description: None,
            account_sub_type: Some(AccountSubType::Cash),
            parent_account_id: Some(parent.id),
            is_header: Some(false),
            is_posting: Some(true),
            currency: Some(CurrencyCode::USD),
        })
        .expect("Failed to create child account");

    let hierarchy = commerce
        .general_ledger()
        .get_account_hierarchy()
        .expect("Failed to get account hierarchy");

    let ids: Vec<_> = hierarchy.iter().map(|a| a.id).collect();
    assert!(ids.contains(&parent.id));
    assert!(ids.contains(&child.id));
}

#[test]
fn gl_create_accounting_period() {
    let commerce = new_commerce();

    let period = commerce
        .general_ledger()
        .create_period(CreateGlPeriod {
            period_name: "January 2025".into(),
            fiscal_year: 2025,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
        })
        .expect("Failed to create GL period");

    assert_eq!(period.period_name, "January 2025");
    assert_eq!(period.fiscal_year, 2025);
    assert_eq!(period.period_number, 1);
}

#[test]
fn gl_list_periods_filtered_by_year() {
    let commerce = new_commerce();

    let p1 = commerce
        .general_ledger()
        .create_period(CreateGlPeriod {
            period_name: "Q1 2025".into(),
            fiscal_year: 2025,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2025, 3, 31).unwrap(),
        })
        .expect("Failed to create period 1");

    commerce
        .general_ledger()
        .create_period(CreateGlPeriod {
            period_name: "Q1 2026".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        })
        .expect("Failed to create period 2");

    let periods_2025 = commerce
        .general_ledger()
        .list_periods(GlPeriodFilter { fiscal_year: Some(2025), ..Default::default() })
        .expect("Failed to list periods");

    assert!(periods_2025.iter().all(|p| p.fiscal_year == 2025));
    assert!(periods_2025.iter().any(|p| p.id == p1.id));
}

#[test]
fn gl_trial_balance_is_zero_with_no_entries() {
    let commerce = new_commerce();

    // Initialize a standard chart of accounts
    commerce
        .general_ledger()
        .initialize_chart_of_accounts()
        .expect("Failed to initialize chart of accounts");

    // Trial balance on a future date — all zero balances expected
    let trial_balance = commerce
        .general_ledger()
        .get_trial_balance(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap())
        .expect("Failed to get trial balance");

    assert!(
        trial_balance.is_balanced,
        "Trial balance should be balanced when no transactions exist"
    );
    assert_eq!(trial_balance.total_debits, trial_balance.total_credits);
}

#[test]
fn gl_list_journal_entries_is_empty_initially() {
    let commerce = new_commerce();

    let entries = commerce
        .general_ledger()
        .list_journal_entries(JournalEntryFilter { ..Default::default() })
        .expect("Failed to list journal entries on empty DB");

    assert!(entries.is_empty(), "Expected no journal entries in a fresh DB");
}

#[test]
fn gl_create_journal_entry_requires_open_period() {
    let commerce = new_commerce();

    // Create accounts to use in the journal entry
    let cash = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "1001".into(),
            name: "Petty Cash".into(),
            account_type: AccountType::Asset,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create cash account");

    let expense = commerce
        .general_ledger()
        .create_account(CreateGlAccount {
            account_number: "6100".into(),
            name: "Office Supplies Expense".into(),
            account_type: AccountType::Expense,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("Failed to create expense account");

    // Attempt to create a journal entry without any open period — must fail
    let result = commerce.general_ledger().create_journal_entry(CreateJournalEntry {
        entry_date: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
        description: "Office supply purchase".into(),
        lines: vec![
            CreateJournalEntryLine::debit(expense.id, dec!(80.00), Some("Paper".into())),
            CreateJournalEntryLine::credit(cash.id, dec!(80.00), Some("Cash".into())),
        ],
        entry_type: None,
        source_document_type: None,
        source_document_id: None,
        auto_post: None,
    });

    assert!(
        result.is_err(),
        "Expected journal entry creation to fail without an open period"
    );
}

#[test]
fn gl_initialize_chart_of_accounts_creates_multiple_accounts() {
    let commerce = new_commerce();

    let accounts = commerce
        .general_ledger()
        .initialize_chart_of_accounts()
        .expect("Failed to initialize chart of accounts");

    // A standard chart of accounts should include several accounts
    assert!(!accounts.is_empty(), "Expected at least one standard account");

    // Check that various account types are present
    let has_asset = accounts.iter().any(|a| a.account_type == AccountType::Asset);
    let has_liability = accounts.iter().any(|a| a.account_type == AccountType::Liability);
    let has_revenue = accounts.iter().any(|a| a.account_type == AccountType::Revenue);
    let has_expense = accounts.iter().any(|a| a.account_type == AccountType::Expense);

    assert!(has_asset, "Expected at least one Asset account");
    assert!(has_liability, "Expected at least one Liability account");
    assert!(has_revenue, "Expected at least one Revenue account");
    assert!(has_expense, "Expected at least one Expense account");
}

// ============================================================================
// Credit Tests
// ============================================================================

#[test]
fn credit_create_account_with_limit() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let account = commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(10000.00),
            payment_terms: Some("Net 30".into()),
            risk_rating: Some(RiskRating::Low),
            notes: Some("Premium customer".into()),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    assert_eq!(account.customer_id, customer_id);
    assert_eq!(account.credit_limit, dec!(10000.00));
}

#[test]
fn credit_get_account_by_id_round_trips() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let account = commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(5000.00),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let fetched = commerce
        .credit()
        .get_credit_account(account.id)
        .expect("get_credit_account failed")
        .expect("Account not found");

    assert_eq!(fetched.id, account.id);
    assert_eq!(fetched.credit_limit, dec!(5000.00));
}

#[test]
fn credit_get_account_by_customer_id() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let account = commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(2500.00),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let by_customer = commerce
        .credit()
        .get_credit_account_by_customer(customer_id)
        .expect("get_credit_account_by_customer failed")
        .expect("Account not found by customer ID");

    assert_eq!(by_customer.id, account.id);
}

#[test]
fn credit_list_accounts_includes_created_account() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let account = commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(7500.00),
            risk_rating: Some(RiskRating::Medium),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let accounts = commerce
        .credit()
        .list_credit_accounts(CreditAccountFilter { ..Default::default() })
        .expect("Failed to list credit accounts");

    assert!(accounts.iter().any(|a| a.id == account.id));
}

#[test]
fn credit_adjust_limit_reflects_new_value() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(3000.00),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let adjusted = commerce
        .credit()
        .adjust_credit_limit(customer_id, dec!(8000.00), "Annual review - good payment history")
        .expect("Failed to adjust credit limit");

    assert_eq!(adjusted.credit_limit, dec!(8000.00));
}

#[test]
fn credit_check_credit_approved_when_under_limit() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(10000.00),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let result = commerce
        .credit()
        .check_credit(customer_id, dec!(500.00))
        .expect("check_credit failed");

    assert!(result.approved, "Expected credit to be approved for amount well under limit");
}

#[test]
fn credit_update_account_changes_payment_terms() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let account = commerce
        .credit()
        .create_credit_account(CreateCreditAccount {
            customer_id,
            credit_limit: dec!(15000.00),
            payment_terms: Some("Net 30".into()),
            ..Default::default()
        })
        .expect("Failed to create credit account");

    let updated = commerce
        .credit()
        .update_credit_account(account.id, UpdateCreditAccount {
            payment_terms: Some("Net 60".into()),
            ..Default::default()
        })
        .expect("Failed to update credit account");

    assert_eq!(updated.payment_terms.as_deref(), Some("Net 60"));
}

// ============================================================================
// Promotions Tests
// ============================================================================

#[test]
fn promotions_create_percentage_off() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Summer Sale 20%".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.20)),
            description: Some("20% off sitewide".into()),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    assert_eq!(promo.name, "Summer Sale 20%");
    assert_eq!(promo.promotion_type, PromotionType::PercentageOff);
    assert_eq!(promo.percentage_off, Some(dec!(0.20)));
}

#[test]
fn promotions_create_fixed_amount_off() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "$15 Off Orders Over $100".into(),
            promotion_type: PromotionType::FixedAmountOff,
            fixed_amount_off: Some(dec!(15.00)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    assert_eq!(promo.promotion_type, PromotionType::FixedAmountOff);
    assert_eq!(promo.fixed_amount_off, Some(dec!(15.00)));
}

#[test]
fn promotions_get_by_id_round_trips() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Flash Sale".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    let fetched = commerce
        .promotions()
        .get(promo.id)
        .expect("get promotion failed")
        .expect("Promotion not found");

    assert_eq!(fetched.id, promo.id);
    assert_eq!(fetched.name, "Flash Sale");
}

#[test]
fn promotions_list_includes_created_promotion() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Black Friday Deal".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.30)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    let promos = commerce
        .promotions()
        .list(PromotionFilter { ..Default::default() })
        .expect("Failed to list promotions");

    assert!(promos.iter().any(|p| p.id == promo.id));
}

#[test]
fn promotions_activate_and_deactivate() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Weekend Special".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.15)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    let activated = commerce
        .promotions()
        .activate(promo.id)
        .expect("Failed to activate promotion");

    assert!(activated.is_active(), "Expected promotion to be active after activation");

    let deactivated = commerce
        .promotions()
        .deactivate(promo.id)
        .expect("Failed to deactivate promotion");

    assert!(!deactivated.is_active(), "Expected promotion to be inactive after deactivation");
}

#[test]
fn promotions_update_name_and_description() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Old Name".into(),
            promotion_type: PromotionType::FixedAmountOff,
            fixed_amount_off: Some(dec!(5.00)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    let updated = commerce
        .promotions()
        .update(promo.id, UpdatePromotion {
            name: Some("New Name".into()),
            description: Some("Updated description".into()),
            ..Default::default()
        })
        .expect("Failed to update promotion");

    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("Updated description"));
}

#[test]
fn promotions_create_coupon_and_validate() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Coupon Promo".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.25)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    commerce.promotions().activate(promo.id).expect("Failed to activate promotion");

    let coupon = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "SAVE25".into(),
            usage_limit: Some(50),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");

    assert_eq!(coupon.code, "SAVE25");
    assert_eq!(coupon.usage_limit, Some(50));

    let validated = commerce
        .promotions()
        .validate_coupon("SAVE25")
        .expect("validate_coupon failed");

    assert!(validated.is_some(), "Expected coupon to be valid");
}

#[test]
fn promotions_list_coupons_for_promotion() {
    let commerce = new_commerce();

    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Multi-Code Promo".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.05)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    let coupon1 = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "CODE1".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon 1");

    let coupon2 = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "CODE2".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon 2");

    let coupons = commerce
        .promotions()
        .list_coupons(CouponFilter { promotion_id: Some(promo.id), ..Default::default() })
        .expect("Failed to list coupons");

    let coupon_ids: Vec<_> = coupons.iter().map(|c| c.id).collect();
    assert!(coupon_ids.contains(&coupon1.id));
    assert!(coupon_ids.contains(&coupon2.id));
}

#[test]
fn promotions_apply_percentage_discount() {
    let commerce = new_commerce();

    // Use CouponCode trigger so the promo is only applied via coupon (not also as auto-promo)
    let promo = commerce
        .promotions()
        .create(CreatePromotion {
            name: "10% Sitewide Coupon".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.10)),
            trigger: PromotionTrigger::CouponCode,
            ..Default::default()
        })
        .expect("Failed to create promotion");

    commerce.promotions().activate(promo.id).expect("Failed to activate promotion");

    let _coupon = commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            promotion_id: promo.id,
            code: "TENOFF".into(),
            usage_limit: None,
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");

    let result = commerce
        .promotions()
        .apply(ApplyPromotionsRequest {
            subtotal: dec!(200.00),
            coupon_codes: vec!["TENOFF".into()],
            line_items: vec![PromotionLineItem {
                id: "item-1".into(),
                quantity: 2,
                unit_price: dec!(100.00),
                line_total: dec!(200.00),
                product_id: None,
                variant_id: None,
                sku: None,
                category_ids: vec![],
            }],
            ..Default::default()
        })
        .expect("Failed to apply promotions");

    assert!(result.total_discount > dec!(0), "Expected a discount to be applied");
    assert_eq!(result.applied_promotions.len(), 1);
}

#[test]
fn promotions_get_active_returns_only_active() {
    let commerce = new_commerce();

    let promo_a = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Active Promo".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.05)),
            ..Default::default()
        })
        .expect("Failed to create active promo");

    commerce.promotions().activate(promo_a.id).expect("Failed to activate promo A");

    let _promo_b = commerce
        .promotions()
        .create(CreatePromotion {
            name: "Inactive Promo".into(),
            promotion_type: PromotionType::FixedAmountOff,
            fixed_amount_off: Some(dec!(10.00)),
            ..Default::default()
        })
        .expect("Failed to create inactive promo");

    let active = commerce.promotions().get_active().expect("Failed to get active promotions");

    assert!(active.iter().all(|p| p.is_active()));
    assert!(active.iter().any(|p| p.id == promo_a.id));
}

// ============================================================================
// Warranties Tests
// ============================================================================

#[test]
fn warranties_create_standard_warranty() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            coverage_description: Some("Covers manufacturing defects".into()),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    assert_eq!(warranty.customer_id, customer_id);
    assert_eq!(warranty.warranty_type, WarrantyType::Standard);
    assert_eq!(warranty.duration_months, Some(12));
}

#[test]
fn warranties_create_extended_warranty() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Extended),
            duration_months: Some(36),
            ..Default::default()
        })
        .expect("Failed to create extended warranty");

    assert_eq!(warranty.warranty_type, WarrantyType::Extended);
    assert_eq!(warranty.duration_months, Some(36));
}

#[test]
fn warranties_get_by_id_round_trips() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(24),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let fetched = commerce
        .warranties()
        .get(warranty.id.into())
        .expect("get warranty failed")
        .expect("Warranty not found");

    assert_eq!(fetched.id, warranty.id);
    assert_eq!(fetched.customer_id, customer_id);
}

#[test]
fn warranties_get_by_number_round_trips() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let by_number = commerce
        .warranties()
        .get_by_number(&warranty.warranty_number)
        .expect("get_by_number failed")
        .expect("Warranty not found by number");

    assert_eq!(by_number.id, warranty.id);
}

#[test]
fn warranties_list_includes_created_warranty() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(6),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let warranties = commerce
        .warranties()
        .list(WarrantyFilter { ..Default::default() })
        .expect("Failed to list warranties");

    assert!(warranties.iter().any(|w| w.id == warranty.id));
}

#[test]
fn warranties_for_customer_returns_their_warranties() {
    let commerce = new_commerce();
    let customer_a = create_test_customer(&commerce);
    let customer_b = create_test_customer(&commerce);

    // Create a warranty for customer A only; avoid creating two warranties in rapid
    // succession to sidestep the millisecond-precision warranty_number UNIQUE constraint.
    let warranty_a = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id: customer_a,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty for customer A");

    let warranties_for_a = commerce
        .warranties()
        .for_customer(customer_a.into())
        .expect("Failed to get warranties for customer A");

    // customer_a should have exactly the warranty we created
    assert!(warranties_for_a.iter().all(|w| w.customer_id == customer_a));
    assert!(warranties_for_a.iter().any(|w| w.id == warranty_a.id));

    // customer_b has no warranties
    let warranties_for_b = commerce
        .warranties()
        .for_customer(customer_b.into())
        .expect("Failed to get warranties for customer B");

    assert!(warranties_for_b.is_empty(), "customer_b should have no warranties");
}

#[test]
fn warranties_create_claim_and_get() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(24),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let claim = commerce
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Device stopped charging after 6 months".into(),
            contact_email: Some("customer@example.com".into()),
            ..Default::default()
        })
        .expect("Failed to create warranty claim");

    assert_eq!(claim.warranty_id, warranty.id);
    assert_eq!(claim.issue_description, "Device stopped charging after 6 months");

    let fetched = commerce
        .warranties()
        .get_claim(claim.id)
        .expect("get_claim failed")
        .expect("Claim not found");

    assert_eq!(fetched.id, claim.id);
}

#[test]
fn warranties_get_claims_for_warranty() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Extended),
            duration_months: Some(36),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    // Create a single claim; creating two in rapid succession risks a UNIQUE
    // constraint violation on claim_number (millisecond-precision generator).
    let claim = commerce
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Screen cracked".into(),
            ..Default::default()
        })
        .expect("Failed to create claim");

    let claims = commerce
        .warranties()
        .get_claims(warranty.id.into())
        .expect("Failed to get claims");

    assert!(!claims.is_empty(), "Expected at least one claim for the warranty");
    assert!(claims.iter().any(|c| c.id == claim.id));

    // A random UUID should have no claims
    let unknown_warranty_id = Uuid::new_v4();
    let no_claims = commerce
        .warranties()
        .get_claims(unknown_warranty_id)
        .expect("Failed to get claims for unknown warranty");

    assert!(no_claims.is_empty(), "Unknown warranty should have no claims");
}

#[test]
fn warranties_approve_claim_changes_status() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let claim = commerce
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Faulty power button".into(),
            ..Default::default()
        })
        .expect("Failed to create claim");

    let approved = commerce
        .warranties()
        .approve_claim(claim.id)
        .expect("Failed to approve claim");

    assert_eq!(approved.id, claim.id);
    // Approved status should differ from the initial open status
    assert_ne!(
        format!("{:?}", approved.status),
        "Open",
        "Expected claim status to change after approval"
    );
}

#[test]
fn warranties_complete_claim_with_replacement() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let claim = commerce
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Unit stopped working entirely".into(),
            ..Default::default()
        })
        .expect("Failed to create claim");

    commerce.warranties().approve_claim(claim.id).expect("Failed to approve claim");

    let completed = commerce
        .warranties()
        .complete_claim(claim.id, ClaimResolution::Replacement)
        .expect("Failed to complete claim with replacement");

    assert_eq!(completed.id, claim.id);
    assert_eq!(completed.resolution, ClaimResolution::Replacement);
}

#[test]
fn warranties_update_preserves_customer_id() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id,
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    let updated = commerce
        .warranties()
        .update(
            warranty.id.into(),
            UpdateWarranty {
                coverage_description: Some("Updated coverage terms".into()),
                ..Default::default()
            },
        )
        .expect("Failed to update warranty");

    assert_eq!(updated.customer_id, customer_id);
    assert_eq!(updated.coverage_description.as_deref(), Some("Updated coverage terms"));
}
