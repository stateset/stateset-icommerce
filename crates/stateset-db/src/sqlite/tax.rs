//! SQLite implementation of tax repository

use chrono::{DateTime, NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use stateset_core::{
    CommerceError, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ExemptionType,
    JurisdictionLevel, ProductTaxCategory, Result, TaxAddress, TaxBreakdown, TaxCalculationMethod,
    TaxCalculationRequest, TaxCalculationResult, TaxCompoundMethod, TaxExemption, TaxJurisdiction,
    TaxJurisdictionFilter, TaxRate, TaxRateFilter, TaxSettings, TaxType, LineItemTax, TaxDetail,
    JurisdictionSummary,
};
use uuid::Uuid;

use super::map_db_error;

/// SQLite tax repository
pub struct SqliteTaxRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteTaxRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn parse_decimal(s: &str) -> Decimal {
        s.parse().unwrap_or_default()
    }

    fn parse_datetime(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(|_| Utc::now())
            })
    }

    fn parse_date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .unwrap_or_else(|_| Utc::now().date_naive())
    }

    fn parse_tax_type(s: &str) -> TaxType {
        match s {
            "sales_tax" => TaxType::SalesTax,
            "vat" => TaxType::Vat,
            "gst" => TaxType::Gst,
            "hst" => TaxType::Hst,
            "pst" => TaxType::Pst,
            "qst" => TaxType::Qst,
            "consumption_tax" => TaxType::ConsumptionTax,
            "custom" => TaxType::Custom,
            _ => TaxType::SalesTax,
        }
    }

    fn parse_jurisdiction_level(s: &str) -> JurisdictionLevel {
        match s {
            "country" => JurisdictionLevel::Country,
            "state" => JurisdictionLevel::State,
            "county" => JurisdictionLevel::County,
            "city" => JurisdictionLevel::City,
            "district" => JurisdictionLevel::District,
            "special" => JurisdictionLevel::Special,
            _ => JurisdictionLevel::Country,
        }
    }

    fn parse_product_category(s: &str) -> ProductTaxCategory {
        match s {
            "standard" => ProductTaxCategory::Standard,
            "reduced" => ProductTaxCategory::Reduced,
            "super_reduced" => ProductTaxCategory::SuperReduced,
            "zero_rated" => ProductTaxCategory::ZeroRated,
            "exempt" => ProductTaxCategory::Exempt,
            "digital" => ProductTaxCategory::Digital,
            "clothing" => ProductTaxCategory::Clothing,
            "food" => ProductTaxCategory::Food,
            "prepared_food" => ProductTaxCategory::PreparedFood,
            "medical" => ProductTaxCategory::Medical,
            "educational" => ProductTaxCategory::Educational,
            "luxury" => ProductTaxCategory::Luxury,
            _ => ProductTaxCategory::Standard,
        }
    }

    fn parse_exemption_type(s: &str) -> ExemptionType {
        match s {
            "resale" => ExemptionType::Resale,
            "non_profit" => ExemptionType::NonProfit,
            "government" => ExemptionType::Government,
            "educational" => ExemptionType::Educational,
            "religious" => ExemptionType::Religious,
            "medical" => ExemptionType::Medical,
            "manufacturing" => ExemptionType::Manufacturing,
            "agricultural" => ExemptionType::Agricultural,
            "export" => ExemptionType::Export,
            "diplomatic" => ExemptionType::Diplomatic,
            _ => ExemptionType::Other,
        }
    }

    fn parse_calculation_method(s: &str) -> TaxCalculationMethod {
        match s {
            "inclusive" => TaxCalculationMethod::Inclusive,
            _ => TaxCalculationMethod::Exclusive,
        }
    }

    fn parse_compound_method(s: &str) -> TaxCompoundMethod {
        match s {
            "compound" => TaxCompoundMethod::Compound,
            "separate" => TaxCompoundMethod::Separate,
            _ => TaxCompoundMethod::Combined,
        }
    }

    fn jurisdiction_level_str(level: JurisdictionLevel) -> &'static str {
        match level {
            JurisdictionLevel::Country => "country",
            JurisdictionLevel::State => "state",
            JurisdictionLevel::County => "county",
            JurisdictionLevel::City => "city",
            JurisdictionLevel::District => "district",
            JurisdictionLevel::Special => "special",
        }
    }

    fn exemption_type_str(t: ExemptionType) -> &'static str {
        match t {
            ExemptionType::Resale => "resale",
            ExemptionType::NonProfit => "non_profit",
            ExemptionType::Government => "government",
            ExemptionType::Educational => "educational",
            ExemptionType::Religious => "religious",
            ExemptionType::Medical => "medical",
            ExemptionType::Manufacturing => "manufacturing",
            ExemptionType::Agricultural => "agricultural",
            ExemptionType::Export => "export",
            ExemptionType::Diplomatic => "diplomatic",
            ExemptionType::Other => "other",
        }
    }
}

