//! PostgreSQL implementation of currency repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::FromRow;
use stateset_core::{
    CommerceError, ConversionResult, ConvertCurrency, Currency, CurrencyRepository, ExchangeRate,
    ExchangeRateFilter, Result, RoundingMode, SetExchangeRate, StoreCurrencySettings,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL currency repository
#[derive(Clone)]
pub struct PgCurrencyRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ExchangeRateRow {
    id: Uuid,
    base_currency: String,
    quote_currency: String,
    rate: Decimal,
    source: String,
    rate_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct SettingsRow {
    base_currency: String,
    enabled_currencies: serde_json::Value,
    auto_convert: bool,
    rounding_mode: String,
}

impl PgCurrencyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_currency(s: &str) -> Result<Currency> {
        Currency::from_str(s).map_err(CommerceError::ValidationError)
    }

    fn row_to_exchange_rate(row: ExchangeRateRow) -> Result<ExchangeRate> {
        Ok(ExchangeRate {
            id: row.id,
            base_currency: Self::parse_currency(&row.base_currency)?,
            quote_currency: Self::parse_currency(&row.quote_currency)?,
            rate: row.rate,
            source: row.source,
            rate_at: row.rate_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Get rate (async)
    pub async fn get_rate_async(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        // Same currency = rate of 1
        if from == to {
            return Ok(Some(ExchangeRate {
                id: Uuid::nil(),
                base_currency: from,
                quote_currency: to,
                rate: Decimal::ONE,
                source: "identity".into(),
                rate_at: Utc::now(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }));
        }

        // Try direct rate
        let row = sqlx::query_as::<_, ExchangeRateRow>(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates WHERE base_currency = $1 AND quote_currency = $2",
        )
        .bind(from.code())
        .bind(to.code())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            return Ok(Some(Self::row_to_exchange_rate(row)?));
        }

        // Try inverse rate
        let inverse_row = sqlx::query_as::<_, ExchangeRateRow>(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates WHERE base_currency = $1 AND quote_currency = $2",
        )
        .bind(to.code())
        .bind(from.code())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = inverse_row {
            let inverse_rate = if row.rate.is_zero() {
                Decimal::ZERO
            } else {
                Decimal::ONE / row.rate
            };

            return Ok(Some(ExchangeRate {
                id: Uuid::new_v4(),
                base_currency: from,
                quote_currency: to,
                rate: inverse_rate,
                source: format!("inverse:{}", row.source),
                rate_at: row.rate_at,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }));
        }

        Ok(None)
    }

    /// Get rates for base currency (async)
    pub async fn get_rates_for_async(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        let rows = sqlx::query_as::<_, ExchangeRateRow>(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates WHERE base_currency = $1 ORDER BY quote_currency",
        )
        .bind(base.code())
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_exchange_rate).collect()
    }

    /// List rates (async)
    pub async fn list_rates_async(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        let mut query = String::from(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates WHERE 1=1",
        );
        let mut param_idx = 1;

        if filter.base_currency.is_some() {
            query.push_str(&format!(" AND base_currency = ${}", param_idx));
            param_idx += 1;
        }
        if filter.quote_currency.is_some() {
            query.push_str(&format!(" AND quote_currency = ${}", param_idx));
            param_idx += 1;
        }
        if filter.since.is_some() {
            query.push_str(&format!(" AND rate_at >= ${}", param_idx));
        }
        query.push_str(" ORDER BY base_currency, quote_currency");

        // Build query with bindings
        let mut q = sqlx::query_as::<_, ExchangeRateRow>(&query);

        if let Some(base) = &filter.base_currency {
            q = q.bind(base.code());
        }
        if let Some(quote) = &filter.quote_currency {
            q = q.bind(quote.code());
        }
        if let Some(since) = &filter.since {
            q = q.bind(since);
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_exchange_rate).collect()
    }

    /// Set rate (async)
    pub async fn set_rate_async(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let source = input.source.unwrap_or_else(|| "manual".into());

        // Upsert the rate
        sqlx::query(
            "INSERT INTO exchange_rates (id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (base_currency, quote_currency) DO UPDATE SET
                rate = EXCLUDED.rate,
                source = EXCLUDED.source,
                rate_at = EXCLUDED.rate_at,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(id)
        .bind(input.base_currency.code())
        .bind(input.quote_currency.code())
        .bind(input.rate)
        .bind(&source)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Record in history
        sqlx::query(
            "INSERT INTO exchange_rate_history (base_currency, quote_currency, rate, source, rate_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(input.base_currency.code())
        .bind(input.quote_currency.code())
        .bind(input.rate)
        .bind(&source)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_rate_async(input.base_currency, input.quote_currency)
            .await?
            .ok_or(CommerceError::NotFound)
    }

    /// Delete rate (async)
    pub async fn delete_rate_async(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM exchange_rates WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        if result.rows_affected() == 0 {
            Err(CommerceError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Convert currency (async)
    pub async fn convert_async(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        if input.from == input.to {
            return Ok(ConversionResult {
                original_amount: input.amount,
                original_currency: input.from,
                converted_amount: input.amount,
                target_currency: input.to,
                rate: Decimal::ONE,
                inverse_rate: Decimal::ONE,
                rate_at: Utc::now(),
            });
        }

        let rate = self
            .get_rate_async(input.from, input.to)
            .await?
            .ok_or_else(|| {
                CommerceError::ValidationError(format!(
                    "No exchange rate found for {} to {}",
                    input.from, input.to
                ))
            })?;

        let converted_amount = input.amount * rate.rate;
        let inverse_rate = if rate.rate.is_zero() {
            Decimal::ZERO
        } else {
            Decimal::ONE / rate.rate
        };

        Ok(ConversionResult {
            original_amount: input.amount,
            original_currency: input.from,
            converted_amount,
            target_currency: input.to,
            rate: rate.rate,
            inverse_rate,
            rate_at: rate.rate_at,
        })
    }

    /// Get settings (async)
    pub async fn get_settings_async(&self) -> Result<StoreCurrencySettings> {
        let row = sqlx::query_as::<_, SettingsRow>(
            "SELECT base_currency, enabled_currencies, auto_convert, rounding_mode
             FROM store_currency_settings WHERE id = 'default'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => {
                let base_currency = Self::parse_currency(&row.base_currency)?;
                let enabled_currencies: Vec<Currency> =
                    serde_json::from_value(row.enabled_currencies)
                        .unwrap_or_else(|_| vec![Currency::USD, Currency::EUR, Currency::GBP]);
                let rounding_mode = match row.rounding_mode.as_str() {
                    "half_down" => RoundingMode::HalfDown,
                    "up" => RoundingMode::Up,
                    "down" => RoundingMode::Down,
                    "half_even" => RoundingMode::HalfEven,
                    _ => RoundingMode::HalfUp,
                };

                Ok(StoreCurrencySettings {
                    base_currency,
                    enabled_currencies,
                    auto_convert: row.auto_convert,
                    rounding_mode,
                })
            }
            None => Ok(StoreCurrencySettings::default()),
        }
    }

    /// Update settings (async)
    pub async fn update_settings_async(
        &self,
        settings: StoreCurrencySettings,
    ) -> Result<StoreCurrencySettings> {
        let enabled_json = serde_json::to_value(&settings.enabled_currencies)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rounding_str = match settings.rounding_mode {
            RoundingMode::HalfUp => "half_up",
            RoundingMode::HalfDown => "half_down",
            RoundingMode::Up => "up",
            RoundingMode::Down => "down",
            RoundingMode::HalfEven => "half_even",
        };

        sqlx::query(
            "INSERT INTO store_currency_settings (id, base_currency, enabled_currencies, auto_convert, rounding_mode, updated_at)
             VALUES ('default', $1, $2, $3, $4, NOW())
             ON CONFLICT (id) DO UPDATE SET
                base_currency = EXCLUDED.base_currency,
                enabled_currencies = EXCLUDED.enabled_currencies,
                auto_convert = EXCLUDED.auto_convert,
                rounding_mode = EXCLUDED.rounding_mode,
                updated_at = EXCLUDED.updated_at",
        )
        .bind(settings.base_currency.code())
        .bind(enabled_json)
        .bind(settings.auto_convert)
        .bind(rounding_str)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_settings_async().await
    }
}

impl CurrencyRepository for PgCurrencyRepository {
    fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        tokio::runtime::Handle::current().block_on(self.get_rate_async(from, to))
    }

    fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        tokio::runtime::Handle::current().block_on(self.get_rates_for_async(base))
    }

    fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        tokio::runtime::Handle::current().block_on(self.list_rates_async(filter))
    }

    fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        tokio::runtime::Handle::current().block_on(self.set_rate_async(input))
    }

    fn set_rates(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>> {
        let mut results = Vec::new();
        for rate in rates {
            results.push(self.set_rate(rate)?);
        }
        Ok(results)
    }

    fn delete_rate(&self, id: Uuid) -> Result<()> {
        tokio::runtime::Handle::current().block_on(self.delete_rate_async(id))
    }

    fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        tokio::runtime::Handle::current().block_on(self.convert_async(input))
    }

    fn get_settings(&self) -> Result<StoreCurrencySettings> {
        tokio::runtime::Handle::current().block_on(self.get_settings_async())
    }

    fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings> {
        tokio::runtime::Handle::current().block_on(self.update_settings_async(settings))
    }
}
