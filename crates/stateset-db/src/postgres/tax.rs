//! PostgreSQL implementation of tax repository

use super::map_db_error;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    CommerceError, CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ExemptionType,
    JurisdictionLevel, ProductTaxCategory, Result, TaxAddress, TaxBreakdown, TaxCalculationMethod,
    TaxCalculationRequest, TaxCalculationResult, TaxCompoundMethod, TaxDetail, TaxExemption,
    TaxJurisdiction, TaxJurisdictionFilter, TaxRate, TaxRateFilter, TaxRepository, TaxSettings,
    TaxType, LineItemTax, JurisdictionSummary,
};
use uuid::Uuid;

/// PostgreSQL tax repository
#[derive(Clone)]
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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

    fn calculation_method_str(method: TaxCalculationMethod) -> &'static str {
        match method {
            TaxCalculationMethod::Inclusive => "inclusive",
            TaxCalculationMethod::Exclusive => "exclusive",
        }
    }

    fn compound_method_str(method: TaxCompoundMethod) -> &'static str {
        match method {
            TaxCompoundMethod::Combined => "combined",
            TaxCompoundMethod::Compound => "compound",
            TaxCompoundMethod::Separate => "separate",
        }
    }

    fn row_to_jurisdiction(row: TaxJurisdictionRow) -> TaxJurisdiction {
        let postal_codes: Vec<String> = serde_json::from_value(row.postal_codes).unwrap_or_default();

        TaxJurisdiction {
            id: row.id,
            parent_id: row.parent_id,
            name: row.name,
            code: row.code,
            level: Self::parse_jurisdiction_level(&row.level),
            country_code: row.country_code,
            state_code: row.state_code,
            county: row.county,
            city: row.city,
            postal_codes,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_rate(row: TaxRateRow) -> TaxRate {
        TaxRate {
            id: row.id,
            jurisdiction_id: row.jurisdiction_id,
            tax_type: Self::parse_tax_type(&row.tax_type),
            product_category: Self::parse_product_category(&row.product_category),
            rate: row.rate,
            name: row.name,
            description: row.description,
            is_compound: row.is_compound,
            priority: row.priority,
            threshold_min: row.threshold_min,
            threshold_max: row.threshold_max,
            fixed_amount: row.fixed_amount,
            effective_from: row.effective_from,
            effective_to: row.effective_to,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_exemption(row: TaxExemptionRow) -> TaxExemption {
        let jurisdiction_ids: Vec<Uuid> = serde_json::from_value::<Vec<String>>(row.jurisdiction_ids)
            .unwrap_or_default()
            .iter()
            .filter_map(|id| id.parse().ok())
            .collect();

        let exempt_categories: Vec<ProductTaxCategory> =
            serde_json::from_value::<Vec<String>>(row.exempt_categories)
                .unwrap_or_default()
                .iter()
                .map(|s| Self::parse_product_category(s))
                .collect();

        TaxExemption {
            id: row.id,
            customer_id: row.customer_id,
            exemption_type: Self::parse_exemption_type(&row.exemption_type),
            certificate_number: row.certificate_number,
            issuing_authority: row.issuing_authority,
            jurisdiction_ids,
            exempt_categories,
            effective_from: row.effective_from,
            expires_at: row.expires_at,
            verified: row.verified,
            verified_at: row.verified_at,
            notes: row.notes,
            active: row.active,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_settings(row: TaxSettingsRow) -> TaxSettings {
        let origin_address = row
            .origin_address
            .and_then(|value| serde_json::from_value(value).ok());

        TaxSettings {
            id: row.id.parse().unwrap_or_default(),
            enabled: row.enabled,
            calculation_method: Self::parse_calculation_method(&row.calculation_method),
            compound_method: Self::parse_compound_method(&row.compound_method),
            tax_shipping: row.tax_shipping,
            tax_handling: row.tax_handling,
            tax_gift_wrap: row.tax_gift_wrap,
            origin_address,
            default_product_category: Self::parse_product_category(&row.default_product_category),
            rounding_mode: row.rounding_mode,
            decimal_places: row.decimal_places,
            validate_addresses: row.validate_addresses,
            tax_provider: row.tax_provider,
            provider_credentials: row.provider_credentials,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

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
        .bind(&input.code)
        .bind(match input.level {
            JurisdictionLevel::Country => "country",
            JurisdictionLevel::State => "state",
            JurisdictionLevel::County => "county",
            JurisdictionLevel::City => "city",
            JurisdictionLevel::District => "district",
            JurisdictionLevel::Special => "special",
        })
        .bind(&input.country_code)
        .bind(&input.state_code)
        .bind(&input.county)
        .bind(&input.city)
        .bind(postal_codes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_jurisdiction_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
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

        Ok(row.map(Self::row_to_jurisdiction))
    }

    pub async fn get_jurisdiction_by_code_async(
        &self,
        code: &str,
    ) -> Result<Option<TaxJurisdiction>> {
        let row = sqlx::query_as::<_, TaxJurisdictionRow>(
            "SELECT id, parent_id, name, code, level, country_code, state_code, county, city, postal_codes, active, created_at, updated_at
             FROM tax_jurisdictions WHERE code = $1",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_jurisdiction))
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

        if filter.country_code.is_some() {
            query.push_str(&format!(" AND country_code = ${}", param_idx));
            param_idx += 1;
        }
        if filter.state_code.is_some() {
            query.push_str(&format!(" AND state_code = ${}", param_idx));
            param_idx += 1;
        }
        if filter.level.is_some() {
            query.push_str(&format!(" AND level = ${}", param_idx));
            param_idx += 1;
        }
        if filter.active_only {
            query.push_str(" AND active = true");
        }

        query.push_str(" ORDER BY level, name");

        let mut q = sqlx::query_as::<_, TaxJurisdictionRow>(&query);

        if let Some(country) = &filter.country_code {
            q = q.bind(country);
        }
        if let Some(state) = &filter.state_code {
            q = q.bind(state);
        }
        if let Some(level) = &filter.level {
            let level_str = match level {
                JurisdictionLevel::Country => "country",
                JurisdictionLevel::State => "state",
                JurisdictionLevel::County => "county",
                JurisdictionLevel::City => "city",
                JurisdictionLevel::District => "district",
                JurisdictionLevel::Special => "special",
            };
            q = q.bind(level_str);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_jurisdiction).collect())
    }

    pub async fn create_rate_async(&self, input: CreateTaxRate) -> Result<TaxRate> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let tax_type = input.tax_type.as_str();
        let category = input.product_category.as_str();

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
        .bind(input.priority.unwrap_or(0))
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

        Ok(row.map(Self::row_to_rate))
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
            query.push_str(&format!(" AND effective_from <= ${} AND (effective_to IS NULL OR effective_to >= ${})", param_idx, param_idx + 1));
            param_idx += 2;
        }

        query.push_str(" ORDER BY priority, name");

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
        Ok(rows.into_iter().map(Self::row_to_rate).collect())
    }

    pub async fn get_rates_for_address_async(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        let mut jurisdiction_ids = Vec::new();

        if let Some(country) = self.get_jurisdiction_by_code_async(&address.country).await? {
            jurisdiction_ids.push(country.id);
        }

        if let Some(state) = &address.state {
            let state_code = format!("{}-{}", address.country, state);
            if let Some(state_jurisdiction) = self.get_jurisdiction_by_code_async(&state_code).await? {
                jurisdiction_ids.push(state_jurisdiction.id);
            }
        }

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

    pub async fn create_exemption_async(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let jurisdiction_ids: Vec<String> = input
            .jurisdiction_ids
            .iter()
            .map(|id| id.to_string())
            .collect();
        let exempt_categories: Vec<String> = input
            .exempt_categories
            .iter()
            .map(|cat| cat.as_str().to_string())
            .collect();

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
        .bind(match input.exemption_type {
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
        })
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

        self.get_exemption_async(id)
            .await?
            .ok_or(CommerceError::NotFound)
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

        Ok(row.map(Self::row_to_exemption))
    }

    pub async fn get_customer_exemptions_async(
        &self,
        customer_id: Uuid,
    ) -> Result<Vec<TaxExemption>> {
        let today = Utc::now().date_naive();
        let rows = sqlx::query_as::<_, TaxExemptionRow>(
            "SELECT id, customer_id, exemption_type, certificate_number, issuing_authority,
                    jurisdiction_ids, exempt_categories, effective_from, expires_at, verified,
                    verified_at, notes, active, created_at, updated_at
             FROM tax_exemptions
             WHERE customer_id = $1 AND active = true AND effective_from <= $2 AND (expires_at IS NULL OR expires_at >= $2)",
        )
        .bind(customer_id)
        .bind(today)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_exemption).collect())
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

        Ok(row.map(Self::row_to_settings).unwrap_or_default())
    }

    pub async fn update_settings_async(&self, settings: TaxSettings) -> Result<TaxSettings> {
        let origin_address = settings
            .origin_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let calc_method = Self::calculation_method_str(settings.calculation_method);
        let compound_method = Self::compound_method_str(settings.compound_method);

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
        .bind(settings.default_product_category.as_str())
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

    pub async fn calculate_tax_async(
        &self,
        request: TaxCalculationRequest,
    ) -> Result<TaxCalculationResult> {
        let settings = self.get_settings_async().await?;
        let now = Utc::now();
        let transaction_date = request.transaction_date.unwrap_or_else(|| now.date_naive());

        let exemptions = if let Some(customer_id) = request.customer_id {
            self.get_customer_exemptions_async(customer_id).await?
        } else {
            Vec::new()
        };

        let mut subtotal = Decimal::ZERO;
        let mut total_tax = Decimal::ZERO;
        let mut line_item_taxes = Vec::new();
        let mut tax_breakdown: Vec<TaxBreakdown> = Vec::new();
        let mut jurisdictions_map = std::collections::HashMap::new();

        for item in &request.line_items {
            let line_amount = item.unit_price * item.quantity - item.discount_amount;
            subtotal += line_amount;

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

            let rates = self
                .get_rates_for_address_async(&request.shipping_address, item.tax_category, transaction_date)
                .await?;

            let mut line_tax = Decimal::ZERO;
            let mut line_tax_details = Vec::new();
            let taxable_base = line_amount;

            for rate in &rates {
                let tax_amount = if rate.is_compound {
                    (taxable_base + line_tax) * rate.rate
                } else {
                    taxable_base * rate.rate
                };

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

                if let Some(jurisdiction) = self.get_jurisdiction_async(rate.jurisdiction_id).await? {
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
                        summary.total_tax += final_tax;
                    }

                    if let Some(existing) = tax_breakdown.iter_mut().find(|b| {
                        b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type
                    }) {
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

        let mut shipping_tax = Decimal::ZERO;
        if settings.tax_shipping {
            if let Some(shipping_amount) = request.shipping_amount {
                let shipping_rates = self
                    .get_rates_for_address_async(
                        &request.shipping_address,
                        ProductTaxCategory::Standard,
                        transaction_date,
                    )
                    .await?;
                for rate in shipping_rates {
                    shipping_tax += shipping_amount * rate.rate;
                }
                total_tax += shipping_tax;
            }
        }

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
            exemption_details: None,
            jurisdictions: jurisdictions_map.into_values().collect(),
            calculated_at: now,
            is_estimate: true,
        })
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