// ============================================================================
// Jurisdiction Operations
// ============================================================================

impl SqliteTaxRepository {
    /// Get a jurisdiction by ID
    pub fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE id = ?",
            params![id.to_string()],
            |row| {
                let postal_codes_json: String = row.get(9)?;
                let postal_codes: Vec<String> = serde_json::from_str(&postal_codes_json).unwrap_or_default();

                Ok(TaxJurisdiction {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    parent_id: row.get::<_, Option<String>>(1)?.and_then(|s| s.parse().ok()),
                    name: row.get(2)?,
                    code: row.get(3)?,
                    level: Self::parse_jurisdiction_level(&row.get::<_, String>(4)?),
                    country_code: row.get(5)?,
                    state_code: row.get(6)?,
                    county: row.get(7)?,
                    city: row.get(8)?,
                    postal_codes,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: Self::parse_datetime(&row.get::<_, String>(11)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(12)?),
                })
            },
        );

        match result {
            Ok(jurisdiction) => Ok(Some(jurisdiction)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Get a jurisdiction by code (e.g., "US-CA")
    pub fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE code = ?",
            params![code],
            |row| {
                let postal_codes_json: String = row.get(9)?;
                let postal_codes: Vec<String> = serde_json::from_str(&postal_codes_json).unwrap_or_default();

                Ok(TaxJurisdiction {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    parent_id: row.get::<_, Option<String>>(1)?.and_then(|s| s.parse().ok()),
                    name: row.get(2)?,
                    code: row.get(3)?,
                    level: Self::parse_jurisdiction_level(&row.get::<_, String>(4)?),
                    country_code: row.get(5)?,
                    state_code: row.get(6)?,
                    county: row.get(7)?,
                    city: row.get(8)?,
                    postal_codes,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: Self::parse_datetime(&row.get::<_, String>(11)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(12)?),
                })
            },
        );

        match result {
            Ok(jurisdiction) => Ok(Some(jurisdiction)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// List jurisdictions with optional filter
    pub fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE 1=1"
        );
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(country) = &filter.country_code {
            query.push_str(" AND country_code = ?");
            params_vec.push(country.clone());
        }

        if let Some(state) = &filter.state_code {
            query.push_str(" AND state_code = ?");
            params_vec.push(state.clone());
        }

        if let Some(level) = &filter.level {
            query.push_str(" AND level = ?");
            params_vec.push(Self::jurisdiction_level_str(*level).to_string());
        }

        if filter.active_only {
            query.push_str(" AND active = 1");
        }

        query.push_str(" ORDER BY country_code, state_code, level, name");

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            let postal_codes_json: String = row.get(9)?;
            let postal_codes: Vec<String> = serde_json::from_str(&postal_codes_json).unwrap_or_default();

            Ok(TaxJurisdiction {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                parent_id: row.get::<_, Option<String>>(1)?.and_then(|s| s.parse().ok()),
                name: row.get(2)?,
                code: row.get(3)?,
                level: Self::parse_jurisdiction_level(&row.get::<_, String>(4)?),
                country_code: row.get(5)?,
                state_code: row.get(6)?,
                county: row.get(7)?,
                city: row.get(8)?,
                postal_codes,
                active: row.get::<_, i32>(10)? != 0,
                created_at: Self::parse_datetime(&row.get::<_, String>(11)?),
                updated_at: Self::parse_datetime(&row.get::<_, String>(12)?),
            })
        }).map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Create a new jurisdiction
    pub fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let postal_codes_json = serde_json::to_string(&input.postal_codes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tax_jurisdictions (id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
            params![
                id.to_string(),
                input.parent_id.map(|id| id.to_string()),
                input.name,
                input.code,
                Self::jurisdiction_level_str(input.level),
                input.country_code,
                input.state_code,
                input.county,
                input.city,
                postal_codes_json,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        self.get_jurisdiction(id)?.ok_or(CommerceError::NotFound)
    }
}

// ============================================================================
// Tax Rate Operations
// ============================================================================

impl SqliteTaxRepository {
    /// Get a tax rate by ID
    pub fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, jurisdiction_id, tax_type, product_category, rate, name, description, is_compound, priority, threshold_min, threshold_max, fixed_amount, effective_from, effective_to, active, created_at, updated_at
             FROM tax_rates WHERE id = ?",
            params![id.to_string()],
            |row| {
                Ok(TaxRate {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    jurisdiction_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                    tax_type: Self::parse_tax_type(&row.get::<_, String>(2)?),
                    product_category: Self::parse_product_category(&row.get::<_, String>(3)?),
                    rate: Self::parse_decimal(&row.get::<_, String>(4)?),
                    name: row.get(5)?,
                    description: row.get(6)?,
                    is_compound: row.get::<_, i32>(7)? != 0,
                    priority: row.get(8)?,
                    threshold_min: row.get::<_, Option<String>>(9)?.map(|s| Self::parse_decimal(&s)),
                    threshold_max: row.get::<_, Option<String>>(10)?.map(|s| Self::parse_decimal(&s)),
                    fixed_amount: row.get::<_, Option<String>>(11)?.map(|s| Self::parse_decimal(&s)),
                    effective_from: Self::parse_date(&row.get::<_, String>(12)?),
                    effective_to: row.get::<_, Option<String>>(13)?.map(|s| Self::parse_date(&s)),
                    active: row.get::<_, i32>(14)? != 0,
                    created_at: Self::parse_datetime(&row.get::<_, String>(15)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(16)?),
                })
            },
        );

        match result {
            Ok(rate) => Ok(Some(rate)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// List tax rates with optional filter
    pub fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, jurisdiction_id, tax_type, product_category, rate, name, description, is_compound, priority, threshold_min, threshold_max, fixed_amount, effective_from, effective_to, active, created_at, updated_at
             FROM tax_rates WHERE 1=1"
        );
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(jurisdiction_id) = &filter.jurisdiction_id {
            query.push_str(" AND jurisdiction_id = ?");
            params_vec.push(jurisdiction_id.to_string());
        }

        if let Some(tax_type) = &filter.tax_type {
            query.push_str(" AND tax_type = ?");
            params_vec.push(tax_type.as_str().to_string());
        }

        if let Some(category) = &filter.product_category {
            query.push_str(" AND product_category = ?");
            params_vec.push(category.as_str().to_string());
        }

        if filter.active_only {
            query.push_str(" AND active = 1");
        }

        if let Some(date) = &filter.effective_date {
            query.push_str(" AND effective_from <= ? AND (effective_to IS NULL OR effective_to >= ?)");
            params_vec.push(date.to_string());
            params_vec.push(date.to_string());
        }

        query.push_str(" ORDER BY priority, name");

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok(TaxRate {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                jurisdiction_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                tax_type: Self::parse_tax_type(&row.get::<_, String>(2)?),
                product_category: Self::parse_product_category(&row.get::<_, String>(3)?),
                rate: Self::parse_decimal(&row.get::<_, String>(4)?),
                name: row.get(5)?,
                description: row.get(6)?,
                is_compound: row.get::<_, i32>(7)? != 0,
                priority: row.get(8)?,
                threshold_min: row.get::<_, Option<String>>(9)?.map(|s| Self::parse_decimal(&s)),
                threshold_max: row.get::<_, Option<String>>(10)?.map(|s| Self::parse_decimal(&s)),
                fixed_amount: row.get::<_, Option<String>>(11)?.map(|s| Self::parse_decimal(&s)),
                effective_from: Self::parse_date(&row.get::<_, String>(12)?),
                effective_to: row.get::<_, Option<String>>(13)?.map(|s| Self::parse_date(&s)),
                active: row.get::<_, i32>(14)? != 0,
                created_at: Self::parse_datetime(&row.get::<_, String>(15)?),
                updated_at: Self::parse_datetime(&row.get::<_, String>(16)?),
            })
        }).map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Get rates for a jurisdiction and product category
    pub fn get_rates_for_address(&self, address: &TaxAddress, category: ProductTaxCategory, date: NaiveDate) -> Result<Vec<TaxRate>> {
        // Find applicable jurisdictions (country, state, etc.)
        let mut jurisdiction_ids = Vec::new();

        // Get country jurisdiction
        if let Some(country) = self.get_jurisdiction_by_code(&address.country)? {
            jurisdiction_ids.push(country.id);
        }

        // Get state jurisdiction if applicable
        if let Some(state) = &address.state {
            let state_code = format!("{}-{}", address.country, state);
            if let Some(state_jurisdiction) = self.get_jurisdiction_by_code(&state_code)? {
                jurisdiction_ids.push(state_jurisdiction.id);
            }
        }

        if jurisdiction_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Get all applicable rates
        let mut all_rates = Vec::new();
        for jurisdiction_id in jurisdiction_ids {
            let filter = TaxRateFilter {
                jurisdiction_id: Some(jurisdiction_id),
                product_category: Some(category),
                active_only: true,
                effective_date: Some(date),
                ..Default::default()
            };
            let rates = self.list_rates(filter)?;
            all_rates.extend(rates);
        }

        // Sort by priority
        all_rates.sort_by_key(|r| r.priority);
        Ok(all_rates)
    }

    /// Create a new tax rate
    pub fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tax_rates (id, jurisdiction_id, tax_type, product_category, rate, name, description, is_compound, priority, threshold_min, threshold_max, fixed_amount, effective_from, effective_to, active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
            params![
                id.to_string(),
                input.jurisdiction_id.to_string(),
                input.tax_type.as_str(),
                input.product_category.as_str(),
                input.rate.to_string(),
                input.name,
                input.description,
                input.is_compound as i32,
                input.priority,
                input.threshold_min.map(|d| d.to_string()),
                input.threshold_max.map(|d| d.to_string()),
                input.fixed_amount.map(|d| d.to_string()),
                input.effective_from.to_string(),
                input.effective_to.map(|d| d.to_string()),
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        self.get_rate(id)?.ok_or(CommerceError::NotFound)
    }
}

