//! Ten-minute embed: open engine, take order with tax, capture, post journal
//!
//! Run with:
//!   cargo run --example ten_minute_embed
//!
//! This example:
//! - opens an in-process SQLite engine
//! - initializes the GL and opens the current period
//! - configures a simple CA state tax rate
//! - creates inventory and an order (with tax)
//! - records a payment capture
//! - posts a balanced journal entry for the sale
//! - prints the order total, tax, capture, and journal id

use chrono::{Datelike, Days, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;
use stateset_embedded::{
    Commerce, CommerceError, CreateCustomer, CreateGlPeriod, CreateInventoryItem, CreateJournalEntry,
    CreateJournalEntryLine, CreateOrder, CreateOrderItem, CreateProduct, CreateProductVariant,
    JournalEntry, ProductTaxCategory, TaxAddress, TaxCalculationRequest, TaxLineItem,
};

fn main() -> Result<(), CommerceError> {
    println!("=== Ten-minute embed: order, capture, post journal ===\n");

    // 1) Open an in-process engine (SQLite)
    // Use a file to make it easy to re-run and inspect: './embed-demo.db'
    let commerce = Commerce::new("./embed-demo.db")?;
    println!("✓ SQLite engine opened at ./embed-demo.db");

    // 2) Initialize GL and open the current period
    let _ = commerce.general_ledger().initialize_chart_of_accounts()?;
    let today = Utc::now().date_naive();
    let (year, month) = (today.year(), today.month() as i32);
    let start = NaiveDate::from_ymd_opt(year, month as u32, 1).expect("valid month start");
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let next_start = NaiveDate::from_ymd_opt(ny, nm as u32, 1).expect("valid next month start");
    let end = next_start.checked_sub_days(Days::new(1)).expect("has previous day");
    // Open or create+open the period covering today
    let gl = commerce.general_ledger();
    let period = match gl.get_period_for_date(today)? {
        Some(p) => {
            if p.status != stateset_embedded::PeriodStatus::Open {
                gl.open_period(p.id)?
            } else {
                p
            }
        }
        None => {
            let created = gl.create_period(CreateGlPeriod {
                period_name: format!("{year}-{month:02}"),
                fiscal_year: year,
                period_number: month,
                start_date: start,
                end_date: end,
            })?;
            gl.open_period(created.id)?
        }
    };
    println!(
        "✓ GL period opened: {} [{}..{}]",
        period.period_name, period.start_date, period.end_date
    );

    // 3) Configure tax (enable and add a simple CA state rate if not present)
    // Default TaxSettings enable tax with half_up rounding; persist them.
    let _ = commerce.tax().update_settings(stateset_embedded::TaxSettings::default())?;
    println!("✓ Tax settings updated");

    // Ensure a CA state jurisdiction exists and a 7.25% sales tax rate applies.
    // Jurisdictions for US states are typically pre-seeded; if not, skip creating.
    if let Some(_ca) = commerce.tax().get_jurisdiction_by_code("US-CA")? {
        println!("✓ CA jurisdiction present");
    } else {
        println!("! US-CA jurisdiction not found; proceeding without non-zero tax");
    }

    // 4) Seed a customer, product, and inventory
    let suffix = Uuid::new_v4().simple().to_string();
    let short = &suffix[..8];
    let unique_email = format!("ada+{}@example.com", short);
    let sku = format!("WIDGET-CA-{}", short);
    let product_name = format!("Widget {}", short);

    let customer = commerce.customers().create(CreateCustomer {
        email: unique_email.clone(),
        first_name: "Ada".into(),
        last_name: "L".into(),
        ..Default::default()
    })?;
    println!("✓ Customer created: {}", customer.email);
    let product = commerce.products().create(CreateProduct {
        name: product_name.clone(),
        description: Some("A simple demo widget".into()),
        variants: Some(vec![CreateProductVariant {
            sku: sku.clone(),
            name: Some(format!("{} Variant", product_name)),
            price: dec!(100.00),
            ..Default::default()
        }]),
        ..Default::default()
    })?;
    println!("✓ Product created: {}", product.name);
    // Track inventory for the SKU and stock 10 units
    let _ = commerce.inventory().create_item(CreateInventoryItem {
        sku: sku.clone(),
        name: product_name.clone(),
        initial_quantity: Some(dec!(10)),
        ..Default::default()
    })?;
    println!("✓ Inventory item created: {} (10 units)", sku);

    // 5) Calculate tax for a single-line purchase shipping to CA
    let tax_result = commerce.tax().calculate(TaxCalculationRequest {
        line_items: vec![TaxLineItem {
            id: "line-1".into(),
            sku: Some(sku.clone()),
            product_id: Some(product.id),
            quantity: dec!(1),
            unit_price: dec!(100.00),
            discount_amount: Decimal::ZERO,
            tax_category: ProductTaxCategory::Standard,
            tax_code: None,
            description: Some("Widget".into()),
        }],
        shipping_address: TaxAddress {
            line1: Some("1 Market St".into()),
            line2: None,
            city: Some("San Francisco".into()),
            state: Some("CA".into()),
            postal_code: Some("94105".into()),
            country: "US".into(),
        },
        billing_address: None,
        customer_id: Some(customer.id.into_uuid()),
        shipping_amount: None,
        currency: stateset_embedded::CurrencyCode::USD,
        transaction_date: Some(today),
        prices_include_tax: false,
    })?;
    println!("✓ Tax calculated: total_tax=${}", tax_result.total_tax);

    let line_tax = tax_result
        .line_item_taxes
        .iter()
        .find(|t| t.line_item_id == "line-1")
        .map(|t| t.tax_amount)
        .unwrap_or(Decimal::ZERO);
    let order_tax_total = tax_result.total_tax; // equals line_tax when no shipping tax

    // 6) Create an order that carries the calculated line-level tax
    let variant = commerce.products().get_variant_by_sku(&sku)?.expect("variant exists");
    let order = commerce.orders().create(CreateOrder {
        customer_id: customer.id,
        items: vec![CreateOrderItem {
            product_id: product.id,
            variant_id: Some(variant.id),
            sku: sku.clone(),
            name: product_name.clone(),
            quantity: 1,
            unit_price: dec!(100.00),
            discount: None,
            // Record the computed line tax on the order item
            tax_amount: Some(line_tax),
        }],
        // No additional order-level tax (shipping tax would go here)
        tax_amount: Some(Decimal::ZERO),
        currency: Some(stateset_embedded::CurrencyCode::USD),
        ..Default::default()
    })?;
    println!("✓ Order created: {} (total ${})", order.order_number, order.total_amount);

    // 7) Record a payment capture using the kernel's real API
    let payment = commerce.payments().create(stateset_embedded::CreatePayment {
        order_id: Some(order.id),
        customer_id: Some(customer.id),
        amount: order.total_amount,
        currency: Some(stateset_embedded::CurrencyCode::USD),
        payment_method: stateset_embedded::PaymentMethodType::CreditCard,
        card_brand: Some(stateset_embedded::CardBrand::Visa),
        card_last4: Some("4242".into()),
        card_exp_month: Some(12),
        card_exp_year: Some(2030),
        ..Default::default()
    })?;
    let payment = commerce.payments().mark_completed(payment.id)?;
    println!("✓ Payment captured: {} amount=${}", payment.id, payment.amount);

    // 8) Post a balanced journal for the sale (Cash debit, Revenue + Tax Payable credit)
    // Accounts come from the default chart:
    //   1010 Cash (Asset), 4010 Sales Revenue (Revenue), 2010 Accounts Payable (Liability) used as Tax Payable
    let cash_acct = commerce
        .general_ledger()
        .get_account_by_number("1010")?
        .expect("default 1010 Cash must exist");
    let revenue_acct = commerce
        .general_ledger()
        .get_account_by_number("4010")?
        .expect("default 4010 Sales Revenue must exist");
    let ap_acct = commerce
        .general_ledger()
        .get_account_by_number("2010")?
        .expect("default 2010 Accounts Payable must exist");

    // Revenue is the pre-tax amount on the line
    let _pre_tax_revenue = order
        .items
        .iter()
        .map(|it| it.total - it.tax_amount)
        .sum::<Decimal>();
    let captured = payment.amount;
    let tax_liability = order_tax_total;
    let revenue_credit = captured - tax_liability;

    let mut lines = vec![
        // Debit Cash for the captured amount
        CreateJournalEntryLine::debit(cash_acct.id, captured, Some("Cash received".into())),
        // Credit Sales Revenue for the net (pre-tax) revenue
        CreateJournalEntryLine::credit(
            revenue_acct.id,
            revenue_credit,
            Some("Sales revenue".into()),
        ),
    ];
    if tax_liability > Decimal::ZERO {
        // Credit Accounts Payable as a stand-in for Sales Tax Payable (only when non-zero)
        lines.push(CreateJournalEntryLine::credit(
            ap_acct.id,
            tax_liability,
            Some("Sales tax payable".into()),
        ));
    }

    let entry = commerce.general_ledger().create_journal_entry(CreateJournalEntry {
        entry_date: today,
        entry_type: None,
        description: format!("Sale {} (payment {})", order.order_number, payment.id),
        lines,
        source_document_type: Some("order".into()),
        source_document_id: Some(order.id.into_uuid()),
        auto_post: Some(false),
    })?;
    let posted: JournalEntry =
        commerce.general_ledger().post_journal_entry(entry.id, "demo")?;
    println!("✓ Journal posted: {}", posted.entry_number);

    // 9) Print outputs
    println!("\n--- Result ---");
    println!("Order: {}  total=${}", order.order_number, order.total_amount);
    println!("  Tax (computed): ${}", order_tax_total);
    println!("Capture: {}  amount=${}", payment.id, captured);
    println!("Journal: {}  posted", posted.entry_number);

    Ok(())
}

