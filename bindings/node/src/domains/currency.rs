//! Currency API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Currency API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SetExchangeRateInput {
    /// Base currency code (e.g., "USD")
    pub base_currency: String,
    /// Quote currency code (e.g., "EUR")
    pub quote_currency: String,
    /// Exchange rate (e.g., 0.92 for USD to EUR)
    pub rate: f64,
    /// Optional source of the rate (e.g., "manual", "api")
    pub source: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ConvertCurrencyInput {
    /// Source currency code
    pub from: String,
    /// Target currency code
    pub to: String,
    /// Amount to convert
    pub amount: f64,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ExchangeRateFilterInput {
    /// Filter by base currency
    pub base_currency: Option<String>,
    /// Filter by quote currency
    pub quote_currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ExchangeRateOutput {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: f64,
    pub source: String,
    pub rate_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct ConversionResultOutput {
    /// @deprecated Use the `originalAmountExact` twin; float money will be removed in 2.0.
    pub original_amount: f64,
    /// Exact base-10 original amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub original_amount_exact: String,
    pub original_currency: String,
    /// @deprecated Use the `convertedAmountExact` twin; float money will be removed in 2.0.
    pub converted_amount: f64,
    /// Exact base-10 converted amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub converted_amount_exact: String,
    pub target_currency: String,
    pub rate: f64,
    pub inverse_rate: f64,
    pub rate_at: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCurrencySettingsInput {
    /// Base currency for the store
    pub base_currency: String,
    /// List of enabled currency codes
    pub enabled_currencies: Vec<String>,
    /// Whether to auto-convert prices
    pub auto_convert: Option<bool>,
    /// Rounding mode; an unrecognised value is refused with `VALIDATION`.
    #[napi(ts_type = "RoundingMode")]
    pub rounding_mode: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Clone)]
pub struct StoreCurrencySettingsOutput {
    pub base_currency: String,
    pub enabled_currencies: Vec<String>,
    pub auto_convert: bool,
    #[napi(ts_type = "RoundingMode")]
    pub rounding_mode: String,
}

pub(crate) fn parse_currency(code: &str) -> Result<stateset_embedded::Currency> {
    use std::str::FromStr;
    stateset_embedded::Currency::from_str(code)
        .map_err(|e| coded(ErrCode::Validation, format!("Invalid currency code '{}': {}", code, e)))
}

pub(crate) fn parse_rounding_mode(mode: &str) -> Result<stateset_embedded::RoundingMode> {
    Ok(match mode.to_lowercase().as_str() {
        "half_up" => stateset_embedded::RoundingMode::HalfUp,
        "half_down" => stateset_embedded::RoundingMode::HalfDown,
        "up" => stateset_embedded::RoundingMode::Up,
        "down" => stateset_embedded::RoundingMode::Down,
        "half_even" => stateset_embedded::RoundingMode::HalfEven,
        _ => {
            return Err(unknown_variant(
                "rounding mode",
                mode,
                &["half_up", "half_down", "up", "down", "half_even"],
            ));
        }
    })
}

pub(crate) fn rounding_mode_to_string(mode: &stateset_embedded::RoundingMode) -> String {
    match mode {
        stateset_embedded::RoundingMode::HalfUp => "half_up".to_string(),
        stateset_embedded::RoundingMode::HalfDown => "half_down".to_string(),
        stateset_embedded::RoundingMode::Up => "up".to_string(),
        stateset_embedded::RoundingMode::Down => "down".to_string(),
        stateset_embedded::RoundingMode::HalfEven => "half_even".to_string(),
        &_ => "half_up".to_string(),
    }
}

pub(crate) fn exchange_rate_to_output(
    rate: stateset_embedded::ExchangeRate,
) -> Result<ExchangeRateOutput> {
    Ok(ExchangeRateOutput {
        id: rate.id.to_string(),
        base_currency: rate.base_currency.code().to_string(),
        quote_currency: rate.quote_currency.code().to_string(),
        rate: to_f64_checked(rate.rate, "exchange rate")?,
        source: rate.source,
        rate_at: rate.rate_at.to_rfc3339(),
        created_at: rate.created_at.to_rfc3339(),
        updated_at: rate.updated_at.to_rfc3339(),
    })
}

/// Currency and exchange rate operations API
#[napi]
pub struct CurrencyOperations {
    pub(crate) commerce: Handle,
}

#[napi]
impl CurrencyOperations {
    /// Get exchange rate between two currencies
    #[napi]
    pub async fn get_rate(&self, from: String, to: String) -> Result<Option<ExchangeRateOutput>> {
        let commerce = self.commerce.get()?;
        let from_currency = parse_currency(&from)?;
        let to_currency = parse_currency(&to)?;

        let rate = commerce
            .currency()
            .get_rate(from_currency, to_currency)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get rate", e))?;

        rate.map(exchange_rate_to_output).transpose()
    }

    /// Get all exchange rates for a base currency
    #[napi]
    pub async fn get_rates_for(&self, base_currency: String) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.get()?;
        let currency = parse_currency(&base_currency)?;

        let rates = commerce
            .currency()
            .get_rates_for(currency)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get rates", e))?;

        rates.into_iter().map(exchange_rate_to_output).collect()
    }

    /// List exchange rates with optional filtering
    #[napi]
    pub async fn list_rates(
        &self,
        filter: Option<ExchangeRateFilterInput>,
    ) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.get()?;

        let mut f = stateset_embedded::ExchangeRateFilter::default();
        if let Some(ref input) = filter {
            if let Some(ref base) = input.base_currency {
                f.base_currency = Some(parse_currency(base)?);
            }
            if let Some(ref quote) = input.quote_currency {
                f.quote_currency = Some(parse_currency(quote)?);
            }
        }

        let rates = commerce
            .currency()
            .list_rates(f)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list rates", e))?;

        rates.into_iter().map(exchange_rate_to_output).collect()
    }

    /// Set an exchange rate
    #[napi]
    pub async fn set_rate(&self, input: SetExchangeRateInput) -> Result<ExchangeRateOutput> {
        let commerce = self.commerce.get()?;

        let rate = commerce
            .currency()
            .set_rate(stateset_embedded::SetExchangeRate {
                base_currency: parse_currency(&input.base_currency)?,
                quote_currency: parse_currency(&input.quote_currency)?,
                rate: Decimal::try_from(input.rate)
                    .map_err(|e| wrap(ErrCode::Validation, "Invalid rate", e))?,
                source: input.source,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set rate", e))?;

        exchange_rate_to_output(rate)
    }

    /// Set multiple exchange rates at once
    #[napi]
    pub async fn set_rates(
        &self,
        inputs: Vec<SetExchangeRateInput>,
    ) -> Result<Vec<ExchangeRateOutput>> {
        let commerce = self.commerce.get()?;

        let mut rates = Vec::new();
        for input in inputs {
            rates.push(stateset_embedded::SetExchangeRate {
                base_currency: parse_currency(&input.base_currency)?,
                quote_currency: parse_currency(&input.quote_currency)?,
                rate: Decimal::try_from(input.rate)
                    .map_err(|e| wrap(ErrCode::Validation, "Invalid rate", e))?,
                source: input.source,
            });
        }

        let results = commerce
            .currency()
            .set_rates(rates)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set rates", e))?;

        results.into_iter().map(exchange_rate_to_output).collect()
    }

    /// Delete an exchange rate by ID
    #[napi]
    pub async fn delete_rate(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let rate_id = uuid::Uuid::parse_str(&id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid rate ID", e))?;

        commerce
            .currency()
            .delete_rate(rate_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete rate", e))?;

        Ok(true)
    }

    /// Convert an amount from one currency to another
    #[napi]
    pub async fn convert(&self, input: ConvertCurrencyInput) -> Result<ConversionResultOutput> {
        let commerce = self.commerce.get()?;

        let result = commerce
            .currency()
            .convert(stateset_embedded::ConvertCurrency {
                from: parse_currency(&input.from)?,
                to: parse_currency(&input.to)?,
                amount: Decimal::try_from(input.amount)
                    .map_err(|e| wrap(ErrCode::Validation, "Invalid amount", e))?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to convert currency", e))?;

        let (original_amount, original_amount_exact) =
            money_pair(result.original_amount, "conversion original amount")?;
        let (converted_amount, converted_amount_exact) =
            money_pair(result.converted_amount, "conversion converted amount")?;
        Ok(ConversionResultOutput {
            original_amount,
            original_amount_exact,
            original_currency: result.original_currency.code().to_string(),
            converted_amount,
            converted_amount_exact,
            target_currency: result.target_currency.code().to_string(),
            rate: to_f64_checked(result.rate, "conversion rate")?,
            inverse_rate: to_f64_checked(result.inverse_rate, "conversion inverse rate")?,
            rate_at: result.rate_at.to_rfc3339(),
        })
    }

    /// Get store currency settings
    #[napi]
    pub async fn get_settings(&self) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.get()?;

        let settings = commerce
            .currency()
            .get_settings()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get settings", e))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Update store currency settings
    #[napi]
    pub async fn update_settings(
        &self,
        input: StoreCurrencySettingsInput,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.get()?;

        let mut enabled = Vec::new();
        for code in &input.enabled_currencies {
            enabled.push(parse_currency(code)?);
        }

        let settings = commerce
            .currency()
            .update_settings(stateset_embedded::StoreCurrencySettings {
                base_currency: parse_currency(&input.base_currency)?,
                enabled_currencies: enabled,
                auto_convert: input.auto_convert.unwrap_or(true),
                rounding_mode: input
                    .rounding_mode
                    .as_deref()
                    .map(parse_rounding_mode)
                    .transpose()?
                    .unwrap_or_default(),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update settings", e))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Set the store's base currency
    #[napi]
    pub async fn set_base_currency(
        &self,
        currency_code: String,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.get()?;
        let currency = parse_currency(&currency_code)?;

        let settings = commerce
            .currency()
            .set_base_currency(currency)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set base currency", e))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Enable currencies for the store
    #[napi]
    pub async fn enable_currencies(
        &self,
        currency_codes: Vec<String>,
    ) -> Result<StoreCurrencySettingsOutput> {
        let commerce = self.commerce.get()?;

        let mut currencies = Vec::new();
        for code in &currency_codes {
            currencies.push(parse_currency(code)?);
        }

        let settings = commerce
            .currency()
            .enable_currencies(currencies)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to enable currencies", e))?;

        Ok(StoreCurrencySettingsOutput {
            base_currency: settings.base_currency.code().to_string(),
            enabled_currencies: settings
                .enabled_currencies
                .iter()
                .map(|c| c.code().to_string())
                .collect(),
            auto_convert: settings.auto_convert,
            rounding_mode: rounding_mode_to_string(&settings.rounding_mode),
        })
    }

    /// Check if a currency is enabled
    #[napi]
    pub async fn is_enabled(&self, currency_code: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let currency = parse_currency(&currency_code)?;

        commerce
            .currency()
            .is_enabled(currency)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check currency", e))
    }

    /// Get the store's base currency
    #[napi]
    pub async fn get_base_currency(&self) -> Result<String> {
        let commerce = self.commerce.get()?;

        let currency = commerce
            .currency()
            .base_currency()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get base currency", e))?;

        Ok(currency.code().to_string())
    }

    /// Get all enabled currencies
    #[napi]
    pub async fn get_enabled_currencies(&self) -> Result<Vec<String>> {
        let commerce = self.commerce.get()?;

        let currencies = commerce
            .currency()
            .enabled_currencies()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get enabled currencies", e))?;

        Ok(currencies.iter().map(|c| c.code().to_string()).collect())
    }

    /// How many decimal places a currency permits.
    ///
    /// The engine refuses an amount with more places than this, so a caller
    /// formatting or validating money needs the number rather than assuming
    /// two: JPY, KRW and VND have none, BTC and ETH have eight.
    #[napi]
    pub fn decimal_places(&self, currency_code: String) -> Result<u32> {
        Ok(u32::from(parse_currency(&currency_code)?.decimal_places()))
    }

    /// Format an amount with currency symbol
    #[napi]
    pub async fn format(&self, amount: f64, currency_code: String) -> Result<String> {
        let commerce = self.commerce.get()?;
        let currency = parse_currency(&currency_code)?;
        let amount_decimal = Decimal::try_from(amount)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid amount", e))?;

        Ok(commerce.currency().format(amount_decimal, currency))
    }
}
