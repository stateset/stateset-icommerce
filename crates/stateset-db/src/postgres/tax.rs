//! PostgreSQL implementation of tax repository

use super::map_db_error;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ExemptionType,
    JurisdictionLevel, ProductTaxCategory, ResolvedTaxRate, Result, TaxAddress,
    TaxCalculationMethod, TaxCalculationRequest, TaxCalculationResult, TaxCompoundMethod,
    TaxComputationInputs, TaxExemption, TaxJurisdiction, TaxJurisdictionFilter, TaxRate,
    TaxRateFilter, TaxRepository, TaxSettings, TaxType, compute_tax,
};
use uuid::Uuid;

/// Upper-case, trimmed form of a jurisdiction/country/state code — the
/// canonical stored form and the form every lookup compares against.
fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

/// How many active jurisdictions of a country are scanned when resolving the
/// local (below-state) ones for an address. Bounded so a country with a large
/// local-tax table can never turn one tax calculation into an unbounded scan;
/// this is the list endpoint's own maximum page size.
const LOCAL_JURISDICTION_SCAN_LIMIT: u32 = 1000;

/// PostgreSQL tax repository
#[derive(Debug, Clone)]
pub struct PgTaxRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct TaxJurisdictionRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    code: String,
    level: String,
    country_code: String,
    state_code: Option<String>,
    county: Option<String>,
    city: Option<String>,
    postal_codes: serde_json::Value,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct TaxRateRow {
    id: Uuid,
    jurisdiction_id: Uuid,
    tax_type: String,
    product_category: String,
    rate: Decimal,
    name: String,
    description: Option<String>,
    is_compound: bool,
    priority: i32,
    threshold_min: Option<Decimal>,
    threshold_max: Option<Decimal>,
    fixed_amount: Option<Decimal>,
    effective_from: NaiveDate,
    effective_to: Option<NaiveDate>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct TaxExemptionRow {
    id: Uuid,
    customer_id: Uuid,
    exemption_type: String,
    certificate_number: Option<String>,
    issuing_authority: Option<String>,
    jurisdiction_ids: serde_json::Value,
    exempt_categories: serde_json::Value,
    effective_from: NaiveDate,
    expires_at: Option<NaiveDate>,
    verified: bool,
    verified_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct TaxSettingsRow {
    id: String,
    enabled: bool,
    calculation_method: String,
    compound_method: String,
    tax_shipping: bool,
    tax_handling: bool,
    tax_gift_wrap: bool,
    origin_address: Option<serde_json::Value>,
    default_product_category: String,
    rounding_mode: String,
    decimal_places: i32,
    validate_addresses: bool,
    tax_provider: Option<String>,
    provider_credentials: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgTaxRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_jurisdiction(row: TaxJurisdictionRow) -> Result<TaxJurisdiction> {
        let TaxJurisdictionRow {
            id,
            parent_id,
            name,
            code,
            level,
            country_code,
            state_code,
            county,
            city,
            postal_codes,
            active,
            created_at,
            updated_at,
        } = row;

        let level: JurisdictionLevel = level.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid tax_jurisdiction.level '{}': {}",
                level, e
            ))
        })?;
        let postal_codes: Vec<String> = serde_json::from_value(postal_codes).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid JSON for tax_jurisdiction.postal_codes: {}",
                e
            ))
        })?;

        Ok(TaxJurisdiction {
            id,
            parent_id,
            name,
            code,
            level,
            country_code,
            state_code,
            county,
            city,
            postal_codes,
            active,
            created_at,
            updated_at,
        })
    }

    fn row_to_rate(row: TaxRateRow) -> Result<TaxRate> {
        let TaxRateRow {
            id,
            jurisdiction_id,
            tax_type,
            product_category,
            rate,
            name,
            description,
            is_compound,
            priority,
            threshold_min,
            threshold_max,
            fixed_amount,
            effective_from,
            effective_to,
            active,
            created_at,
            updated_at,
        } = row;

        let tax_type: TaxType = tax_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid tax_rate.tax_type '{}': {}", tax_type, e))
        })?;
        let product_category: ProductTaxCategory = product_category.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid tax_rate.product_category '{}': {}",
                product_category, e
            ))
        })?;

        Ok(TaxRate {
            id,
            jurisdiction_id,
            tax_type,
            product_category,
            rate,
            name,
            description,
            is_compound,
            priority,
            threshold_min,
            threshold_max,
            fixed_amount,
            effective_from,
            effective_to,
            active,
            created_at,
            updated_at,
        })
    }

    fn row_to_exemption(row: TaxExemptionRow) -> Result<TaxExemption> {
        let TaxExemptionRow {
            id,
            customer_id,
            exemption_type,
            certificate_number,
            issuing_authority,
            jurisdiction_ids,
            exempt_categories,
            effective_from,
            expires_at,
            verified,
            verified_at,
            notes,
            active,
            created_at,
            updated_at,
        } = row;

        let exemption_type: ExemptionType = exemption_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid tax_exemption.exemption_type '{}': {}",
                exemption_type, e
            ))
        })?;
        let raw_jurisdiction_ids: Vec<String> =
            serde_json::from_value(jurisdiction_ids).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for tax_exemption.jurisdiction_ids: {}",
                    e
                ))
            })?;
        let jurisdiction_ids = raw_jurisdiction_ids
            .into_iter()
            .map(|value| {
                value.parse().map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid tax_exemption.jurisdiction_ids value '{}': {}",
                        value, e
                    ))
                })
            })
            .collect::<Result<Vec<Uuid>>>()?;
        let raw_exempt_categories: Vec<String> = serde_json::from_value(exempt_categories)
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for tax_exemption.exempt_categories: {}",
                    e
                ))
            })?;
        let exempt_categories = raw_exempt_categories
            .into_iter()
            .map(|value| {
                value.parse().map_err(|e| {
                    CommerceError::DatabaseError(format!(
                        "Invalid tax_exemption.exempt_categories value '{}': {}",
                        value, e
                    ))
                })
            })
            .collect::<Result<Vec<ProductTaxCategory>>>()?;

        Ok(TaxExemption {
            id,
            customer_id,
            exemption_type,
            certificate_number,
            issuing_authority,
            jurisdiction_ids,
            exempt_categories,
            effective_from,
            expires_at,
            verified,
            verified_at,
            notes,
            active,
            created_at,
            updated_at,
        })
    }

    fn row_to_settings(row: TaxSettingsRow) -> Result<TaxSettings> {
        let TaxSettingsRow {
            id,
            enabled,
            calculation_method,
            compound_method,
            tax_shipping,
            tax_handling,
            tax_gift_wrap,
            origin_address,
            default_product_category,
            rounding_mode,
            decimal_places,
            validate_addresses,
            tax_provider,
            provider_credentials,
            created_at,
            updated_at,
        } = row;

        let id = if id == "default" {
            Uuid::nil()
        } else {
            id.parse().map_err(|e| {
                CommerceError::DatabaseError(format!("Invalid tax_settings.id '{}': {}", id, e))
            })?
        };
        let calculation_method: TaxCalculationMethod = calculation_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid tax_settings.calculation_method '{}': {}",
                calculation_method, e
            ))
        })?;
        let compound_method: TaxCompoundMethod = compound_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid tax_settings.compound_method '{}': {}",
                compound_method, e
            ))
        })?;
        let default_product_category: ProductTaxCategory =
            default_product_category.parse().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid tax_settings.default_product_category '{}': {}",
                    default_product_category, e
                ))
            })?;
        let origin_address =
            origin_address.map(serde_json::from_value).transpose().map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid JSON for tax_settings.origin_address: {}",
                    e
                ))
            })?;

        Ok(TaxSettings {
            id,
            enabled,
            calculation_method,
            compound_method,
            tax_shipping,
            tax_handling,
            tax_gift_wrap,
            origin_address,
            default_product_category,
            rounding_mode,
            decimal_places,
            validate_addresses,
            tax_provider,
            provider_credentials,
            created_at,
            updated_at,
        })
    }

    /// Create a new jurisdiction. `code`, `country_code` and `state_code`
    /// are normalised to upper case so lookups are case-insensitive.
    pub async fn create_jurisdiction_async(
        &self,
        input: CreateTaxJurisdiction,
    ) -> Result<TaxJurisdiction> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let postal_codes = serde_json::to_value(&input.postal_codes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO tax_jurisdictions (
                id, parent_id, name, code, level, country_code, state_code, county, city,
                postal_codes, active, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,true,$11,$12)
            "#,
        )
        .bind(id)
        .bind(input.parent_id)
        .bind(&input.name)
        .bind(normalize_code(&input.code))
        .bind(input.level.to_string())
        .bind(normalize_code(&input.country_code))
        .bind(input.state_code.as_deref().map(normalize_code))
        .bind(&input.county)
        .bind(&input.city)
        .bind(postal_codes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_jurisdiction_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_jurisdiction_async(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        let row = sqlx::query_as::<_, TaxJurisdictionRow>(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_jurisdiction).transpose()
    }

    pub async fn get_jurisdiction_by_code_async(
        &self,
        code: &str,
    ) -> Result<Option<TaxJurisdiction>> {
        let row = sqlx::query_as::<_, TaxJurisdictionRow>(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE UPPER(code) = $1",
        )
        .bind(normalize_code(code))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_jurisdiction).transpose()
    }

    pub async fn list_jurisdictions_async(
        &self,
        filter: TaxJurisdictionFilter,
    ) -> Result<Vec<TaxJurisdiction>> {
        let mut query = String::from(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE 1=1",
        );
        let mut param_idx = 1;

        // Codes are stored upper-case, but compare case-insensitively so
        // rows written before normalisation still match ("us" == "US").
        if filter.country_code.is_some() {
            query.push_str(&format!(" AND UPPER(country_code) = ${}", param_idx));
            param_idx += 1;
        }
        if filter.state_code.is_some() {
            query.push_str(&format!(" AND UPPER(state_code) = ${}", param_idx));
            param_idx += 1;
        }
        if filter.level.is_some() {
            query.push_str(&format!(" AND level = ${}", param_idx));
        }
        if filter.active_only {
            query.push_str(" AND active = true");
        }

        // Deterministic, backend-identical order: group by country then state
        // (COALESCE so a NULL state_code sorts consistently with SQLite), then
        // level and name. Must match the SQLite backend's ORDER BY.
        query.push_str(" ORDER BY country_code, COALESCE(state_code, ''), level, name");
        {
            let limit = super::effective_limit(filter.limit);
            let offset = i64::from(filter.offset.unwrap_or(0));
            query.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, TaxJurisdictionRow>(&query);

        if let Some(country) = &filter.country_code {
            q = q.bind(normalize_code(country));
        }
        if let Some(state) = &filter.state_code {
            q = q.bind(normalize_code(state));
        }
        if let Some(level) = &filter.level {
            q = q.bind(level.to_string());
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut jurisdictions = Vec::with_capacity(rows.len());
        for row in rows {
            jurisdictions.push(Self::row_to_jurisdiction(row)?);
        }
        Ok(jurisdictions)
    }

    pub async fn create_rate_async(&self, input: CreateTaxRate) -> Result<TaxRate> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let tax_type = input.tax_type.to_string();
        let category = input.product_category.to_string();

        sqlx::query(
            r#"
            INSERT INTO tax_rates (
                id, jurisdiction_id, tax_type, product_category, rate, name, description,
                is_compound, priority, threshold_min, threshold_max, fixed_amount,
                effective_from, effective_to, active, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,true,$15,$16)
            "#,
        )
        .bind(id)
        .bind(input.jurisdiction_id)
        .bind(tax_type)
        .bind(category)
        .bind(input.rate)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.is_compound)
        .bind(input.priority)
        .bind(input.threshold_min)
        .bind(input.threshold_max)
        .bind(input.fixed_amount)
        .bind(input.effective_from)
        .bind(input.effective_to)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_rate_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_rate_async(&self, id: Uuid) -> Result<Option<TaxRate>> {
        let row = sqlx::query_as::<_, TaxRateRow>(
            "SELECT id, jurisdiction_id, tax_type, product_category, rate, name, description, is_compound, priority,
                    threshold_min, threshold_max, fixed_amount, effective_from, effective_to, active, created_at, updated_at
             FROM tax_rates WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_rate).transpose()
    }

    pub async fn list_rates_async(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        let mut query = String::from(
            "SELECT id, jurisdiction_id, tax_type, product_category, rate, name, description, is_compound, priority,
                    threshold_min, threshold_max, fixed_amount, effective_from, effective_to, active, created_at, updated_at
             FROM tax_rates WHERE 1=1",
        );
        let mut param_idx = 1;

        if filter.jurisdiction_id.is_some() {
            query.push_str(&format!(" AND jurisdiction_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.tax_type.is_some() {
            query.push_str(&format!(" AND tax_type = ${}", param_idx));
            param_idx += 1;
        }
        if filter.product_category.is_some() {
            query.push_str(&format!(" AND product_category = ${}", param_idx));
            param_idx += 1;
        }
        if filter.active_only {
            query.push_str(" AND active = true");
        }
        if filter.effective_date.is_some() {
            query.push_str(&format!(
                " AND effective_from <= ${} AND (effective_to IS NULL OR effective_to >= ${})",
                param_idx,
                param_idx + 1
            ));
        }

        query.push_str(" ORDER BY priority, name");
        {
            let limit = super::effective_limit(filter.limit);
            let offset = i64::from(filter.offset.unwrap_or(0));
            query.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, TaxRateRow>(&query);

        if let Some(jurisdiction_id) = filter.jurisdiction_id {
            q = q.bind(jurisdiction_id);
        }
        if let Some(tax_type) = &filter.tax_type {
            q = q.bind(tax_type.as_str());
        }
        if let Some(category) = &filter.product_category {
            q = q.bind(category.as_str());
        }
        if let Some(date) = filter.effective_date {
            q = q.bind(date).bind(date);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        let mut rates = Vec::with_capacity(rows.len());
        for row in rows {
            rates.push(Self::row_to_rate(row)?);
        }
        Ok(rates)
    }

    /// Every jurisdiction that levies tax on `address` (mirrors the SQLite
    /// backend).
    ///
    /// Country and state are resolved by the `COUNTRY` / `COUNTRY-STATE` code
    /// convention. Below the state there is no code convention, so `County`,
    /// `City`, `District` and `Special` jurisdictions are matched by
    /// [`TaxJurisdiction::covers_address`] against the address's city and
    /// postal code — without this, US local sales tax was unreachable. At
    /// most `LOCAL_JURISDICTION_SCAN_LIMIT` (1000) active jurisdictions per
    /// country are scanned.
    pub async fn jurisdictions_for_address_async(
        &self,
        address: &TaxAddress,
    ) -> Result<Vec<TaxJurisdiction>> {
        let mut resolved: Vec<TaxJurisdiction> = Vec::new();

        if let Some(country) = self.get_jurisdiction_by_code_async(&address.country).await? {
            resolved.push(country);
        }

        if let Some(state) = &address.state {
            let state_code = format!("{}-{}", address.country, state);
            if let Some(state_jurisdiction) =
                self.get_jurisdiction_by_code_async(&state_code).await?
            {
                resolved.push(state_jurisdiction);
            }
        }

        let locals = self
            .list_jurisdictions_async(TaxJurisdictionFilter {
                country_code: Some(address.country.clone()),
                active_only: true,
                limit: Some(LOCAL_JURISDICTION_SCAN_LIMIT),
                ..Default::default()
            })
            .await?;
        for jurisdiction in locals {
            if jurisdiction.level > JurisdictionLevel::State
                && jurisdiction.covers_address(address)
                && !resolved.iter().any(|j| j.id == jurisdiction.id)
            {
                resolved.push(jurisdiction);
            }
        }

        Ok(resolved)
    }

    pub async fn get_rates_for_address_async(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        let jurisdiction_ids: Vec<Uuid> = self
            .jurisdictions_for_address_async(address)
            .await?
            .into_iter()
            .map(|j| j.id)
            .collect();

        if jurisdiction_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_rates = Vec::new();
        for jurisdiction_id in jurisdiction_ids {
            let filter = TaxRateFilter {
                jurisdiction_id: Some(jurisdiction_id),
                product_category: Some(category),
                active_only: true,
                effective_date: Some(date),
                ..Default::default()
            };
            let mut rates = self.list_rates_async(filter).await?;
            all_rates.append(&mut rates);
        }

        all_rates.sort_by_key(|rate| rate.priority);
        Ok(all_rates)
    }

    /// Create a new exemption. It is created **unverified** and is not
    /// honoured by tax calculation until [`Self::verify_exemption_async`] is
    /// called.
    pub async fn create_exemption_async(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let jurisdiction_ids: Vec<String> =
            input.jurisdiction_ids.iter().map(|id| id.to_string()).collect();
        let exempt_categories: Vec<String> =
            input.exempt_categories.iter().map(|cat| cat.to_string()).collect();

        sqlx::query(
            r#"
            INSERT INTO tax_exemptions (
                id, customer_id, exemption_type, certificate_number, issuing_authority,
                jurisdiction_ids, exempt_categories, effective_from, expires_at, verified,
                verified_at, notes, active, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,false,NULL,$10,true,$11,$12)
            "#,
        )
        .bind(id)
        .bind(input.customer_id)
        .bind(input.exemption_type.to_string())
        .bind(&input.certificate_number)
        .bind(&input.issuing_authority)
        .bind(serde_json::to_value(jurisdiction_ids).unwrap_or_default())
        .bind(serde_json::to_value(exempt_categories).unwrap_or_default())
        .bind(input.effective_from)
        .bind(input.expires_at)
        .bind(&input.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_exemption_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_exemption_async(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        let row = sqlx::query_as::<_, TaxExemptionRow>(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority,
                    jurisdiction_ids, exempt_categories, effective_from, expires_at, verified,
                    verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_exemption).transpose()
    }

    /// Mark an exemption certificate as verified (or revoke verification).
    /// Only verified exemptions are honoured by tax calculation.
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::NotFound`] if no exemption has `id`.
    pub async fn verify_exemption_async(&self, id: Uuid, verified: bool) -> Result<TaxExemption> {
        let now = Utc::now();
        let verified_at = if verified { Some(now) } else { None };
        let changed = sqlx::query(
            "UPDATE tax_exemptions SET verified = $1, verified_at = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(verified)
        .bind(verified_at)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if changed == 0 {
            return Err(CommerceError::NotFound);
        }
        self.get_exemption_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get all active exemptions for a customer, whatever their verification
    /// state or validity window. Tax calculation honours only those that are
    /// [`TaxExemption::is_effective_on`] the transaction date (active,
    /// verified, inside the window).
    pub async fn get_customer_exemptions_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Vec<TaxExemption>> {
        let rows = sqlx::query_as::<_, TaxExemptionRow>(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority,
                    jurisdiction_ids, exempt_categories, effective_from, expires_at, verified,
                    verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions
             WHERE customer_id = $1 AND active = true
             ORDER BY effective_from, id",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut exemptions = Vec::with_capacity(rows.len());
        for row in rows {
            exemptions.push(Self::row_to_exemption(row)?);
        }
        Ok(exemptions)
    }

    pub async fn get_settings_async(&self) -> Result<TaxSettings> {
        let row = sqlx::query_as::<_, TaxSettingsRow>(
            "SELECT id, enabled, calculation_method, compound_method, tax_shipping, tax_handling,
                    tax_gift_wrap, origin_address, default_product_category, rounding_mode,
                    decimal_places, validate_addresses, tax_provider, provider_credentials,
                    created_at, updated_at
             FROM tax_settings WHERE id = 'default'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(settings_row) => Self::row_to_settings(settings_row),
            None => Ok(TaxSettings::default()),
        }
    }

    pub async fn update_settings_async(&self, settings: TaxSettings) -> Result<TaxSettings> {
        let origin_address = settings
            .origin_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let calc_method = settings.calculation_method.to_string();
        let compound_method = settings.compound_method.to_string();

        sqlx::query(
            r#"
            INSERT INTO tax_settings (
                id, enabled, calculation_method, compound_method, tax_shipping, tax_handling,
                tax_gift_wrap, origin_address, default_product_category, rounding_mode,
                decimal_places, validate_addresses, tax_provider, provider_credentials, updated_at
            ) VALUES (
                'default', $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW()
            )
            ON CONFLICT (id) DO UPDATE SET
                enabled = EXCLUDED.enabled,
                calculation_method = EXCLUDED.calculation_method,
                compound_method = EXCLUDED.compound_method,
                tax_shipping = EXCLUDED.tax_shipping,
                tax_handling = EXCLUDED.tax_handling,
                tax_gift_wrap = EXCLUDED.tax_gift_wrap,
                origin_address = EXCLUDED.origin_address,
                default_product_category = EXCLUDED.default_product_category,
                rounding_mode = EXCLUDED.rounding_mode,
                decimal_places = EXCLUDED.decimal_places,
                validate_addresses = EXCLUDED.validate_addresses,
                tax_provider = EXCLUDED.tax_provider,
                provider_credentials = EXCLUDED.provider_credentials,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(settings.enabled)
        .bind(calc_method)
        .bind(compound_method)
        .bind(settings.tax_shipping)
        .bind(settings.tax_handling)
        .bind(settings.tax_gift_wrap)
        .bind(origin_address)
        .bind(settings.default_product_category.to_string())
        .bind(settings.rounding_mode)
        .bind(settings.decimal_places)
        .bind(settings.validate_addresses)
        .bind(settings.tax_provider)
        .bind(settings.provider_credentials)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_settings_async().await
    }

    /// Calculate tax for a request.
    ///
    /// This backend only resolves the data — settings, the customer's
    /// exemptions, and the rates (with their jurisdictions) applicable to the
    /// shipping address for every product category the request touches —
    /// and hands it to the shared, pure [`stateset_core::compute_tax`], so
    /// SQLite and Postgres produce identical results for identical inputs.
    pub async fn calculate_tax_async(
        &self,
        request: TaxCalculationRequest,
    ) -> Result<TaxCalculationResult> {
        let now = Utc::now();
        let inputs = self.computation_inputs(&request, now).await?;
        Ok(compute_tax(&request, &inputs, now))
    }

    /// Load everything [`stateset_core::compute_tax`] needs for `request`.
    async fn computation_inputs(
        &self,
        request: &TaxCalculationRequest,
        now: DateTime<Utc>,
    ) -> Result<TaxComputationInputs> {
        let settings = self.get_settings_async().await?;
        let transaction_date = request.transaction_date.unwrap_or_else(|| now.date_naive());

        let exemptions = match request.customer_id {
            Some(customer_id) => self.get_customer_exemptions_async(customer_id).await?,
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

        // Resolve the address's jurisdictions ONCE (not once per category):
        // the address does not change between categories, and the local-level
        // resolution scans the country's jurisdictions.
        let jurisdictions = self.jurisdictions_for_address_async(&request.shipping_address).await?;
        let mut rates = Vec::new();
        for category in categories {
            for jurisdiction in &jurisdictions {
                let filter = TaxRateFilter {
                    jurisdiction_id: Some(jurisdiction.id),
                    product_category: Some(category),
                    active_only: true,
                    effective_date: Some(transaction_date),
                    ..Default::default()
                };
                for rate in self.list_rates_async(filter).await? {
                    rates.push(ResolvedTaxRate { rate, jurisdiction: jurisdiction.clone() });
                }
            }
        }

        Ok(TaxComputationInputs { settings, rates, exemptions })
    }

    pub async fn save_calculation_async(
        &self,
        result: &TaxCalculationResult,
        order_id: Option<Uuid>,
        cart_id: Option<Uuid>,
        customer_id: Option<Uuid>,
        address: &TaxAddress,
        currency: &str,
    ) -> Result<()> {
        let address_json = serde_json::to_value(address)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let line_items_json = serde_json::to_value(&result.line_item_taxes)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let breakdown_json = serde_json::to_value(&result.tax_breakdown)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let exemption_json = result
            .exemption_details
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO tax_calculations (
                id, order_id, cart_id, customer_id, subtotal, total_tax, shipping_tax, total,
                currency, shipping_address, billing_address, line_items, tax_breakdown,
                exemptions_applied, exemption_details, is_estimate, calculated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
            "#,
        )
        .bind(result.id)
        .bind(order_id)
        .bind(cart_id)
        .bind(customer_id)
        .bind(result.subtotal)
        .bind(result.total_tax)
        .bind(result.shipping_tax)
        .bind(result.total)
        .bind(currency)
        .bind(address_json)
        .bind(serde_json::Value::Null)
        .bind(line_items_json)
        .bind(breakdown_json)
        .bind(result.exemptions_applied)
        .bind(exemption_json)
        .bind(result.is_estimate)
        .bind(result.calculated_at)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }
}

impl TaxRepository for PgTaxRepository {
    fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        super::block_on(self.create_jurisdiction_async(input))
    }

    fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        super::block_on(self.get_jurisdiction_async(id))
    }

    fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        super::block_on(self.get_jurisdiction_by_code_async(code))
    }

    fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        super::block_on(self.list_jurisdictions_async(filter))
    }

    fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        super::block_on(self.create_rate_async(input))
    }

    fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        super::block_on(self.get_rate_async(id))
    }

    fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        super::block_on(self.list_rates_async(filter))
    }

    fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: chrono::NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        super::block_on(self.get_rates_for_address_async(address, category, date))
    }

    fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        super::block_on(self.create_exemption_async(input))
    }

    fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        super::block_on(self.get_exemption_async(id))
    }

    fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        super::block_on(self.get_customer_exemptions_async(customer_id))
    }

    fn verify_exemption(&self, id: Uuid, verified: bool) -> Result<TaxExemption> {
        super::block_on(self.verify_exemption_async(id, verified))
    }

    fn get_settings(&self) -> Result<TaxSettings> {
        super::block_on(self.get_settings_async())
    }

    fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        super::block_on(self.update_settings_async(settings))
    }

    fn calculate_tax(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        super::block_on(self.calculate_tax_async(request))
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
        super::block_on(self.save_calculation_async(
            result,
            order_id,
            cart_id,
            customer_id,
            address,
            currency,
        ))
    }
}