// ============================================================================
// Tax Exemption Operations
// ============================================================================

impl SqliteTaxRepository {
    /// Get an exemption by ID
    pub fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority, jurisdiction_ids, exempt_categories, effective_from, expires_at, verified, verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions WHERE id = ?",
            params![id.to_string()],
            |row| {
                let jurisdiction_ids_json: String = row.get(5)?;
                let jurisdiction_ids: Vec<Uuid> = serde_json::from_str::<Vec<String>>(&jurisdiction_ids_json)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                let categories_json: String = row.get(6)?;
                let exempt_categories: Vec<ProductTaxCategory> = serde_json::from_str::<Vec<String>>(&categories_json)
                    .unwrap_or_default()
                    .iter()
                    .map(|s| Self::parse_product_category(s))
                    .collect();

                Ok(TaxExemption {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    customer_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                    exemption_type: Self::parse_exemption_type(&row.get::<_, String>(2)?),
                    certificate_number: row.get(3)?,
                    issuing_authority: row.get(4)?,
                    jurisdiction_ids,
                    exempt_categories,
                    effective_from: Self::parse_date(&row.get::<_, String>(7)?),
                    expires_at: row.get::<_, Option<String>>(8)?.map(|s| Self::parse_date(&s)),
                    verified: row.get::<_, i32>(9)? != 0,
                    verified_at: row.get::<_, Option<String>>(10)?.map(|s| Self::parse_datetime(&s)),
                    notes: row.get(11)?,
                    active: row.get::<_, i32>(12)? != 0,
                    created_at: Self::parse_datetime(&row.get::<_, String>(13)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(14)?),
                })
            },
        );

        match result {
            Ok(exemption) => Ok(Some(exemption)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Get active exemptions for a customer
    pub fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let today = Utc::now().date_naive().to_string();

        let mut stmt = conn.prepare(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority, jurisdiction_ids, exempt_categories, effective_from, expires_at, verified, verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions
             WHERE customer_id = ? AND active = 1 AND effective_from <= ? AND (expires_at IS NULL OR expires_at >= ?)"
        ).map_err(map_db_error)?;

        let rows = stmt.query_map(params![customer_id.to_string(), &today, &today], |row| {
            let jurisdiction_ids_json: String = row.get(5)?;
            let jurisdiction_ids: Vec<Uuid> = serde_json::from_str::<Vec<String>>(&jurisdiction_ids_json)
                .unwrap_or_default()
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect();

            let categories_json: String = row.get(6)?;
            let exempt_categories: Vec<ProductTaxCategory> = serde_json::from_str::<Vec<String>>(&categories_json)
                .unwrap_or_default()
                .iter()
                .map(|s| Self::parse_product_category(s))
                .collect();

            Ok(TaxExemption {
                id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                customer_id: row.get::<_, String>(1)?.parse().unwrap_or_default(),
                exemption_type: Self::parse_exemption_type(&row.get::<_, String>(2)?),
                certificate_number: row.get(3)?,
                issuing_authority: row.get(4)?,
                jurisdiction_ids,
                exempt_categories,
                effective_from: Self::parse_date(&row.get::<_, String>(7)?),
                expires_at: row.get::<_, Option<String>>(8)?.map(|s| Self::parse_date(&s)),
                verified: row.get::<_, i32>(9)? != 0,
                verified_at: row.get::<_, Option<String>>(10)?.map(|s| Self::parse_datetime(&s)),
                notes: row.get(11)?,
                active: row.get::<_, i32>(12)? != 0,
                created_at: Self::parse_datetime(&row.get::<_, String>(13)?),
                updated_at: Self::parse_datetime(&row.get::<_, String>(14)?),
            })
        }).map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Create a new exemption
    pub fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let jurisdiction_ids_json = serde_json::to_string(
            &input.jurisdiction_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
        ).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let categories_json = serde_json::to_string(
            &input.exempt_categories.iter().map(|c| c.as_str()).collect::<Vec<_>>()
        ).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tax_exemptions (id, customer_id, exemption_type, certificate_number, issuing_authority, jurisdiction_ids, exempt_categories, effective_from, expires_at, verified, notes, active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, 1, ?, ?)",
            params![
                id.to_string(),
                input.customer_id.to_string(),
                Self::exemption_type_str(input.exemption_type),
                input.certificate_number,
                input.issuing_authority,
                jurisdiction_ids_json,
                categories_json,
                input.effective_from.to_string(),
                input.expires_at.map(|d| d.to_string()),
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        self.get_exemption(id)?.ok_or(CommerceError::NotFound)
    }
}

// ============================================================================
// Tax Settings Operations
// ============================================================================

impl SqliteTaxRepository {
    /// Get tax settings
    pub fn get_settings(&self) -> Result<TaxSettings> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT id, enabled, calculation_method, compound_method, tax_shipping, tax_handling, tax_gift_wrap, origin_address, default_product_category, rounding_mode, decimal_places, validate_addresses, tax_provider, provider_credentials, created_at, updated_at
             FROM tax_settings WHERE id = 'default'",
            [],
            |row| {
                let origin_address: Option<TaxAddress> = row.get::<_, Option<String>>(7)?
                    .and_then(|s| serde_json::from_str(&s).ok());

                Ok(TaxSettings {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    enabled: row.get::<_, i32>(1)? != 0,
                    calculation_method: Self::parse_calculation_method(&row.get::<_, String>(2)?),
                    compound_method: Self::parse_compound_method(&row.get::<_, String>(3)?),
                    tax_shipping: row.get::<_, i32>(4)? != 0,
                    tax_handling: row.get::<_, i32>(5)? != 0,
                    tax_gift_wrap: row.get::<_, i32>(6)? != 0,
                    origin_address,
                    default_product_category: Self::parse_product_category(&row.get::<_, String>(8)?),
                    rounding_mode: row.get(9)?,
                    decimal_places: row.get(10)?,
                    validate_addresses: row.get::<_, i32>(11)? != 0,
                    tax_provider: row.get(12)?,
                    provider_credentials: row.get(13)?,
                    created_at: Self::parse_datetime(&row.get::<_, String>(14)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(15)?),
                })
            },
        );

        match result {
            Ok(settings) => Ok(settings),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(TaxSettings::default()),
            Err(e) => Err(map_db_error(e)),
        }
    }

    /// Update tax settings
    pub fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let origin_address_json = settings.origin_address
            .as_ref()
            .map(|a| serde_json::to_string(a))
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let calc_method = match settings.calculation_method {
            TaxCalculationMethod::Inclusive => "inclusive",
            TaxCalculationMethod::Exclusive => "exclusive",
        };

        let compound_method = match settings.compound_method {
            TaxCompoundMethod::Combined => "combined",
            TaxCompoundMethod::Compound => "compound",
            TaxCompoundMethod::Separate => "separate",
        };

        conn.execute(
            "INSERT INTO tax_settings (id, enabled, calculation_method, compound_method, tax_shipping, tax_handling, tax_gift_wrap, origin_address, default_product_category, rounding_mode, decimal_places, validate_addresses, tax_provider, provider_credentials, updated_at)
             VALUES ('default', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT (id) DO UPDATE SET
                enabled = excluded.enabled,
                calculation_method = excluded.calculation_method,
                compound_method = excluded.compound_method,
                tax_shipping = excluded.tax_shipping,
                tax_handling = excluded.tax_handling,
                tax_gift_wrap = excluded.tax_gift_wrap,
                origin_address = excluded.origin_address,
                default_product_category = excluded.default_product_category,
                rounding_mode = excluded.rounding_mode,
                decimal_places = excluded.decimal_places,
                validate_addresses = excluded.validate_addresses,
                tax_provider = excluded.tax_provider,
                provider_credentials = excluded.provider_credentials,
                updated_at = excluded.updated_at",
            params![
                settings.enabled as i32,
                calc_method,
                compound_method,
                settings.tax_shipping as i32,
                settings.tax_handling as i32,
                settings.tax_gift_wrap as i32,
                origin_address_json,
                settings.default_product_category.as_str(),
                settings.rounding_mode,
                settings.decimal_places,
                settings.validate_addresses as i32,
                settings.tax_provider,
                settings.provider_credentials
            ],
        ).map_err(map_db_error)?;

        self.get_settings()
    }
}

