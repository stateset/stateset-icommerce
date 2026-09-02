//! SQLite implementation of tax repository

use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use stateset_core::{
    CommerceError, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ProductTaxCategory,
    ResolvedTaxRate, Result, TaxAddress, TaxCalculationRequest, TaxCalculationResult,
    TaxComputationInputs, TaxExemption, TaxJurisdiction, TaxJurisdictionFilter, TaxRate,
    TaxRateFilter, TaxRepository, TaxSettings, compute_tax,
};
use std::collections::HashMap;
use uuid::Uuid;

use super::{
    map_db_error, parse_date_row, parse_datetime_opt_row, parse_datetime_row,
    parse_decimal_opt_row, parse_decimal_row, parse_enum_row, parse_json_opt_row, parse_json_row,
    parse_uuid_opt_row, parse_uuid_row,
};

/// Upper-case, trimmed form of a jurisdiction/country/state code — the
/// canonical stored form and the form every lookup compares against.
fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

/// SQLite tax repository
#[derive(Debug)]
pub struct SqliteTaxRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteTaxRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
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
             FROM tax_jurisdictions WHERE UPPER(code) = ?",
            params![normalize_code(code)],
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
    pub fn list_jurisdictions(
        &self,
        filter: TaxJurisdictionFilter,
    ) -> Result<Vec<TaxJurisdiction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE 1=1"
        );
        let mut params_vec: Vec<String> = Vec::new();

        // Codes are stored upper-case, but compare case-insensitively so
        // rows written before normalisation still match ("us" == "US").
        if let Some(country) = &filter.country_code {
            query.push_str(" AND UPPER(country_code) = ?");
            params_vec.push(normalize_code(country));
        }

        if let Some(state) = &filter.state_code {
            query.push_str(" AND UPPER(state_code) = ?");
            params_vec.push(normalize_code(state));
        }

        if let Some(level) = &filter.level {
            query.push_str(" AND level = ?");
            params_vec.push(level.to_string());
        }

        if filter.active_only {
            query.push_str(" AND active = 1");
        }

        // Deterministic, backend-identical order (COALESCE so a NULL state_code
        // sorts consistently with the Postgres backend). Must match Postgres.
        query.push_str(" ORDER BY country_code, COALESCE(state_code, ''), level, name");
        crate::sqlite::append_limit_offset(&mut query, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params.as_slice(), |row| {
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
            })
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Create a new jurisdiction. `code`, `country_code` and `state_code`
    /// are normalised to upper case so lookups are case-insensitive.
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
                normalize_code(&input.code),
                input.level.to_string(),
                normalize_code(&input.country_code),
                input.state_code.as_deref().map(normalize_code),
                input.county,
                input.city,
                postal_codes_json,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        drop(conn);

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
            query.push_str(
                " AND effective_from <= ? AND (effective_to IS NULL OR effective_to >= ?)",
            );
            params_vec.push(date.to_string());
            params_vec.push(date.to_string());
        }

        query.push_str(" ORDER BY priority, name");
        crate::sqlite::append_limit_offset(&mut query, filter.limit, filter.offset);

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params.as_slice(), |row| {
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
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(15)?,
                        "tax_rate",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(16)?,
                        "tax_rate",
                        "updated_at",
                    )?,
                })
            })
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Get rates for a jurisdiction and product category
    pub fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
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
                i32::from(input.is_compound),
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

        drop(conn);

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

    /// Get all active exemptions for a customer, whatever their verification
    /// state or validity window. Tax calculation honours only those that are
    /// [`TaxExemption::is_effective_on`] the transaction date (active,
    /// verified, inside the window).
    pub fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority, jurisdiction_ids, exempt_categories, effective_from, expires_at, verified, verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions
             WHERE customer_id = ? AND active = 1
             ORDER BY effective_from, id"
        ).map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![customer_id.to_string()], |row| {
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
            })
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Mark an exemption certificate as verified (or revoke verification).
    /// Only verified exemptions are honoured by tax calculation.
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::NotFound`] if no exemption has `id`.
    pub fn verify_exemption(&self, id: Uuid, verified: bool) -> Result<TaxExemption> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();
        let verified_at = if verified { Some(now.to_rfc3339()) } else { None };
        let changed = conn
            .execute(
                "UPDATE tax_exemptions SET verified = ?1, verified_at = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![i32::from(verified), verified_at, now.to_rfc3339(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if changed == 0 {
            return Err(CommerceError::NotFound);
        }
        self.get_exemption(id)?.ok_or(CommerceError::NotFound)
    }

    /// Create a new exemption. It is created **unverified** and is not
    /// honoured by tax calculation until [`Self::verify_exemption`] is called.
    pub fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let jurisdiction_ids_json = serde_json::to_string(
            &input
                .jurisdiction_ids
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let categories_json = serde_json::to_string(
            &input
                .exempt_categories
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

        drop(conn);

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

        let origin_address_json = settings
            .origin_address
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
                i32::from(settings.enabled),
                calc_method,
                compound_method,
                i32::from(settings.tax_shipping),
                i32::from(settings.tax_handling),
                i32::from(settings.tax_gift_wrap),
                origin_address_json,
                settings.default_product_category.to_string(),
                settings.rounding_mode,
                settings.decimal_places,
                i32::from(settings.validate_addresses),
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
    /// Calculate tax for a request.
    ///
    /// This backend only resolves the data — settings, the customer's
    /// exemptions, and the rates (with their jurisdictions) applicable to the
    /// shipping address for every product category the request touches —
    /// and hands it to the shared, pure [`stateset_core::compute_tax`], so
    /// SQLite and Postgres produce identical results for identical inputs.
    pub fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        let now = Utc::now();
        let inputs = self.computation_inputs(&request, now)?;
        Ok(compute_tax(&request, &inputs, now))
    }

    /// Load everything [`stateset_core::compute_tax`] needs for `request`.
    fn computation_inputs(
        &self,
        request: &TaxCalculationRequest,
        now: chrono::DateTime<Utc>,
    ) -> Result<TaxComputationInputs> {
        let settings = self.get_settings()?;
        let transaction_date = request.transaction_date.unwrap_or_else(|| now.date_naive());

        let exemptions = match request.customer_id {
            Some(customer_id) => self.get_customer_exemptions(customer_id)?,
            None => Vec::new(),
        };

        let mut categories: Vec<ProductTaxCategory> = request
            .line_items
            .iter()
            .map(|item| item.tax_category)
            .filter(|category| *category != ProductTaxCategory::Exempt)
            .collect();
        if settings.tax_shipping && request.shipping_amount.is_some() {
            categories.push(ProductTaxCategory::Standard);
        }
        categories.sort_by_key(|c| c.as_str());
        categories.dedup();

        let mut jurisdictions: HashMap<Uuid, TaxJurisdiction> = HashMap::new();
        let mut rates = Vec::new();
        for category in categories {
            for rate in
                self.get_rates_for_address(&request.shipping_address, category, transaction_date)?
            {
                let jurisdiction = match jurisdictions.get(&rate.jurisdiction_id) {
                    Some(jurisdiction) => jurisdiction.clone(),
                    None => {
                        let Some(jurisdiction) = self.get_jurisdiction(rate.jurisdiction_id)?
                        else {
                            continue;
                        };
                        jurisdictions.insert(jurisdiction.id, jurisdiction.clone());
                        jurisdiction
                    }
                };
                rates.push(ResolvedTaxRate { rate, jurisdiction });
            }
        }

        Ok(TaxComputationInputs { settings, rates, exemptions })
    }

    /// Save a tax calculation to the database
    pub fn save_calculation(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let address_json = serde_json::to_string(address)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let line_items_json = serde_json::to_string(&result.line_item_taxes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let breakdown_json = serde_json::to_string(&result.tax_breakdown)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let exemption_json = result
            .exemption_details
            .as_ref()
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
                i32::from(result.exemptions_applied),
                exemption_json,
                i32::from(result.is_estimate),
                result.calculated_at.to_rfc3339()
            ],
        ).map_err(map_db_error)?;

        Ok(())
    }
}

impl TaxRepository for SqliteTaxRepository {
    fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        Self::create_jurisdiction(self, input)
    }

    fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        Self::get_jurisdiction(self, id)
    }

    fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        Self::get_jurisdiction_by_code(self, code)
    }

    fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        Self::list_jurisdictions(self, filter)
    }

    fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        Self::create_rate(self, input)
    }

    fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        Self::get_rate(self, id)
    }

    fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        Self::list_rates(self, filter)
    }

    fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        Self::get_rates_for_address(self, address, category, date)
    }

    fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        Self::create_exemption(self, input)
    }

    fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        Self::get_exemption(self, id)
    }

    fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        Self::get_customer_exemptions(self, customer_id)
    }

    fn verify_exemption(&self, id: Uuid, verified: bool) -> Result<TaxExemption> {
        Self::verify_exemption(self, id, verified)
    }

    fn get_settings(&self) -> Result<TaxSettings> {
        Self::get_settings(self)
    }

    fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        Self::update_settings(self, settings)
    }

    fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        Self::calculate_tax(self, request)
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
        Self::save_calculation(self, result, order_id, cart_id, customer_id, address, currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, CurrencyCode, ExemptionType,
        JurisdictionLevel, ProductTaxCategory, TaxAddress, TaxCalculationRequest,
        TaxJurisdictionFilter, TaxLineItem, TaxRateFilter, TaxType,
    };

    fn fresh_repo() -> SqliteTaxRepository {
        SqliteDatabase::in_memory().expect("in-memory").tax()
    }

    fn make_state_jur(repo: &SqliteTaxRepository, state: &str) -> TaxJurisdiction {
        // Use ZZ-* prefix so we don't collide with the 50 pre-seeded US states.
        repo.create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: format!("{state} Test State"),
            code: format!("ZZ-{state}"),
            level: JurisdictionLevel::State,
            country_code: "ZZ".into(),
            state_code: Some(state.into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .expect("create jurisdiction")
    }

    fn make_rate(repo: &SqliteTaxRepository, jur_id: Uuid, rate: Decimal) -> TaxRate {
        repo.create_rate(CreateTaxRate {
            jurisdiction_id: jur_id,
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate,
            name: "Sales Tax".into(),
            description: None,
            is_compound: false,
            priority: 1,
            threshold_min: None,
            threshold_max: None,
            fixed_amount: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            effective_to: None,
        })
        .expect("create rate")
    }

    #[test]
    fn create_jurisdiction_round_trips() {
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "CA");
        assert_eq!(j.code, "ZZ-CA");
        assert_eq!(j.level, JurisdictionLevel::State);

        let by_id = repo.get_jurisdiction(j.id).expect("ok").expect("found");
        assert_eq!(by_id.id, j.id);
        let by_code = repo.get_jurisdiction_by_code("ZZ-CA").expect("ok").expect("found");
        assert_eq!(by_code.id, j.id);
        assert!(repo.get_jurisdiction_by_code("missing-xyz").expect("ok").is_none());
    }

    #[test]
    fn list_jurisdictions_applies_limit_and_offset() {
        let repo = fresh_repo();
        make_state_jur(&repo, "AA");
        make_state_jur(&repo, "BB");
        make_state_jur(&repo, "CC");

        let base =
            || TaxJurisdictionFilter { country_code: Some("ZZ".into()), ..Default::default() };
        let all = repo.list_jurisdictions(base()).expect("list all");
        assert_eq!(all.len(), 3);

        let page = repo
            .list_jurisdictions(TaxJurisdictionFilter { limit: Some(2), ..base() })
            .expect("limited");
        assert_eq!(page.len(), 2, "limit must bound the result set");
        assert_eq!(page[0].id, all[0].id);

        let rest = repo
            .list_jurisdictions(TaxJurisdictionFilter { limit: Some(2), offset: Some(2), ..base() })
            .expect("offset");
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].id, all[2].id);
    }

    #[test]
    fn list_rates_applies_limit_and_offset() {
        let repo = fresh_repo();
        let jur = make_state_jur(&repo, "LR");
        for _ in 0..3 {
            make_rate(&repo, jur.id, dec!(0.05));
        }

        let base = || TaxRateFilter { jurisdiction_id: Some(jur.id), ..Default::default() };
        let all = repo.list_rates(base()).expect("list all");
        assert_eq!(all.len(), 3);

        let page = repo.list_rates(TaxRateFilter { limit: Some(2), ..base() }).expect("limited");
        assert_eq!(page.len(), 2, "limit must bound the result set");

        let rest = repo
            .list_rates(TaxRateFilter { limit: Some(2), offset: Some(2), ..base() })
            .expect("offset");
        assert_eq!(rest.len(), 1);
    }

    #[test]
    fn list_jurisdictions_filters_by_country_and_state() {
        let repo = fresh_repo();
        make_state_jur(&repo, "AA");
        make_state_jur(&repo, "BB");
        repo.create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "Test BC".into(),
            code: "YY-BC".into(),
            level: JurisdictionLevel::State,
            country_code: "YY".into(),
            state_code: Some("BC".into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .expect("yy-bc");

        let zz = repo
            .list_jurisdictions(TaxJurisdictionFilter {
                country_code: Some("ZZ".into()),
                ..Default::default()
            })
            .expect("zz");
        assert_eq!(zz.len(), 2);

        let zz_aa = repo
            .list_jurisdictions(TaxJurisdictionFilter {
                country_code: Some("ZZ".into()),
                state_code: Some("AA".into()),
                ..Default::default()
            })
            .expect("aa");
        assert_eq!(zz_aa.len(), 1);
        assert_eq!(zz_aa[0].state_code.as_deref(), Some("AA"));
    }

    #[test]
    fn list_jurisdictions_orders_by_country_then_state() {
        let repo = fresh_repo();
        // Two jurisdictions in different countries where ordering by country and
        // ordering by name DISAGREE, so the sort key is observable. Postgres used
        // to order only by (level, name), which would put XB (Apple) first.
        repo.create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "Zebra".into(),
            code: "XA-1".into(),
            level: JurisdictionLevel::State,
            country_code: "XA".into(),
            state_code: Some("X1".into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .expect("xa");
        repo.create_jurisdiction(CreateTaxJurisdiction {
            parent_id: None,
            name: "Apple".into(),
            code: "XB-1".into(),
            level: JurisdictionLevel::State,
            country_code: "XB".into(),
            state_code: Some("X2".into()),
            county: None,
            city: None,
            postal_codes: vec![],
        })
        .expect("xb");

        let all = repo.list_jurisdictions(TaxJurisdictionFilter::default()).expect("list");
        let mine: Vec<&str> = all
            .iter()
            .filter(|j| j.country_code == "XA" || j.country_code == "XB")
            .map(|j| j.country_code.as_str())
            .collect();
        assert_eq!(mine, vec!["XA", "XB"], "must order by country_code, not by name");
    }

    #[test]
    fn create_rate_round_trips() {
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "CA");
        let r = make_rate(&repo, j.id, dec!(0.0725));
        assert_eq!(r.rate, dec!(0.0725));
        let by_id = repo.get_rate(r.id).expect("ok").expect("found");
        assert_eq!(by_id.id, r.id);
    }

    fn single_item_request(
        country: &str,
        state: &str,
        unit_price: Decimal,
    ) -> TaxCalculationRequest {
        TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "line-1".into(),
                sku: None,
                product_id: None,
                quantity: dec!(1),
                unit_price,
                discount_amount: Decimal::ZERO,
                tax_category: ProductTaxCategory::Standard,
                tax_code: None,
                description: None,
            }],
            shipping_address: TaxAddress {
                line1: None,
                line2: None,
                city: None,
                state: Some(state.into()),
                postal_code: None,
                country: country.into(),
            },
            billing_address: None,
            customer_id: None,
            shipping_amount: None,
            currency: CurrencyCode::USD,
            transaction_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
            prices_include_tax: false,
        }
    }

    #[test]
    fn calculate_tax_honors_configured_rounding_mode() {
        // A $2.50 line at 5% yields exactly $0.125 of tax — a rounding midpoint.
        // half_up rounds it to 0.13; half_even (banker's, required by some
        // jurisdictions) rounds to 0.12 because the retained digit (2) is even.
        // The configured rounding_mode must actually change the result.
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "CA");
        make_rate(&repo, j.id, dec!(0.05));
        let request = single_item_request("ZZ", "CA", dec!(2.50));

        let mut settings = repo.get_settings().expect("settings");
        settings.rounding_mode = "half_even".into();
        repo.update_settings(settings).expect("update settings");
        let even = repo.calculate_tax(request.clone()).expect("calc");
        assert_eq!(
            even.total_tax,
            dec!(0.12),
            "half_even must round $0.125 down to 0.12 (retained digit is even): {even:?}"
        );

        let mut settings = repo.get_settings().expect("settings");
        settings.rounding_mode = "half_up".into();
        repo.update_settings(settings).expect("update settings");
        let up = repo.calculate_tax(request).expect("calc");
        assert_eq!(up.total_tax, dec!(0.13), "half_up must round $0.125 up to 0.13: {up:?}");
    }

    #[test]
    fn list_rates_filters_by_jurisdiction() {
        let repo = fresh_repo();
        let j_a = make_state_jur(&repo, "AA");
        let j_b = make_state_jur(&repo, "BB");
        make_rate(&repo, j_a.id, dec!(0.0725));
        make_rate(&repo, j_a.id, dec!(0.01));
        make_rate(&repo, j_b.id, dec!(0.04));

        let a_rates = repo
            .list_rates(TaxRateFilter { jurisdiction_id: Some(j_a.id), ..Default::default() })
            .expect("a");
        assert_eq!(a_rates.len(), 2);
    }

    #[test]
    fn calculate_tax_returns_jurisdictions_in_stable_order() {
        // The jurisdictions summary must be emitted in a stable, backend-agnostic
        // order (by code) rather than the tax engine's internal HashMap order,
        // so the same input always produces the same result.
        let repo = fresh_repo();
        // Country-level ZZ plus state-level ZZ-CA, each taxing the item.
        let country = repo
            .create_jurisdiction(CreateTaxJurisdiction {
                parent_id: None,
                name: "ZZ Country".into(),
                code: "ZZ".into(),
                level: JurisdictionLevel::Country,
                country_code: "ZZ".into(),
                state_code: None,
                county: None,
                city: None,
                postal_codes: vec![],
            })
            .expect("create country");
        make_rate(&repo, country.id, dec!(0.05));
        let state = make_state_jur(&repo, "CA"); // code ZZ-CA
        make_rate(&repo, state.id, dec!(0.03));

        let result =
            repo.calculate_tax(single_item_request("ZZ", "CA", dec!(100.00))).expect("calc");

        let codes: Vec<&str> = result.jurisdictions.iter().map(|j| j.code.as_str()).collect();
        assert_eq!(
            codes,
            vec!["ZZ", "ZZ-CA"],
            "jurisdictions must be returned in stable code order: {codes:?}"
        );
        assert!(
            result.jurisdictions.windows(2).all(|w| w[0].code <= w[1].code),
            "jurisdictions must be sorted by code"
        );
    }

    #[test]
    fn create_exemption_requires_existing_customer() {
        // tax_exemptions has a FK to customers — creating against a nonexistent
        // customer must surface a clear validation error rather than silently
        // inserting an orphan row.
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "AA");
        let result = repo.create_exemption(CreateTaxExemption {
            customer_id: Uuid::new_v4(),
            exemption_type: ExemptionType::Resale,
            certificate_number: Some("RES-12345".into()),
            issuing_authority: None,
            jurisdiction_ids: vec![j.id],
            exempt_categories: vec![ProductTaxCategory::Standard],
            effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            expires_at: Some(NaiveDate::from_ymd_opt(2027, 1, 1).expect("date")),
            notes: None,
        });
        assert!(result.is_err(), "expected FK rejection for unknown customer");
    }

    #[test]
    fn get_customer_exemptions_unknown_customer_is_empty() {
        let repo = fresh_repo();
        let exemptions = repo.get_customer_exemptions(Uuid::new_v4()).expect("ok");
        assert!(exemptions.is_empty());
    }

    #[test]
    fn get_settings_returns_defaults() {
        let repo = fresh_repo();
        let _ = repo.get_settings().expect("settings");
    }

    #[test]
    fn get_jurisdiction_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_jurisdiction(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn get_rate_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_rate(Uuid::new_v4()).expect("ok").is_none());
    }

    #[test]
    fn get_exemption_unknown_id_returns_none() {
        let repo = fresh_repo();
        assert!(repo.get_exemption(Uuid::new_v4()).expect("ok").is_none());
    }

    // ------------------------------------------------------------------
    // Engine wiring: rounding, inclusive pricing, exemptions, case-insensitive
    // jurisdictions. Postgres mirrors live in tests/postgres_tax_engine.rs.
    // ------------------------------------------------------------------

    fn fresh_db() -> SqliteDatabase {
        SqliteDatabase::in_memory().expect("in-memory")
    }

    fn make_customer(db: &SqliteDatabase) -> Uuid {
        use stateset_core::{CreateCustomer, CustomerRepository};
        db.customers()
            .create(CreateCustomer {
                email: format!("tax-{}@example.com", Uuid::new_v4()),
                first_name: "Tax".into(),
                last_name: "Payer".into(),
                phone: None,
                accepts_marketing: None,
                tags: None,
                metadata: None,
            })
            .expect("create customer")
            .id
            .into_uuid()
    }

    fn make_exemption(
        repo: &SqliteTaxRepository,
        customer_id: Uuid,
        jurisdiction_ids: Vec<Uuid>,
        from: (i32, u32, u32),
        to: Option<(i32, u32, u32)>,
    ) -> TaxExemption {
        repo.create_exemption(CreateTaxExemption {
            customer_id,
            exemption_type: ExemptionType::Resale,
            certificate_number: Some("RES-1".into()),
            issuing_authority: None,
            jurisdiction_ids,
            exempt_categories: vec![],
            effective_from: NaiveDate::from_ymd_opt(from.0, from.1, from.2).expect("date"),
            expires_at: to.map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).expect("date")),
            notes: None,
        })
        .expect("create exemption")
    }

    fn multi_item_request(country: &str, state: &str, prices: &[Decimal]) -> TaxCalculationRequest {
        let mut request = single_item_request(country, state, prices[0]);
        request.line_items = prices
            .iter()
            .enumerate()
            .map(|(i, price)| TaxLineItem {
                id: format!("line-{i}"),
                unit_price: *price,
                ..Default::default()
            })
            .collect();
        request
    }

    #[test]
    fn calculate_tax_rounds_per_line_and_sums_exactly() {
        // 3 × $1.11 @ 8.25% → each line 0.09, total 0.27 (= Σ lines).
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "RL");
        make_rate(&repo, j.id, dec!(0.0825));
        let mut request = multi_item_request("ZZ", "RL", &[dec!(1.11), dec!(1.11), dec!(1.11)]);
        request.shipping_amount = Some(dec!(4.99));
        let res = repo.calculate_tax(request).expect("calc");
        let lines: Vec<Decimal> = res.line_item_taxes.iter().map(|l| l.tax_amount).collect();
        assert_eq!(lines, vec![dec!(0.09), dec!(0.09), dec!(0.09)], "{res:?}");
        assert_eq!(res.shipping_tax, dec!(0.41)); // 4.99 × 0.0825 = 0.411675
        assert_eq!(res.total_tax, dec!(0.68));
        assert_eq!(lines.iter().sum::<Decimal>() + res.shipping_tax, res.total_tax);
        assert_eq!(res.tax_breakdown.iter().map(|b| b.tax_amount).sum::<Decimal>(), res.total_tax);
        assert_eq!(res.total, dec!(3.33) + dec!(4.99) + dec!(0.68));
    }

    #[test]
    fn calculate_tax_backs_out_inclusive_prices() {
        // €19.99 gross @ 19% → tax 3.19, net 16.80, customer pays 19.99.
        let repo = fresh_repo();
        let j = make_state_jur(&repo, "IN");
        make_rate(&repo, j.id, dec!(0.19));
        let mut request = single_item_request("ZZ", "IN", dec!(19.99));
        request.currency = CurrencyCode::EUR;
        request.prices_include_tax = true;
        let res = repo.calculate_tax(request.clone()).expect("calc");
        assert_eq!(res.total_tax, dec!(3.19), "{res:?}");
        assert_eq!(res.subtotal, dec!(16.80));
        assert_eq!(res.total, dec!(19.99));

        // The store-level calculation_method = inclusive has the same effect.
        request.prices_include_tax = false;
        let mut settings = repo.get_settings().expect("settings");
        settings.calculation_method = stateset_core::TaxCalculationMethod::Inclusive;
        repo.update_settings(settings).expect("update settings");
        let res = repo.calculate_tax(request).expect("calc");
        assert_eq!(res.total_tax, dec!(3.19));
        assert_eq!(res.total, dec!(19.99));
    }

    #[test]
    fn calculate_tax_ignores_unverified_exemption() {
        let db = fresh_db();
        let repo = db.tax();
        let j = make_state_jur(&repo, "UV");
        make_rate(&repo, j.id, dec!(0.05));
        let customer = make_customer(&db);
        let exemption = make_exemption(&repo, customer, vec![], (2026, 1, 1), None);
        assert!(!exemption.verified, "exemptions are created unverified");

        let mut request = single_item_request("ZZ", "UV", dec!(100));
        request.customer_id = Some(customer);
        let res = repo.calculate_tax(request.clone()).expect("calc");
        assert_eq!(res.total_tax, dec!(5.00), "unverified exemption must not apply: {res:?}");
        assert!(!res.exemptions_applied);

        let verified = repo.verify_exemption(exemption.id, true).expect("verify");
        assert!(verified.verified && verified.verified_at.is_some());
        let res = repo.calculate_tax(request).expect("calc");
        assert_eq!(res.total_tax, Decimal::ZERO);
        assert!(res.exemptions_applied);
        assert_eq!(res.exemption_details.expect("details").tax_saved, dec!(5.00));
    }

    #[test]
    fn calculate_tax_ignores_exemption_outside_window_at_transaction_date() {
        let db = fresh_db();
        let repo = db.tax();
        let j = make_state_jur(&repo, "WN");
        make_rate(&repo, j.id, dec!(0.05));
        let customer = make_customer(&db);
        // Window starts after the request's transaction_date (2026-06-01) but
        // is current "today" — the transaction date must win.
        let exemption = make_exemption(&repo, customer, vec![], (2026, 7, 1), None);
        repo.verify_exemption(exemption.id, true).expect("verify");

        let mut request = single_item_request("ZZ", "WN", dec!(100));
        request.customer_id = Some(customer);
        let res = repo.calculate_tax(request.clone()).expect("calc");
        assert_eq!(res.total_tax, dec!(5.00), "not yet effective on 2026-06-01: {res:?}");

        request.transaction_date = Some(NaiveDate::from_ymd_opt(2026, 7, 15).expect("date"));
        let res = repo.calculate_tax(request).expect("calc");
        assert_eq!(res.total_tax, Decimal::ZERO, "effective on 2026-07-15: {res:?}");

        // get_customer_exemptions returns every active exemption; the engine
        // does the date/verification filtering.
        let listed = repo.get_customer_exemptions(customer).expect("list");
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn calculate_tax_applies_jurisdiction_scoped_exemption_only_there() {
        let db = fresh_db();
        let repo = db.tax();
        let country = repo
            .create_jurisdiction(CreateTaxJurisdiction {
                parent_id: None,
                name: "ZZ Country".into(),
                code: "ZZ".into(),
                level: JurisdictionLevel::Country,
                country_code: "ZZ".into(),
                state_code: None,
                county: None,
                city: None,
                postal_codes: vec![],
            })
            .expect("country");
        make_rate(&repo, country.id, dec!(0.05));
        let state = make_state_jur(&repo, "JS");
        make_rate(&repo, state.id, dec!(0.03));
        let customer = make_customer(&db);
        let exemption = make_exemption(&repo, customer, vec![state.id], (2026, 1, 1), None);
        repo.verify_exemption(exemption.id, true).expect("verify");

        let mut request = single_item_request("ZZ", "JS", dec!(100));
        request.customer_id = Some(customer);
        let res = repo.calculate_tax(request).expect("calc");
        assert_eq!(res.total_tax, dec!(5.00), "only the state rate is exempt: {res:?}");
        assert!(res.exemptions_applied);
        assert!(!res.line_item_taxes[0].is_exempt);
        assert_eq!(res.jurisdictions.iter().map(|j| j.code.as_str()).collect::<Vec<_>>(), ["ZZ"]);
    }

    #[test]
    fn jurisdiction_codes_are_case_insensitive() {
        let repo = fresh_repo();
        let created = repo
            .create_jurisdiction(CreateTaxJurisdiction {
                parent_id: None,
                name: "Lower".into(),
                code: "zz-lc".into(),
                level: JurisdictionLevel::State,
                country_code: "zz".into(),
                state_code: Some("lc".into()),
                county: None,
                city: None,
                postal_codes: vec![],
            })
            .expect("create");
        assert_eq!(created.code, "ZZ-LC", "stored upper-case");
        assert_eq!(created.country_code, "ZZ");
        assert_eq!(created.state_code.as_deref(), Some("LC"));

        for code in ["zz-lc", "ZZ-LC", " Zz-Lc "] {
            let found = repo.get_jurisdiction_by_code(code).expect("ok").expect("found");
            assert_eq!(found.id, created.id, "lookup by {code:?}");
        }
        let listed = repo
            .list_jurisdictions(TaxJurisdictionFilter {
                country_code: Some("zz".into()),
                state_code: Some("Lc".into()),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(listed.len(), 1);

        make_rate(&repo, created.id, dec!(0.10));
        for (country, state) in [("zz", "lc"), ("ZZ", "LC")] {
            let res =
                repo.calculate_tax(single_item_request(country, state, dec!(10))).expect("calc");
            assert_eq!(res.total_tax, dec!(1.00), "address {country}/{state} must resolve");
        }
    }
}
