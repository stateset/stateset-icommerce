//! Fixed Assets  (all monetary values cross as exact decimal strings).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Fixed Assets  (all monetary values cross as exact decimal strings)
// ============================================================================

pub(crate) fn parse_iso_date(s: &str, field: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        coded(ErrCode::Validation, format!("Invalid {field} date (expected YYYY-MM-DD)"))
    })
}

pub(crate) fn parse_decimal_str(s: &str, field: &str) -> Result<Decimal> {
    s.parse::<Decimal>().map_err(|_| coded(ErrCode::Validation, format!("Invalid {field} decimal")))
}

pub(crate) fn parse_optional_uuid(s: Option<String>, field: &str) -> Result<Option<uuid::Uuid>> {
    s.map(|s| {
        s.parse::<uuid::Uuid>()
            .map_err(|_| coded(ErrCode::Validation, format!("Invalid {field} UUID")))
    })
    .transpose()
}

pub(crate) fn parse_depreciation_method(
    method: &str,
    rate: Option<&str>,
) -> Result<stateset_core::DepreciationMethod> {
    match method {
        "straight_line" => Ok(stateset_core::DepreciationMethod::StraightLine),
        "declining_balance" => {
            let rate = rate.ok_or_else(|| {
                coded(ErrCode::Validation, "declining_balance requires declining_balance_rate")
            })?;
            Ok(stateset_core::DepreciationMethod::DecliningBalance {
                rate: parse_decimal_str(rate, "declining_balance_rate")?,
            })
        }
        "units_of_production" => Ok(stateset_core::DepreciationMethod::UnitsOfProduction),
        _ => Err(coded(
            ErrCode::Validation,
            "Invalid depreciation method (expected straight_line, declining_balance, or units_of_production)",
        )),
    }
}