// ============================================================================
// Tax Calculation
// ============================================================================

impl SqliteTaxRepository {
    /// Calculate tax for a request
    pub fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        let settings = self.get_settings()?;
        let now = Utc::now();
        let transaction_date = request.transaction_date.unwrap_or_else(|| now.date_naive());

        // Check for customer exemptions
        let exemptions = if let Some(customer_id) = request.customer_id {
            self.get_customer_exemptions(customer_id)?
        } else {
            Vec::new()
        };

        let mut subtotal = Decimal::ZERO;
        let mut total_tax = Decimal::ZERO;
        let mut line_item_taxes = Vec::new();
        let mut tax_breakdown: Vec<TaxBreakdown> = Vec::new();
        let mut jurisdictions_map = std::collections::HashMap::new();

        // Calculate tax for each line item
        for item in &request.line_items {
            let line_amount = item.unit_price * item.quantity - item.discount_amount;
            subtotal += line_amount;

            // Check if item is exempt due to customer exemption
            let is_exempt = exemptions.iter().any(|e| {
                e.exempt_categories.is_empty() || e.exempt_categories.contains(&item.tax_category)
            });

            if is_exempt || item.tax_category == ProductTaxCategory::Exempt {
                line_item_taxes.push(LineItemTax {
                    line_item_id: item.id.clone(),
                    taxable_amount: line_amount,
                    tax_amount: Decimal::ZERO,
                    effective_rate: Decimal::ZERO,
                    is_exempt: true,
                    exemption_reason: Some("Customer exemption".to_string()),
                    tax_details: Vec::new(),
                });
                continue;
            }

            // Get applicable tax rates
            let rates = self.get_rates_for_address(&request.shipping_address, item.tax_category, transaction_date)?;

            let mut line_tax = Decimal::ZERO;
            let mut line_tax_details = Vec::new();
            let taxable_base = line_amount;

            for rate in &rates {
                let tax_amount = if rate.is_compound {
                    // Compound tax is applied on (subtotal + previous taxes)
                    (taxable_base + line_tax) * rate.rate
                } else {
                    taxable_base * rate.rate
                };

                // Apply thresholds if set
                let final_tax = if let Some(max) = rate.threshold_max {
                    if taxable_base > max {
                        max * rate.rate
                    } else {
                        tax_amount
                    }
                } else {
                    tax_amount
                };

                line_tax += final_tax;

                // Get jurisdiction info
                if let Some(jurisdiction) = self.get_jurisdiction(rate.jurisdiction_id)? {
                    jurisdictions_map.entry(jurisdiction.id).or_insert_with(|| JurisdictionSummary {
                        id: jurisdiction.id,
                        name: jurisdiction.name.clone(),
                        code: jurisdiction.code.clone(),
                        level: jurisdiction.level,
                        total_rate: Decimal::ZERO,
                        total_tax: Decimal::ZERO,
                    });

                    if let Some(summary) = jurisdictions_map.get_mut(&jurisdiction.id) {
                        summary.total_rate += rate.rate;
                        summary.total_tax += final_tax;
                    }

                    // Add to breakdown
                    if let Some(existing) = tax_breakdown.iter_mut().find(|b| b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type) {
                        existing.taxable_amount += taxable_base;
                        existing.tax_amount += final_tax;
                    } else {
                        tax_breakdown.push(TaxBreakdown {
                            jurisdiction_id: jurisdiction.id,
                            jurisdiction_name: jurisdiction.name.clone(),
                            tax_type: rate.tax_type,
                            rate_name: rate.name.clone(),
                            rate: rate.rate,
                            taxable_amount: taxable_base,
                            tax_amount: final_tax,
                            is_compound: rate.is_compound,
                        });
                    }

                    line_tax_details.push(TaxDetail {
                        tax_type: rate.tax_type,
                        jurisdiction_name: jurisdiction.name,
                        rate: rate.rate,
                        amount: final_tax,
                    });
                }
            }

            let effective_rate = if line_amount.is_zero() {
                Decimal::ZERO
            } else {
                line_tax / line_amount
            };

            total_tax += line_tax;
            line_item_taxes.push(LineItemTax {
                line_item_id: item.id.clone(),
                taxable_amount: line_amount,
                tax_amount: line_tax,
                effective_rate,
                is_exempt: false,
                exemption_reason: None,
                tax_details: line_tax_details,
            });
        }

