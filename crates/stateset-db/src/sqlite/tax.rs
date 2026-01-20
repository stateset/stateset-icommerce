//! SQLite implementation of tax repository

use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use stateset_core::{
    CommerceError, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ProductTaxCategory,
    Result, TaxAddress, TaxBreakdown, TaxCalculationRequest, TaxCalculationResult, TaxExemption,
    TaxJurisdiction, TaxJurisdictionFilter, TaxRate, TaxRateFilter, TaxRepository, TaxSettings,
    LineItemTax, TaxDetail, JurisdictionSummary,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_date_row, parse_datetime_opt_row, parse_datetime_row, parse_decimal_opt_row,
    parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_json_row, parse_uuid_opt_row,
    parse_uuid_row,
};

/// SQLite tax repository
pub struct SqliteTaxRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteTaxRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn parse_date_opt(
        value: Option<String>,
        entity: &str,
        field: &str,
    ) -> rusqlite::Result<Option<NaiveDate>> {
        match value {
            Some(ref val) if !val.is_empty() => Ok(Some(parse_date_row(val, entity, field)?)),
            _ => Ok(None),
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
                let postal_codes: Vec<String> =
                    parse_json_row(&postal_codes_json, "tax_jurisdiction", "postal_codes")?;

                Ok(TaxJurisdiction {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_jurisdiction", "id")?,
                    parent_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(1)?,
                        "tax_jurisdiction",
                        "parent_id",
                    )?,
                    name: row.get(2)?,
                    code: row.get(3)?,
                    level: parse_enum_row(&row.get::<_, String>(4)?, "tax_jurisdiction", "level")?,
                    country_code: row.get(5)?,
                    state_code: row.get(6)?,
                    county: row.get(7)?,
                    city: row.get(8)?,
                    postal_codes,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(11)?,
                        "tax_jurisdiction",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(12)?,
                        "tax_jurisdiction",
                        "updated_at",
                    )?,
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
                let postal_codes: Vec<String> =
                    parse_json_row(&postal_codes_json, "tax_jurisdiction", "postal_codes")?;

                Ok(TaxJurisdiction {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_jurisdiction", "id")?,
                    parent_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(1)?,
                        "tax_jurisdiction",
                        "parent_id",
                    )?,
                    name: row.get(2)?,
                    code: row.get(3)?,
                    level: parse_enum_row(&row.get::<_, String>(4)?, "tax_jurisdiction", "level")?,
                    country_code: row.get(5)?,
                    state_code: row.get(6)?,
                    county: row.get(7)?,
                    city: row.get(8)?,
                    postal_codes,
                    active: row.get::<_, i32>(10)? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(11)?,
                        "tax_jurisdiction",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(12)?,
                        "tax_jurisdiction",
                        "updated_at",
                    )?,
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
            params_vec.push(level.to_string());
        }

        if filter.active_only {
            query.push_str(" AND active = 1");
        }

        query.push_str(" ORDER BY country_code, state_code, level, name");

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params.as_slice(), |row| {
            let postal_codes_json: String = row.get(9)?;
            let postal_codes: Vec<String> =
                parse_json_row(&postal_codes_json, "tax_jurisdiction", "postal_codes")?;

            Ok(TaxJurisdiction {
                id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_jurisdiction", "id")?,
                parent_id: parse_uuid_opt_row(
                    row.get::<_, Option<String>>(1)?,
                    "tax_jurisdiction",
                    "parent_id",
                )?,
                name: row.get(2)?,
                code: row.get(3)?,
                level: parse_enum_row(&row.get::<_, String>(4)?, "tax_jurisdiction", "level")?,
                country_code: row.get(5)?,
                state_code: row.get(6)?,
                county: row.get(7)?,
                city: row.get(8)?,
                postal_codes,
                active: row.get::<_, i32>(10)? != 0,
                created_at: parse_datetime_row(
                    &row.get::<_, String>(11)?,
                    "tax_jurisdiction",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>(12)?,
                    "tax_jurisdiction",
                    "updated_at",
                )?,
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
                input.level.to_string(),
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
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_rate", "id")?,
                    jurisdiction_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "tax_rate",
                        "jurisdiction_id",
                    )?,
                    tax_type: parse_enum_row(&row.get::<_, String>(2)?, "tax_rate", "tax_type")?,
                    product_category: parse_enum_row(
                        &row.get::<_, String>(3)?,
                        "tax_rate",
                        "product_category",
                    )?,
                    rate: parse_decimal_row(&row.get::<_, String>(4)?, "tax_rate", "rate")?,
                    name: row.get(5)?,
                    description: row.get(6)?,
                    is_compound: row.get::<_, i32>(7)? != 0,
                    priority: row.get(8)?,
                    threshold_min: parse_decimal_opt_row(
                        row.get::<_, Option<String>>(9)?,
                        "tax_rate",
                        "threshold_min",
                    )?,
                    threshold_max: parse_decimal_opt_row(
                        row.get::<_, Option<String>>(10)?,
                        "tax_rate",
                        "threshold_max",
                    )?,
                    fixed_amount: parse_decimal_opt_row(
                        row.get::<_, Option<String>>(11)?,
                        "tax_rate",
                        "fixed_amount",
                    )?,
                    effective_from: parse_date_row(
                        &row.get::<_, String>(12)?,
                        "tax_rate",
                        "effective_from",
                    )?,
                    effective_to: Self::parse_date_opt(
                        row.get::<_, Option<String>>(13)?,
                        "tax_rate",
                        "effective_to",
                    )?,
                    active: row.get::<_, i32>(14)? != 0,
                    created_at: parse_datetime_row(&row.get::<_, String>(15)?, "tax_rate", "created_at")?,
                    updated_at: parse_datetime_row(&row.get::<_, String>(16)?, "tax_rate", "updated_at")?,
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
            params_vec.push(tax_type.to_string());
        }

        if let Some(category) = &filter.product_category {
            query.push_str(" AND product_category = ?");
            params_vec.push(category.to_string());
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
                id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_rate", "id")?,
                jurisdiction_id: parse_uuid_row(
                    &row.get::<_, String>(1)?,
                    "tax_rate",
                    "jurisdiction_id",
                )?,
                tax_type: parse_enum_row(&row.get::<_, String>(2)?, "tax_rate", "tax_type")?,
                product_category: parse_enum_row(
                    &row.get::<_, String>(3)?,
                    "tax_rate",
                    "product_category",
                )?,
                rate: parse_decimal_row(&row.get::<_, String>(4)?, "tax_rate", "rate")?,
                name: row.get(5)?,
                description: row.get(6)?,
                is_compound: row.get::<_, i32>(7)? != 0,
                priority: row.get(8)?,
                threshold_min: parse_decimal_opt_row(
                    row.get::<_, Option<String>>(9)?,
                    "tax_rate",
                    "threshold_min",
                )?,
                threshold_max: parse_decimal_opt_row(
                    row.get::<_, Option<String>>(10)?,
                    "tax_rate",
                    "threshold_max",
                )?,
                fixed_amount: parse_decimal_opt_row(
                    row.get::<_, Option<String>>(11)?,
                    "tax_rate",
                    "fixed_amount",
                )?,
                effective_from: parse_date_row(
                    &row.get::<_, String>(12)?,
                    "tax_rate",
                    "effective_from",
                )?,
                effective_to: Self::parse_date_opt(
                    row.get::<_, Option<String>>(13)?,
                    "tax_rate",
                    "effective_to",
                )?,
                active: row.get::<_, i32>(14)? != 0,
                created_at: parse_datetime_row(&row.get::<_, String>(15)?, "tax_rate", "created_at")?,
                updated_at: parse_datetime_row(&row.get::<_, String>(16)?, "tax_rate", "updated_at")?,
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
                input.tax_type.to_string(),
                input.product_category.to_string(),
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
                let raw_jurisdiction_ids: Vec<String> = parse_json_row(
                    &jurisdiction_ids_json,
                    "tax_exemption",
                    "jurisdiction_ids",
                )?;
                let jurisdiction_ids = raw_jurisdiction_ids
                    .into_iter()
                    .map(|value| parse_uuid_row(&value, "tax_exemption", "jurisdiction_ids"))
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                let categories_json: String = row.get(6)?;
                let raw_categories: Vec<String> =
                    parse_json_row(&categories_json, "tax_exemption", "exempt_categories")?;
                let exempt_categories = raw_categories
                    .into_iter()
                    .map(|value| parse_enum_row(&value, "tax_exemption", "exempt_categories"))
                    .collect::<rusqlite::Result<Vec<_>>>()?;

                Ok(TaxExemption {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_exemption", "id")?,
                    customer_id: parse_uuid_row(
                        &row.get::<_, String>(1)?,
                        "tax_exemption",
                        "customer_id",
                    )?,
                    exemption_type: parse_enum_row(
                        &row.get::<_, String>(2)?,
                        "tax_exemption",
                        "exemption_type",
                    )?,
                    certificate_number: row.get(3)?,
                    issuing_authority: row.get(4)?,
                    jurisdiction_ids,
                    exempt_categories,
                    effective_from: parse_date_row(
                        &row.get::<_, String>(7)?,
                        "tax_exemption",
                        "effective_from",
                    )?,
                    expires_at: Self::parse_date_opt(
                        row.get::<_, Option<String>>(8)?,
                        "tax_exemption",
                        "expires_at",
                    )?,
                    verified: row.get::<_, i32>(9)? != 0,
                    verified_at: parse_datetime_opt_row(
                        row.get::<_, Option<String>>(10)?,
                        "tax_exemption",
                        "verified_at",
                    )?,
                    notes: row.get(11)?,
                    active: row.get::<_, i32>(12)? != 0,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(13)?,
                        "tax_exemption",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(14)?,
                        "tax_exemption",
                        "updated_at",
                    )?,
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
            let raw_jurisdiction_ids: Vec<String> =
                parse_json_row(&jurisdiction_ids_json, "tax_exemption", "jurisdiction_ids")?;
            let jurisdiction_ids = raw_jurisdiction_ids
                .into_iter()
                .map(|value| parse_uuid_row(&value, "tax_exemption", "jurisdiction_ids"))
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let categories_json: String = row.get(6)?;
            let raw_categories: Vec<String> =
                parse_json_row(&categories_json, "tax_exemption", "exempt_categories")?;
            let exempt_categories = raw_categories
                .into_iter()
                .map(|value| parse_enum_row(&value, "tax_exemption", "exempt_categories"))
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(TaxExemption {
                id: parse_uuid_row(&row.get::<_, String>(0)?, "tax_exemption", "id")?,
                customer_id: parse_uuid_row(
                    &row.get::<_, String>(1)?,
                    "tax_exemption",
                    "customer_id",
                )?,
                exemption_type: parse_enum_row(
                    &row.get::<_, String>(2)?,
                    "tax_exemption",
                    "exemption_type",
                )?,
                certificate_number: row.get(3)?,
                issuing_authority: row.get(4)?,
                jurisdiction_ids,
                exempt_categories,
                effective_from: parse_date_row(
                    &row.get::<_, String>(7)?,
                    "tax_exemption",
                    "effective_from",
                )?,
                expires_at: Self::parse_date_opt(
                    row.get::<_, Option<String>>(8)?,
                    "tax_exemption",
                    "expires_at",
                )?,
                verified: row.get::<_, i32>(9)? != 0,
                verified_at: parse_datetime_opt_row(
                    row.get::<_, Option<String>>(10)?,
                    "tax_exemption",
                    "verified_at",
                )?,
                notes: row.get(11)?,
                active: row.get::<_, i32>(12)? != 0,
                created_at: parse_datetime_row(
                    &row.get::<_, String>(13)?,
                    "tax_exemption",
                    "created_at",
                )?,
                updated_at: parse_datetime_row(
                    &row.get::<_, String>(14)?,
                    "tax_exemption",
                    "updated_at",
                )?,
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
            &input
                .exempt_categories
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
        ).map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "INSERT INTO tax_exemptions (id, customer_id, exemption_type, certificate_number, issuing_authority, jurisdiction_ids, exempt_categories, effective_from, expires_at, verified, notes, active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, 1, ?, ?)",
            params![
                id.to_string(),
                input.customer_id.to_string(),
                input.exemption_type.to_string(),
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
                let origin_address_json: Option<String> = row.get(7)?;
                let origin_address: Option<TaxAddress> =
                    parse_json_opt_row(origin_address_json, "tax_settings", "origin_address")?;

                let id_str: String = row.get(0)?;
                let id = if id_str == "default" {
                    Uuid::nil()
                } else {
                    parse_uuid_row(&id_str, "tax_settings", "id")?
                };

                Ok(TaxSettings {
                    id,
                    enabled: row.get::<_, i32>(1)? != 0,
                    calculation_method: parse_enum_row(
                        &row.get::<_, String>(2)?,
                        "tax_settings",
                        "calculation_method",
                    )?,
                    compound_method: parse_enum_row(
                        &row.get::<_, String>(3)?,
                        "tax_settings",
                        "compound_method",
                    )?,
                    tax_shipping: row.get::<_, i32>(4)? != 0,
                    tax_handling: row.get::<_, i32>(5)? != 0,
                    tax_gift_wrap: row.get::<_, i32>(6)? != 0,
                    origin_address,
                    default_product_category: parse_enum_row(
                        &row.get::<_, String>(8)?,
                        "tax_settings",
                        "default_product_category",
                    )?,
                    rounding_mode: row.get(9)?,
                    decimal_places: row.get(10)?,
                    validate_addresses: row.get::<_, i32>(11)? != 0,
                    tax_provider: row.get(12)?,
                    provider_credentials: row.get(13)?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(14)?,
                        "tax_settings",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(15)?,
                        "tax_settings",
                        "updated_at",
                    )?,
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
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let calc_method = settings.calculation_method.to_string();
        let compound_method = settings.compound_method.to_string();

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
                settings.default_product_category.to_string(),
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
            let mut line_amount = item.unit_price * item.quantity - item.discount_amount;
            if line_amount < Decimal::ZERO {
                line_amount = Decimal::ZERO;
            }
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

            for rate in &rates {
                if line_amount <= Decimal::ZERO {
                    continue;
                }

                if let Some(min) = rate.threshold_min {
                    if line_amount < min {
                        continue;
                    }
                }

                let capped_base = match rate.threshold_max {
                    Some(max) if line_amount > max => max,
                    _ => line_amount,
                };

                if capped_base <= Decimal::ZERO {
                    continue;
                }

                let taxable_amount = if rate.fixed_amount.is_some() {
                    capped_base
                } else if rate.is_compound {
                    // Compound tax is applied on (subtotal + previous taxes)
                    capped_base + line_tax
                } else {
                    capped_base
                };

                let rate_tax = if let Some(fixed) = rate.fixed_amount {
                    fixed
                } else {
                    taxable_amount * rate.rate
                };

                line_tax += rate_tax;

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
                        summary.total_tax += rate_tax;
                    }

                    // Add to breakdown
                    if let Some(existing) = tax_breakdown.iter_mut().find(|b| b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type) {
                        existing.taxable_amount += taxable_amount;
                        existing.tax_amount += rate_tax;
                    } else {
                        tax_breakdown.push(TaxBreakdown {
                            jurisdiction_id: jurisdiction.id,
                            jurisdiction_name: jurisdiction.name.clone(),
                            tax_type: rate.tax_type,
                            rate_name: rate.name.clone(),
                            rate: rate.rate,
                            taxable_amount,
                            tax_amount: rate_tax,
                            is_compound: rate.is_compound,
                        });
                    }

                    line_tax_details.push(TaxDetail {
                        tax_type: rate.tax_type,
                        jurisdiction_name: jurisdiction.name,
                        rate: rate.rate,
                        amount: rate_tax,
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
            if let Some(mut shipping_amount) = request.shipping_amount {
                if shipping_amount < Decimal::ZERO {
                    shipping_amount = Decimal::ZERO;
                }

                let shipping_rates = self.get_rates_for_address(&request.shipping_address, ProductTaxCategory::Standard, transaction_date)?;
                for rate in &shipping_rates {
                    if shipping_amount <= Decimal::ZERO {
                        continue;
                    }

                    if let Some(min) = rate.threshold_min {
                        if shipping_amount < min {
                            continue;
                        }
                    }

                    let capped_base = match rate.threshold_max {
                        Some(max) if shipping_amount > max => max,
                        _ => shipping_amount,
                    };

                    if capped_base <= Decimal::ZERO {
                        continue;
                    }

                    let taxable_amount = if rate.fixed_amount.is_some() {
                        capped_base
                    } else if rate.is_compound {
                        capped_base + shipping_tax
                    } else {
                        capped_base
                    };

                    let rate_tax = if let Some(fixed) = rate.fixed_amount {
                        fixed
                    } else {
                        taxable_amount * rate.rate
                    };

                    shipping_tax += rate_tax;

                    if let Some(jurisdiction) = self.get_jurisdiction(rate.jurisdiction_id)? {
                        jurisdictions_map
                            .entry(jurisdiction.id)
                            .or_insert_with(|| JurisdictionSummary {
                                id: jurisdiction.id,
                                name: jurisdiction.name.clone(),
                                code: jurisdiction.code.clone(),
                                level: jurisdiction.level,
                                total_rate: Decimal::ZERO,
                                total_tax: Decimal::ZERO,
                            });

                        if let Some(summary) = jurisdictions_map.get_mut(&jurisdiction.id) {
                            summary.total_rate += rate.rate;
                            summary.total_tax += rate_tax;
                        }

                        if let Some(existing) = tax_breakdown.iter_mut().find(|b| b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type) {
                            existing.taxable_amount += taxable_amount;
                            existing.tax_amount += rate_tax;
                        } else {
                            tax_breakdown.push(TaxBreakdown {
                                jurisdiction_id: jurisdiction.id,
                                jurisdiction_name: jurisdiction.name.clone(),
                                tax_type: rate.tax_type,
                                rate_name: rate.name.clone(),
                                rate: rate.rate,
                                taxable_amount,
                                tax_amount: rate_tax,
                                is_compound: rate.is_compound,
                            });
                        }
                    }
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
            .map(serde_json::to_string)
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

impl TaxRepository for SqliteTaxRepository {
    fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        SqliteTaxRepository::create_jurisdiction(self, input)
    }

    fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        SqliteTaxRepository::get_jurisdiction(self, id)
    }

    fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        SqliteTaxRepository::get_jurisdiction_by_code(self, code)
    }

    fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        SqliteTaxRepository::list_jurisdictions(self, filter)
    }

    fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        SqliteTaxRepository::create_rate(self, input)
    }

    fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        SqliteTaxRepository::get_rate(self, id)
    }

    fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        SqliteTaxRepository::list_rates(self, filter)
    }

    fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        SqliteTaxRepository::get_rates_for_address(self, address, category, date)
    }

    fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        SqliteTaxRepository::create_exemption(self, input)
    }

    fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        SqliteTaxRepository::get_exemption(self, id)
    }

    fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        SqliteTaxRepository::get_customer_exemptions(self, customer_id)
    }

    fn get_settings(&self) -> Result<TaxSettings> {
        SqliteTaxRepository::get_settings(self)
    }

    fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        SqliteTaxRepository::update_settings(self, settings)
    }

    fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        SqliteTaxRepository::calculate_tax(self, request)
    }

    fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()> {
        SqliteTaxRepository::save_calculation(
            self,
            result,
            order_id,
            cart_id,
            customer_id,
            address,
            currency,
        )
    }
}