pub(crate) fn depreciation_method_parts(
    method: stateset_core::DepreciationMethod,
) -> (String, Option<String>) {
    match method {
        stateset_core::DepreciationMethod::StraightLine => ("straight_line".to_string(), None),
        stateset_core::DepreciationMethod::DecliningBalance { rate } => {
            ("declining_balance".to_string(), Some(rate.to_string()))
        }
        stateset_core::DepreciationMethod::UnitsOfProduction => {
            ("units_of_production".to_string(), None)
        }
        _ => ("unknown".to_string(), None),
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFixedAssetInput {
    /// Optional asset number; auto-generated when omitted (FA-...)
    pub asset_number: Option<String>,
    pub name: String,
    pub description: Option<String>,
    /// Category: land, building, machinery, equipment, vehicle,
    /// furniture_and_fixtures, computer_hardware, software,
    /// leasehold_improvement, other
    #[napi(ts_type = "FixedAssetCategory")]
    pub category: String,
    /// ISO date (YYYY-MM-DD)
    pub acquisition_date: String,
    /// Exact decimal string, e.g. "10000.00"
    pub acquisition_cost: String,
    /// Exact decimal string
    pub salvage_value: String,
    pub useful_life_months: u32,
    /// straight_line, declining_balance, units_of_production
    #[napi(ts_type = "DepreciationMethod")]
    pub depreciation_method: String,
    /// Required for declining_balance: periodic rate as exact decimal string
    /// strictly between 0 and 1 (e.g. "0.2" for 20%)
    pub declining_balance_rate: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateFixedAssetInput {
    pub name: Option<String>,
    pub description: Option<String>,
    #[napi(ts_type = "FixedAssetCategory")]
    pub category: Option<String>,
    /// Exact decimal string
    pub salvage_value: Option<String>,
    pub useful_life_months: Option<u32>,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FixedAssetFilterInput {
    #[napi(ts_type = "FixedAssetCategory")]
    pub category: Option<String>,
    /// draft, in_service, fully_depreciated, disposed, written_off
    #[napi(ts_type = "FixedAssetStatus")]
    pub status: Option<String>,
    pub location_id: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub acquired_from: Option<String>,
    /// ISO date (YYYY-MM-DD)
    pub acquired_to: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AssetDisposalOutput {
    /// ISO date (YYYY-MM-DD)
    pub disposal_date: String,
    /// Exact decimal string
    pub proceeds: String,
    /// Exact decimal string
    pub book_value_at_disposal: String,
    /// Exact decimal string: proceeds - book value
    pub gain_loss: String,
    pub notes: Option<String>,
}

impl From<stateset_core::AssetDisposal> for AssetDisposalOutput {
    fn from(d: stateset_core::AssetDisposal) -> Self {
        Self {
            disposal_date: d.disposal_date.to_string(),
            proceeds: d.proceeds.to_string(),
            book_value_at_disposal: d.book_value_at_disposal.to_string(),
            gain_loss: d.gain_loss.to_string(),
            notes: d.notes,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FixedAssetOutput {
    pub id: String,
    pub asset_number: String,
    pub name: String,
    pub description: Option<String>,
    #[napi(ts_type = "FixedAssetCategory")]
    pub category: String,
    /// ISO date (YYYY-MM-DD)
    pub acquisition_date: String,
    /// Exact decimal string
    pub acquisition_cost: String,
    /// Exact decimal string
    pub salvage_value: String,
    pub useful_life_months: u32,
    /// straight_line, declining_balance, units_of_production
    #[napi(ts_type = "DepreciationMethodOutput")]
    pub depreciation_method: String,
    /// Set when depreciation_method is declining_balance
    pub declining_balance_rate: Option<String>,
    /// draft, in_service, fully_depreciated, disposed, written_off
    #[napi(ts_type = "FixedAssetStatus")]
    pub status: String,
    /// ISO date (YYYY-MM-DD)
    pub in_service_date: Option<String>,
    pub location_id: Option<String>,
    pub asset_account_id: Option<String>,
    pub accumulated_depreciation_account_id: Option<String>,
    pub depreciation_expense_account_id: Option<String>,
    /// Exact decimal string
    pub accumulated_depreciation: String,
    /// Exact decimal string: acquisition_cost - accumulated_depreciation
    pub book_value: String,
    pub currency: String,
    pub disposal: Option<AssetDisposalOutput>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FixedAsset> for FixedAssetOutput {
    fn from(a: stateset_core::FixedAsset) -> Self {
        let book_value = a.book_value();
        let (method, rate) = depreciation_method_parts(a.depreciation_method);
        Self {
            id: a.id.to_string(),
            asset_number: a.asset_number,
            name: a.name,
            description: a.description,
            category: format!("{}", a.category),
            acquisition_date: a.acquisition_date.to_string(),
            acquisition_cost: a.acquisition_cost.to_string(),
            salvage_value: a.salvage_value.to_string(),
            useful_life_months: a.useful_life_months,
            depreciation_method: method,
            declining_balance_rate: rate,
            status: format!("{}", a.status),
            in_service_date: a.in_service_date.map(|d| d.to_string()),
            location_id: a.location_id.map(|id| id.to_string()),
            asset_account_id: a.asset_account_id.map(|id| id.to_string()),
            accumulated_depreciation_account_id: a
                .accumulated_depreciation_account_id
                .map(|id| id.to_string()),
            depreciation_expense_account_id: a
                .depreciation_expense_account_id
                .map(|id| id.to_string()),
            accumulated_depreciation: a.accumulated_depreciation.to_string(),
            book_value: book_value.to_string(),
            currency: a.currency.to_string(),
            disposal: a.disposal.map(Into::into),
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct DepreciationEntryOutput {
    pub period: u32,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub accumulated: String,
    /// Exact decimal string
    pub book_value: String,
    /// scheduled or posted
    #[napi(ts_type = "DepreciationEntryStatus")]
    pub status: String,
}

impl From<stateset_core::DepreciationEntry> for DepreciationEntryOutput {
    fn from(e: stateset_core::DepreciationEntry) -> Self {
        Self {
            period: e.period,
            amount: e.amount.to_string(),
            accumulated: e.accumulated.to_string(),
            book_value: e.book_value.to_string(),
            status: format!("{}", e.status),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct DepreciationScheduleOutput {
    pub asset_id: String,
    /// straight_line, declining_balance, units_of_production
    #[napi(ts_type = "DepreciationMethodOutput")]
    pub method: String,
    /// Set when method is declining_balance
    pub declining_balance_rate: Option<String>,
    pub entries: Vec<DepreciationEntryOutput>,
    /// Exact decimal string
    pub total_depreciation: String,
}

impl From<stateset_core::DepreciationSchedule> for DepreciationScheduleOutput {
    fn from(s: stateset_core::DepreciationSchedule) -> Self {
        let (method, rate) = depreciation_method_parts(s.method);
        Self {
            asset_id: s.asset_id.to_string(),
            method,
            declining_balance_rate: rate,
            entries: s.entries.into_iter().map(Into::into).collect(),
            total_depreciation: s.total_depreciation.to_string(),
        }
    }
}

#[napi]
pub struct FixedAssets {
    pub(crate) commerce: Handle,
}

#[napi]
impl FixedAssets {
    /// Whether the fixed-assets backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.fixed_assets().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateFixedAssetInput) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let category = input
            .category
            .parse::<stateset_core::FixedAssetCategory>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid fixed asset category"))?;
        let depreciation_method = parse_depreciation_method(
            &input.depreciation_method,
            input.declining_balance_rate.as_deref(),
        )?;
        let currency = input
            .currency
            .map(|s| {
                s.parse::<CurrencyCode>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid currency code"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .create(stateset_core::CreateFixedAsset {
                asset_number: input.asset_number,
                name: input.name,
                description: input.description,
                category,
                acquisition_date: parse_iso_date(&input.acquisition_date, "acquisition_date")?,
                acquisition_cost: parse_decimal_str(&input.acquisition_cost, "acquisition_cost")?,
                salvage_value: parse_decimal_str(&input.salvage_value, "salvage_value")?,
                useful_life_months: input.useful_life_months,
                depreciation_method,
                in_service_date: input
                    .in_service_date
                    .as_deref()
                    .map(|s| parse_iso_date(s, "in_service_date"))
                    .transpose()?,
                location_id: parse_optional_uuid(input.location_id, "location_id")?,
                asset_account_id: parse_optional_uuid(input.asset_account_id, "asset_account_id")?,
                accumulated_depreciation_account_id: parse_optional_uuid(
                    input.accumulated_depreciation_account_id,
                    "accumulated_depreciation_account_id",
                )?,
                depreciation_expense_account_id: parse_optional_uuid(
                    input.depreciation_expense_account_id,
                    "depreciation_expense_account_id",
                )?,
                currency,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create fixed asset", e))?;
        Ok(asset.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<FixedAssetOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get fixed asset", e))?;
        Ok(asset.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<FixedAssetFilterInput>,
    ) -> Result<Vec<FixedAssetOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FixedAssetFilter::default()),
            |f| -> Result<stateset_core::FixedAssetFilter> {
                Ok(stateset_core::FixedAssetFilter {
                    category: f
                        .category
                        .map(|s| {
                            s.parse::<stateset_core::FixedAssetCategory>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid fixed asset category")
                            })
                        })
                        .transpose()?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::FixedAssetStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid fixed asset status")
                            })
                        })
                        .transpose()?,
                    location_id: parse_optional_uuid(f.location_id, "location_id")?,
                    acquired_from: f
                        .acquired_from
                        .as_deref()
                        .map(|s| parse_iso_date(s, "acquired_from"))
                        .transpose()?,
                    acquired_to: f
                        .acquired_to
                        .as_deref()
                        .map(|s| parse_iso_date(s, "acquired_to"))
                        .transpose()?,
                    search: f.search,
                    limit: f.limit,
                    offset: f.offset,
                    after_cursor: None,
                })
            },
        )?;
        let assets = commerce
            .fixed_assets()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list fixed assets", e))?;
        Ok(assets.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateFixedAssetInput,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let category = input
            .category
            .map(|s| {
                s.parse::<stateset_core::FixedAssetCategory>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid fixed asset category"))
            })
            .transpose()?;
        let asset = commerce
            .fixed_assets()
            .update(
                uuid,
                stateset_core::UpdateFixedAsset {
                    name: input.name,
                    description: input.description,
                    category,
                    salvage_value: input
                        .salvage_value
                        .as_deref()
                        .map(|s| parse_decimal_str(s, "salvage_value"))
                        .transpose()?,
                    useful_life_months: input.useful_life_months,
                    in_service_date: input
                        .in_service_date
                        .as_deref()
                        .map(|s| parse_iso_date(s, "in_service_date"))
                        .transpose()?,
                    location_id: parse_optional_uuid(input.location_id, "location_id")?,
                    asset_account_id: parse_optional_uuid(
                        input.asset_account_id,
                        "asset_account_id",
                    )?,
                    accumulated_depreciation_account_id: parse_optional_uuid(
                        input.accumulated_depreciation_account_id,
                        "accumulated_depreciation_account_id",
                    )?,
                    depreciation_expense_account_id: parse_optional_uuid(
                        input.depreciation_expense_account_id,
                        "depreciation_expense_account_id",
                    )?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update fixed asset", e))?;
        Ok(asset.into())
    }

    /// Place a draft asset in service on the given ISO date (YYYY-MM-DD).
    #[napi]
    pub async fn place_in_service(&self, id: String, date: String) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let date = parse_iso_date(&date, "date")?;
        let asset = commerce
            .fixed_assets()
            .place_in_service(uuid, date)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to place asset in service", e))?;
        Ok(asset.into())
    }

    /// Dispose of an asset for the given proceeds (exact decimal string),
    /// recording gain/loss. `date` is an ISO date (YYYY-MM-DD); defaults to today.
    #[napi]
    pub async fn dispose(
        &self,
        id: String,
        proceeds: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let proceeds = parse_decimal_str(&proceeds, "proceeds")?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce
            .fixed_assets()
            .dispose(uuid, date, proceeds, notes)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to dispose fixed asset", e))?;
        Ok(asset.into())
    }

    /// Write off an asset (disposal with zero proceeds). `date` is an ISO date
    /// (YYYY-MM-DD); defaults to today.
    #[napi]
    pub async fn write_off(
        &self,
        id: String,
        date: Option<String>,
        notes: Option<String>,
    ) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let date = date
            .as_deref()
            .map(|s| parse_iso_date(s, "date"))
            .transpose()?
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let asset = commerce
            .fixed_assets()
            .write_off(uuid, date, notes)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to write off fixed asset", e))?;
        Ok(asset.into())
    }

    /// Generate and persist the depreciation schedule for an asset.
    #[napi]
    pub async fn generate_schedule(&self, id: String) -> Result<DepreciationScheduleOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .generate_schedule(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to generate schedule", e))?;
        Ok(schedule.into())
    }

    /// Get the persisted depreciation schedule for an asset, if generated.
    #[napi]
    pub async fn get_schedule(&self, id: String) -> Result<Option<DepreciationScheduleOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let schedule = commerce
            .fixed_assets()
            .get_schedule(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get schedule", e))?;
        Ok(schedule.map(Into::into))
    }

    /// Post the next `periods` scheduled depreciation entries.
    #[napi]
    pub async fn post_depreciation(&self, id: String, periods: u32) -> Result<FixedAssetOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let asset = commerce
            .fixed_assets()
            .post_depreciation(uuid, periods)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to post depreciation", e))?;
        Ok(asset.into())
    }
}