        // Calculate shipping tax if applicable
        let mut shipping_tax = Decimal::ZERO;
        if settings.tax_shipping {
            if let Some(shipping_amount) = request.shipping_amount {
                let shipping_rates = self.get_rates_for_address(&request.shipping_address, ProductTaxCategory::Standard, transaction_date)?;
                for rate in shipping_rates {
                    shipping_tax += shipping_amount * rate.rate;
                }
                total_tax += shipping_tax;
            }
        }

        // Round tax
        let decimal_places = settings.decimal_places as u32;
        let total_tax = total_tax.round_dp(decimal_places);
        let shipping_tax = shipping_tax.round_dp(decimal_places);

        let total = subtotal + total_tax + request.shipping_amount.unwrap_or_default();

        Ok(TaxCalculationResult {
            id: Uuid::new_v4(),
            total_tax,
            subtotal,
            total,
            shipping_tax,
            tax_breakdown,
            line_item_taxes,
            exemptions_applied: !exemptions.is_empty(),
            exemption_details: None, // Could populate if needed
            jurisdictions: jurisdictions_map.into_values().collect(),
            calculated_at: now,
            is_estimate: true,
        })
    }

    /// Save a tax calculation to the database
    pub fn save_calculation(&self, result: &TaxCalculationResult, order_id: Option<Uuid>, cart_id: Option<Uuid>, customer_id: Option<Uuid>, address: &TaxAddress, currency: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let address_json = serde_json::to_string(address)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let line_items_json = serde_json::to_string(&result.line_item_taxes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let breakdown_json = serde_json::to_string(&result.tax_breakdown)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let exemption_json = result.exemption_details.as_ref()
            .map(|e| serde_json::to_string(e))
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tax_calculations (id, order_id, cart_id, customer_id, subtotal, total_tax, shipping_tax, total, currency, shipping_address, line_items, tax_breakdown, exemptions_applied, exemption_details, is_estimate, calculated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                result.id.to_string(),
                order_id.map(|id| id.to_string()),
                cart_id.map(|id| id.to_string()),
                customer_id.map(|id| id.to_string()),
                result.subtotal.to_string(),
                result.total_tax.to_string(),
                result.shipping_tax.to_string(),
                result.total.to_string(),
                currency,
                address_json,
                line_items_json,
                breakdown_json,
                result.exemptions_applied as i32,
                exemption_json,
                result.is_estimate as i32,
                result.calculated_at.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        Ok(())
    }
}
