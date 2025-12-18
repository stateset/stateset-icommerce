//! SQLite implementation of currency repository

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use rusqlite::params;
use stateset_core::{
    CommerceError, ConversionResult, ConvertCurrency, Currency, ExchangeRate, ExchangeRateFilter,
    Result, RoundingMode, SetExchangeRate, StoreCurrencySettings,
};
use std::str::FromStr;
use uuid::Uuid;

use super::map_db_error;

/// SQLite currency repository
pub struct SqliteCurrencyRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCurrencyRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn parse_currency(s: &str) -> Result<Currency> {
        Currency::from_str(s).map_err(|e| CommerceError::ValidationError(e))
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

    fn parse_decimal(s: &str) -> Decimal {
        s.parse().unwrap_or_default()
    }
}

impl stateset_core::CurrencyRepository for SqliteCurrencyRepository {
    fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

        let result = conn.query_row(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates
             WHERE base_currency = ? AND quote_currency = ?",
            params![from.code(), to.code()],
            |row| {
                Ok(ExchangeRate {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    base_currency: Self::parse_currency(&row.get::<_, String>(1)?).unwrap_or_default(),
                    quote_currency: Self::parse_currency(&row.get::<_, String>(2)?).unwrap_or_default(),
                    rate: Self::parse_decimal(&row.get::<_, String>(3)?),
                    source: row.get(4)?,
                    rate_at: Self::parse_datetime(&row.get::<_, String>(5)?),
                    created_at: Self::parse_datetime(&row.get::<_, String>(6)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(7)?),
                })
            },
        );

        match result {
            Ok(rate) => Ok(Some(rate)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Try to find inverse rate
                let inverse_result = conn.query_row(
                    "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
                     FROM exchange_rates
                     WHERE base_currency = ? AND quote_currency = ?",
                    params![to.code(), from.code()],
                    |row| {
                        let inverse_rate = Self::parse_decimal(&row.get::<_, String>(3)?);
                        let rate = if inverse_rate.is_zero() {
                            Decimal::ZERO
                        } else {
                            Decimal::ONE / inverse_rate
                        };

                        Ok(ExchangeRate {
                            id: Uuid::new_v4(), // Generated for inverse
                            base_currency: from,
                            quote_currency: to,
                            rate,
                            source: format!("inverse:{}", row.get::<_, String>(4)?),
                            rate_at: Self::parse_datetime(&row.get::<_, String>(5)?),
                            created_at: Utc::now(),
                            updated_at: Utc::now(),
                        })
                    },
                );

                match inverse_result {
                    Ok(rate) => Ok(Some(rate)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(map_db_error(e)),
                }
            }
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
                 FROM exchange_rates
                 WHERE base_currency = ?
                 ORDER BY quote_currency",
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map(params![base.code()], |row| {
                Ok(ExchangeRate {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    base_currency: Self::parse_currency(&row.get::<_, String>(1)?).unwrap_or_default(),
                    quote_currency: Self::parse_currency(&row.get::<_, String>(2)?).unwrap_or_default(),
                    rate: Self::parse_decimal(&row.get::<_, String>(3)?),
                    source: row.get(4)?,
                    rate_at: Self::parse_datetime(&row.get::<_, String>(5)?),
                    created_at: Self::parse_datetime(&row.get::<_, String>(6)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(7)?),
                })
            })
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)
    }

    fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at
             FROM exchange_rates WHERE 1=1",
        );
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(base) = &filter.base_currency {
            query.push_str(" AND base_currency = ?");
            params_vec.push(base.code().to_string());
        }

        if let Some(quote) = &filter.quote_currency {
            query.push_str(" AND quote_currency = ?");
            params_vec.push(quote.code().to_string());
        }

        if let Some(since) = &filter.since {
            query.push_str(" AND rate_at >= ?");
            params_vec.push(since.to_rfc3339());
        }

        query.push_str(" ORDER BY base_currency, quote_currency");

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt
            .query_map(params.as_slice(), |row| {
                Ok(ExchangeRate {
                    id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
                    base_currency: Self::parse_currency(&row.get::<_, String>(1)?).unwrap_or_default(),
                    quote_currency: Self::parse_currency(&row.get::<_, String>(2)?).unwrap_or_default(),
                    rate: Self::parse_decimal(&row.get::<_, String>(3)?),
                    source: row.get(4)?,
                    rate_at: Self::parse_datetime(&row.get::<_, String>(5)?),
                    created_at: Self::parse_datetime(&row.get::<_, String>(6)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(7)?),
                })
            })
            .map_err(map_db_error)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)
    }

    fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let source = input.source.unwrap_or_else(|| "manual".into());

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            // Upsert the rate
            conn.execute(
                "INSERT INTO exchange_rates (id, base_currency, quote_currency, rate, source, rate_at, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (base_currency, quote_currency) DO UPDATE SET
                    rate = excluded.rate,
                    source = excluded.source,
                    rate_at = excluded.rate_at,
                    updated_at = excluded.updated_at",
                params![
                    id.to_string(),
                    input.base_currency.code(),
                    input.quote_currency.code(),
                    input.rate.to_string(),
                    source,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    now.to_rfc3339()
                ],
            )
            .map_err(map_db_error)?;

            // Record in history
            conn.execute(
                "INSERT INTO exchange_rate_history (id, base_currency, quote_currency, rate, source, rate_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    Uuid::new_v4().to_string(),
                    input.base_currency.code(),
                    input.quote_currency.code(),
                    input.rate.to_string(),
                    source,
                    now.to_rfc3339()
                ],
            )
            .map_err(map_db_error)?;
        }

        // Fetch and return the rate
        self.get_rate(input.base_currency, input.quote_currency)?
            .ok_or(CommerceError::NotFound)
    }

    fn set_rates(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>> {
        rates.into_iter().map(|r| self.set_rate(r)).collect()
    }

    fn delete_rate(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let affected = conn
            .execute("DELETE FROM exchange_rates WHERE id = ?", params![id.to_string()])
            .map_err(map_db_error)?;

        if affected == 0 {
            Err(CommerceError::NotFound)
        } else {
            Ok(())
        }
    }

    fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        // Same currency = no conversion needed
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
            .get_rate(input.from, input.to)?
            .ok_or(CommerceError::ValidationError(format!(
                "No exchange rate found for {} to {}",
                input.from, input.to
            )))?;

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

    fn get_settings(&self) -> Result<StoreCurrencySettings> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let result = conn.query_row(
            "SELECT base_currency, enabled_currencies, auto_convert, rounding_mode
             FROM store_currency_settings
             WHERE id = 'default'",
            [],
            |row| {
                let base_currency = Self::parse_currency(&row.get::<_, String>(0)?).unwrap_or_default();
                let enabled_json: String = row.get(1)?;
                let enabled_currencies: Vec<Currency> = serde_json::from_str(&enabled_json)
                    .unwrap_or_else(|_| vec![Currency::USD, Currency::EUR, Currency::GBP]);
                let auto_convert: bool = row.get::<_, i32>(2)? != 0;
                let rounding_str: String = row.get(3)?;
                let rounding_mode = match rounding_str.as_str() {
                    "half_down" => RoundingMode::HalfDown,
                    "up" => RoundingMode::Up,
                    "down" => RoundingMode::Down,
                    "half_even" => RoundingMode::HalfEven,
                    _ => RoundingMode::HalfUp,
                };

                Ok(StoreCurrencySettings {
                    base_currency,
                    enabled_currencies,
                    auto_convert,
                    rounding_mode,
                })
            },
        );

        match result {
            Ok(settings) => Ok(settings),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StoreCurrencySettings::default()),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings> {
        let enabled_json = serde_json::to_string(&settings.enabled_currencies)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rounding_str = match settings.rounding_mode {
            RoundingMode::HalfUp => "half_up",
            RoundingMode::HalfDown => "half_down",
            RoundingMode::Up => "up",
            RoundingMode::Down => "down",
            RoundingMode::HalfEven => "half_even",
        };

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            conn.execute(
                "INSERT INTO store_currency_settings (id, base_currency, enabled_currencies, auto_convert, rounding_mode, updated_at)
                 VALUES ('default', ?, ?, ?, ?, datetime('now'))
                 ON CONFLICT (id) DO UPDATE SET
                    base_currency = excluded.base_currency,
                    enabled_currencies = excluded.enabled_currencies,
                    auto_convert = excluded.auto_convert,
                    rounding_mode = excluded.rounding_mode,
                    updated_at = excluded.updated_at",
                params![
                    settings.base_currency.code(),
                    enabled_json,
                    settings.auto_convert as i32,
                    rounding_str
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get_settings()
    }
}
