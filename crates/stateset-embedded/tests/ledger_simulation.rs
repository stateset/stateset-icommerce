//! Randomized ledger burn-in simulation against the SQLite backend.
//!
//! Drives ~200 random finance operations — invoices with GL auto-posting,
//! manual journal entries, depreciation posting, revenue recognition, FX
//! revaluation, and period close — and asserts after EVERY operation that the
//! trial balance balances (total debits == total credits). After the final
//! period close the balance sheet must satisfy the accounting equation.
//!
//! Reproducibility: the operation stream is driven by a seeded deterministic
//! PRNG. Override the seed with the `LEDGER_SIM_SEED` env var (u64) to
//! explore other trajectories; the default is fixed so CI runs are stable.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AccountSubType, AccountType, CreateAutoPostingConfig, CreateCustomer, CreateFixedAsset,
    CreateGlAccount, CreateGlPeriod, CreateInvoice, CreateInvoiceItem, CreateJournalEntry,
    CreateJournalEntryLine, CreatePerformanceObligation, CreateRevenueContract, Currency,
    DepreciationMethod, FixedAssetCategory, GlAccountFilter, JournalEntryType, RecognitionMethod,
    RevenueContractStatus, SetExchangeRate, UpdateRevenueContract,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

const DEFAULT_SEED: u64 = 0x5EED_1CE5_2026_0720;
const OPERATIONS: usize = 200;

/// Deterministic splitmix64-style PRNG — no external dependency, fully
/// reproducible from the seed.
struct Rng(u64);

impl Rng {
    const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, bound)`.
    const fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    /// Uniform value in `[lo, hi]` (inclusive).
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo).expect("valid range") + 1;
        lo + i64::try_from(self.below(span)).expect("fits")
    }

    /// Random money amount with 2 decimal places in `[lo_cents, hi_cents]`.
    fn money(&mut self, lo_cents: i64, hi_cents: i64) -> Decimal {
        Decimal::new(self.range(lo_cents, hi_cents), 2)
    }
}

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

/// Entry date used for manual postings; inside the wide open period.
const POSTING_DATE: NaiveDate = date(2026, 6, 15);
/// Reporting horizon covering every entry the simulation can produce.
const AS_OF: NaiveDate = date(2030, 12, 31);

struct Sim {
    commerce: Commerce,
    rng: Rng,
    period_id: Uuid,
    customer_id: stateset_core::CustomerId,
    /// Posting accounts eligible for random manual journal entries
    /// (balance-sheet accounts only, excluding the EUR cash account so the
    /// FX foreign balance stays controlled).
    je_accounts: Vec<Uuid>,
    /// Current fixed asset and how many depreciation periods remain.
    asset: (Uuid, u32),
    /// Current revenue obligation and the next month (1-12) to recognize.
    obligation: (Uuid, u32),
    /// EUR/USD rate random walk (cents); moves in both directions so FX
    /// revaluations produce gains as well as losses.
    rate_cents: i64,
    counts: [usize; 6],
    closes_skipped_no_income: usize,
}

impl Sim {
    fn new(seed: u64) -> Self {
        let commerce = Commerce::new(":memory:").expect("commerce");
        let gl = commerce.general_ledger();
        gl.initialize_chart_of_accounts().expect("init chart");

        let sub = |number: &str, name: &str, ty: AccountType, sub_type: AccountSubType| {
            gl.create_account(CreateGlAccount {
                account_number: number.into(),
                name: name.into(),
                description: None,
                account_type: ty,
                account_sub_type: Some(sub_type),
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: None,
            })
            .expect("create account")
            .id
        };
        sub(
            "5300",
            "Depreciation Expense",
            AccountType::Expense,
            AccountSubType::DepreciationExpense,
        );
        sub(
            "1510",
            "Accumulated Depreciation",
            AccountType::Asset,
            AccountSubType::AccumulatedDepreciation,
        );
        let unearned_id = sub(
            "2300",
            "Unearned Revenue",
            AccountType::Liability,
            AccountSubType::UnearnedRevenue,
        );
        let fx_id = sub("7900", "FX Gain/Loss", AccountType::Expense, AccountSubType::OtherExpense);

        // EUR cash account for FX revaluation.
        let eur_id = gl
            .create_account(CreateGlAccount {
                account_number: "1015".into(),
                name: "EUR Cash".into(),
                description: None,
                account_type: AccountType::Asset,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: Some("EUR".parse().expect("EUR")),
            })
            .expect("create EUR account")
            .id;

        let by_number =
            |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
        gl.set_auto_posting_config(CreateAutoPostingConfig {
            config_name: "Ledger simulation".into(),
            cash_account_id: by_number("1010"),
            accounts_receivable_account_id: by_number("1100"),
            inventory_account_id: by_number("1200"),
            accounts_payable_account_id: by_number("2010"),
            unearned_revenue_account_id: Some(unearned_id),
            sales_revenue_account_id: by_number("4010"),
            shipping_revenue_account_id: None,
            cogs_account_id: by_number("5010"),
            bad_debt_expense_account_id: None,
            fx_gain_loss_account_id: Some(fx_id),
            auto_post_depreciation: true,
            auto_post_revenue_recognition: true,
        })
        .expect("set auto posting config");

        // Wide open period: GL auto-posting stamps entries with today's date,
        // which must fall inside an open period.
        let period = gl
            .create_period(CreateGlPeriod {
                period_name: "SIM-wide".into(),
                fiscal_year: 2026,
                period_number: 1,
                start_date: date(2020, 1, 1),
                end_date: date(2030, 12, 31),
            })
            .expect("create period");
        gl.open_period(period.id).expect("open period");

        // EUR/USD starting rate, then book 1,000 EUR so revaluation has a
        // foreign balance to work on. The seed entry carries the balance at
        // parity (1000 EUR booked as $1000); the rate then walks in both
        // directions, so revaluations produce gains and losses.
        let rate_cents = 100i64;
        commerce
            .currency()
            .set_rate(SetExchangeRate {
                base_currency: Currency::EUR,
                quote_currency: Currency::USD,
                rate: Decimal::new(rate_cents, 2),
                source: Some("sim".into()),
            })
            .expect("set rate");
        gl.create_journal_entry(CreateJournalEntry {
            entry_date: POSTING_DATE,
            entry_type: Some(JournalEntryType::Standard),
            description: "Seed EUR balance".into(),
            lines: vec![
                CreateJournalEntryLine {
                    account_id: eur_id,
                    description: None,
                    debit_amount: dec!(1000),
                    credit_amount: Decimal::ZERO,
                    reference_type: None,
                    reference_id: None,
                },
                CreateJournalEntryLine {
                    account_id: by_number("4010"),
                    description: None,
                    debit_amount: Decimal::ZERO,
                    credit_amount: dec!(1000),
                    reference_type: None,
                    reference_id: None,
                },
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(true),
        })
        .expect("seed EUR entry");

        let customer_id = commerce
            .customers()
            .create(CreateCustomer {
                email: "sim@example.com".into(),
                first_name: "Ledger".into(),
                last_name: "Sim".into(),
                ..Default::default()
            })
            .expect("create customer")
            .id;

        // Manual-JE pool: active balance-sheet posting accounts, excluding
        // the EUR account (keeps foreign balance under the sim's control).
        let je_accounts: Vec<Uuid> = gl
            .list_accounts(GlAccountFilter { is_posting: Some(true), ..Default::default() })
            .expect("list accounts")
            .into_iter()
            .filter(|a| {
                matches!(
                    a.account_type,
                    AccountType::Asset | AccountType::Liability | AccountType::Equity
                ) && a.id != eur_id
            })
            .map(|a| a.id)
            .collect();
        assert!(je_accounts.len() >= 2, "need at least two accounts for manual entries");

        let mut sim = Self {
            commerce,
            rng: Rng(seed),
            period_id: period.id,
            customer_id,
            je_accounts,
            asset: (Uuid::nil(), 0),
            obligation: (Uuid::nil(), 13),
            rate_cents,
            counts: [0; 6],
            closes_skipped_no_income: 0,
        };
        sim.asset = sim.new_asset();
        sim.obligation = sim.new_obligation();
        sim
    }

    fn new_asset(&mut self) -> (Uuid, u32) {
        let cost = self.rng.money(24_000, 1_200_000); // $240 .. $12,000
        let asset = self
            .commerce
            .fixed_assets()
            .create(CreateFixedAsset {
                asset_number: None,
                name: "Sim asset".into(),
                description: None,
                category: FixedAssetCategory::Machinery,
                acquisition_date: date(2026, 1, 1),
                acquisition_cost: cost,
                salvage_value: Decimal::ZERO,
                useful_life_months: 12,
                depreciation_method: DepreciationMethod::StraightLine,
                in_service_date: None,
                location_id: None,
                asset_account_id: None,
                accumulated_depreciation_account_id: None,
                depreciation_expense_account_id: None,
                currency: None,
            })
            .expect("create asset");
        let asset = self
            .commerce
            .fixed_assets()
            .place_in_service(asset.id, date(2026, 1, 1))
            .expect("place in service");
        self.commerce.fixed_assets().generate_schedule(asset.id).expect("generate schedule");
        (asset.id, 12)
    }

    fn new_obligation(&mut self) -> (Uuid, u32) {
        let amount = self.rng.money(12_000, 600_000); // $120 .. $6,000
        let contract = self
            .commerce
            .revenue_recognition()
            .create_contract(CreateRevenueContract {
                contract_number: None,
                customer_id: Uuid::new_v4(),
                order_id: None,
                invoice_id: None,
                transaction_price: amount,
                currency: None,
                effective_date: date(2026, 1, 1),
                obligations: vec![CreatePerformanceObligation {
                    description: "Sim support".into(),
                    standalone_selling_price: None,
                    allocated_amount: amount,
                    recognition_method: RecognitionMethod::RatableOverTime {
                        start: date(2026, 1, 1),
                        end: date(2026, 12, 31),
                    },
                }],
            })
            .expect("create contract");
        self.commerce
            .revenue_recognition()
            .update_contract(
                contract.id,
                UpdateRevenueContract {
                    status: Some(RevenueContractStatus::Active),
                    ..Default::default()
                },
            )
            .expect("activate contract");
        let obligation_id = contract.obligations[0].id;
        self.commerce
            .revenue_recognition()
            .generate_schedule(obligation_id)
            .expect("generate schedule");
        (obligation_id, 1)
    }

    fn op_invoice(&mut self) {
        let item_count = self.rng.range(1, 3);
        let items = (0..item_count)
            .map(|i| CreateInvoiceItem {
                description: format!("Sim line {i}"),
                quantity: Decimal::from(self.rng.range(1, 20)),
                unit_price: self.rng.money(100, 50_000),
                ..Default::default()
            })
            .collect();
        let invoice = self
            .commerce
            .invoices()
            .create(CreateInvoice { customer_id: self.customer_id, items, ..Default::default() })
            .expect("create invoice");
        let entry = self
            .commerce
            .general_ledger()
            .auto_post_invoice(invoice.id.into())
            .expect("auto-post invoice");
        assert!(entry.is_balanced, "invoice auto-post entry must balance");
    }

    fn op_manual_entry(&mut self) {
        let a = usize::try_from(self.rng.below(self.je_accounts.len() as u64)).expect("index");
        let mut b = usize::try_from(self.rng.below(self.je_accounts.len() as u64)).expect("index");
        if b == a {
            b = (b + 1) % self.je_accounts.len();
        }
        let amount = self.rng.money(1, 1_000_000);
        let entry = self
            .commerce
            .general_ledger()
            .create_journal_entry(CreateJournalEntry {
                entry_date: POSTING_DATE,
                entry_type: Some(JournalEntryType::Standard),
                description: "Sim manual entry".into(),
                lines: vec![
                    CreateJournalEntryLine {
                        account_id: self.je_accounts[a],
                        description: None,
                        debit_amount: amount,
                        credit_amount: Decimal::ZERO,
                        reference_type: None,
                        reference_id: None,
                    },
                    CreateJournalEntryLine {
                        account_id: self.je_accounts[b],
                        description: None,
                        debit_amount: Decimal::ZERO,
                        credit_amount: amount,
                        reference_type: None,
                        reference_id: None,
                    },
                ],
                source_document_type: None,
                source_document_id: None,
                auto_post: Some(true),
            })
            .expect("manual journal entry");
        assert!(entry.is_balanced, "manual entry must balance");
    }

    fn op_depreciation(&mut self) {
        if self.asset.1 == 0 {
            self.asset = self.new_asset();
        }
        let periods =
            u32::try_from(self.rng.range(1, i64::from(self.asset.1.min(3)))).expect("u32");
        self.commerce
            .fixed_assets()
            .post_depreciation(self.asset.0, periods)
            .expect("post depreciation");
        self.asset.1 -= periods;
    }

    fn op_revenue_recognition(&mut self) {
        if self.obligation.1 > 12 {
            self.obligation = self.new_obligation();
        }
        let month = self.obligation.1;
        self.commerce
            .revenue_recognition()
            .recognize_period(self.obligation.0, date(2026, month, 28))
            .expect("recognize revenue");
        self.obligation.1 += 1;
    }

    fn op_fx_revaluation(&mut self) {
        // Two-sided random walk: period close now posts contra-normal
        // income-statement balances (net FX gains on the expense-type
        // gain/loss account), so gains are exercised as well as losses.
        self.rate_cents = (self.rate_cents + self.rng.range(0, 5) - 2).max(50);
        self.commerce
            .currency()
            .set_rate(SetExchangeRate {
                base_currency: Currency::EUR,
                quote_currency: Currency::USD,
                rate: Decimal::new(self.rate_cents, 2),
                source: Some("sim".into()),
            })
            .expect("set rate");
        let result = self
            .commerce
            .general_ledger()
            .revalue(date(2026, 6, 30), None)
            .expect("fx revaluation");
        if let Some(entry) = result.journal_entry {
            assert!(entry.is_balanced, "revaluation entry must balance");
        }
    }

    fn op_period_close(&mut self) {
        match self.commerce.general_ledger().run_period_close(self.period_id, "sim") {
            Ok(entry) => {
                assert!(entry.is_balanced, "closing entry must balance");
                self.assert_trial_balance("after period close");
                // Reopen so the simulation can keep posting.
                self.commerce
                    .general_ledger()
                    .reopen_period(self.period_id)
                    .expect("reopen period");
            }
            // `run_period_close` refuses zero-net-income periods (documented
            // quirk) — e.g. right after a previous close zeroed everything.
            Err(stateset_core::CommerceError::ValidationError(msg))
                if msg.contains("No net income") =>
            {
                self.closes_skipped_no_income += 1;
            }
            Err(e) => panic!("period close failed: {e}"),
        }
    }

    fn assert_trial_balance(&self, context: &str) {
        let tb = self.commerce.general_ledger().get_trial_balance(AS_OF).expect("trial balance");
        assert_eq!(
            tb.total_debits, tb.total_credits,
            "trial balance out of balance {context}: debits {} != credits {}",
            tb.total_debits, tb.total_credits
        );
        assert!(tb.is_balanced, "trial balance is_balanced flag {context}");
    }

    fn step(&mut self, index: usize) {
        // Weighted op selection: invoices 30%, manual JEs 25%, depreciation
        // 15%, revenue recognition 15%, FX revaluation 10%, period close 5%.
        let roll = self.rng.below(100);
        let op = match roll {
            0..=29 => 0,
            30..=54 => 1,
            55..=69 => 2,
            70..=84 => 3,
            85..=94 => 4,
            _ => 5,
        };
        self.counts[op] += 1;
        match op {
            0 => self.op_invoice(),
            1 => self.op_manual_entry(),
            2 => self.op_depreciation(),
            3 => self.op_revenue_recognition(),
            4 => self.op_fx_revaluation(),
            _ => self.op_period_close(),
        }
        self.assert_trial_balance(&format!("after operation {index} (op kind {op})"));
    }
}

#[test]
fn randomized_ledger_simulation_stays_balanced() {
    let seed = std::env::var("LEDGER_SIM_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SEED);
    let mut sim = Sim::new(seed);
    sim.assert_trial_balance("after setup");

    for i in 0..OPERATIONS {
        sim.step(i);
    }

    // Final close: move net income to retained earnings, then the balance
    // sheet must satisfy assets == liabilities + equity.
    match sim.commerce.general_ledger().run_period_close(sim.period_id, "sim-final") {
        Ok(entry) => assert!(entry.is_balanced, "final closing entry must balance"),
        Err(stateset_core::CommerceError::ValidationError(msg))
            if msg.contains("No net income") =>
        {
            // Already fully closed by a late in-simulation close; the balance
            // sheet check below still applies.
            sim.closes_skipped_no_income += 1;
        }
        Err(e) => panic!("final period close failed: {e}"),
    }
    sim.assert_trial_balance("after final close");

    let bs = sim.commerce.general_ledger().get_balance_sheet(AS_OF).expect("balance sheet");
    assert!(
        bs.is_balanced(),
        "balance sheet after close: assets {} != liabilities {} + equity {} (seed {seed})",
        bs.total_assets,
        bs.total_liabilities,
        bs.total_equity
    );

    eprintln!(
        "ledger simulation seed={seed}: {} invoices, {} manual entries, {} depreciation posts, \
         {} revenue recognitions, {} FX revaluations, {} period closes ({} skipped: no net income)",
        sim.counts[0],
        sim.counts[1],
        sim.counts[2],
        sim.counts[3],
        sim.counts[4],
        sim.counts[5],
        sim.closes_skipped_no_income
    );
}
