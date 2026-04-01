//! Advanced forecasting and anomaly detection models
//!
//! Provides time-series forecasting with seasonality detection and anomaly detection
//! capabilities for sales, inventory, and demand planning.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Helper macro for decimal literals
macro_rules! dec {
    (0) => {
        Decimal::ZERO
    };
    (1) => {
        Decimal::ONE
    };
    ($val:expr) => {
        Decimal::from($val as i64)
    };
}

// ============================================================================
// Seasonality Detection
// ============================================================================

/// Detected seasonality patterns in data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityPattern {
    /// Type of seasonality detected
    pub seasonality_type: SeasonalityType,
    /// Seasonal indices (multipliers for each period)
    pub indices: Vec<SeasonalIndex>,
    /// Strength of seasonality (0-1, higher = stronger pattern)
    pub strength: Decimal,
    /// Statistical significance (p-value)
    pub significance: Decimal,
}

/// Type of seasonality pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SeasonalityType {
    /// Daily pattern (hour of day)
    Hourly,
    /// Weekly pattern (day of week)
    Weekly,
    /// Monthly pattern (day of month)
    Monthly,
    /// Quarterly pattern
    Quarterly,
    /// Yearly pattern (month of year)
    Yearly,
    /// No significant seasonality
    None,
}

impl Default for SeasonalityType {
    fn default() -> Self {
        Self::None
    }
}

/// Seasonal index for a specific period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalIndex {
    /// Period identifier (e.g., "Monday", "January", "Q1")
    pub period: String,
    /// Period number (0-indexed)
    pub period_num: u32,
    /// Seasonal index (multiplier, 1.0 = average)
    pub index: Decimal,
    /// Confidence interval lower bound
    pub ci_lower: Decimal,
    /// Confidence interval upper bound
    pub ci_upper: Decimal,
}

/// Request for seasonality analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeasonalityAnalysisRequest {
    /// Time series data points
    pub data: Vec<TimeSeriesPoint>,
    /// Seasonality types to test
    pub test_types: Option<Vec<SeasonalityType>>,
    /// Minimum data points required
    pub min_periods: Option<u32>,
}

/// Single point in a time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Value at this point
    pub value: Decimal,
}

/// Result of seasonality analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityAnalysisResult {
    /// Detected patterns (may be multiple)
    pub patterns: Vec<SeasonalityPattern>,
    /// Primary seasonality type
    pub primary_seasonality: SeasonalityType,
    /// Deseasonalized (trend) component
    pub trend_component: Vec<Decimal>,
    /// Seasonal component
    pub seasonal_component: Vec<Decimal>,
    /// Residual (noise) component
    pub residual_component: Vec<Decimal>,
}

// ============================================================================
// Anomaly Detection
// ============================================================================

/// Detected anomaly in data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Unique identifier
    pub id: String,
    /// When the anomaly occurred
    pub timestamp: DateTime<Utc>,
    /// Actual observed value
    pub actual_value: Decimal,
    /// Expected value based on model
    pub expected_value: Decimal,
    /// Deviation from expected (as percentage)
    pub deviation_percent: Decimal,
    /// Anomaly score (0-1, higher = more anomalous)
    pub anomaly_score: Decimal,
    /// Type of anomaly
    pub anomaly_type: AnomalyType,
    /// Severity level
    pub severity: AnomalySeverity,
    /// Contextual information
    pub context: Option<AnomalyContext>,
}

/// Type of anomaly detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnomalyType {
    /// Sudden spike upward
    Spike,
    /// Sudden drop downward
    Drop,
    /// Level shift (sustained change)
    LevelShift,
    /// Trend change
    TrendChange,
    /// Variance change
    VarianceChange,
    /// Seasonality deviation
    SeasonalAnomaly,
    /// Missing expected pattern
    PatternBreak,
}

/// Severity of anomaly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnomalySeverity {
    /// Minor deviation, likely noise
    Low,
    /// Notable deviation, worth monitoring
    Medium,
    /// Significant deviation, requires attention
    High,
    /// Critical deviation, immediate action needed
    Critical,
}

/// Additional context for an anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyContext {
    /// Related metric that may explain anomaly
    pub related_metric: Option<String>,
    /// Historical frequency of similar anomalies
    pub historical_frequency: Option<Decimal>,
    /// Possible causes
    pub possible_causes: Vec<String>,
    /// Recommended actions
    pub recommendations: Vec<String>,
}

/// Request for anomaly detection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnomalyDetectionRequest {
    /// Time series data to analyze
    pub data: Vec<TimeSeriesPoint>,
    /// Sensitivity level (0-1, higher = more sensitive)
    pub sensitivity: Option<Decimal>,
    /// Detection method to use
    pub method: Option<AnomalyDetectionMethod>,
    /// Look for specific anomaly types only
    pub anomaly_types: Option<Vec<AnomalyType>>,
    /// Include historical context
    pub include_context: Option<bool>,
}

/// Method for anomaly detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AnomalyDetectionMethod {
    /// Statistical z-score based
    ZScore,
    /// Interquartile range based
    Iqr,
    /// Moving average deviation
    MovingAverage,
    /// Exponential smoothing
    ExponentialSmoothing,
    /// Isolation forest (ML-based)
    IsolationForest,
    /// Ensemble of methods
    Ensemble,
}

impl Default for AnomalyDetectionMethod {
    fn default() -> Self {
        Self::Ensemble
    }
}

/// Result of anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionResult {
    /// Detected anomalies
    pub anomalies: Vec<Anomaly>,
    /// Overall anomaly rate
    pub anomaly_rate: Decimal,
    /// Model parameters used
    pub model_params: AnomalyModelParams,
    /// Summary statistics
    pub summary: AnomalySummary,
}

/// Parameters of the anomaly detection model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyModelParams {
    /// Detection method used
    pub method: AnomalyDetectionMethod,
    /// Threshold for anomaly classification
    pub threshold: Decimal,
    /// Lookback window size
    pub window_size: u32,
    /// Sensitivity used
    pub sensitivity: Decimal,
}

/// Summary of anomaly detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalySummary {
    /// Total points analyzed
    pub total_points: u64,
    /// Number of anomalies detected
    pub anomaly_count: u64,
    /// Anomalies by type
    pub by_type: HashMap<String, u64>,
    /// Anomalies by severity
    pub by_severity: HashMap<String, u64>,
    /// Time periods with most anomalies
    pub hotspots: Vec<String>,
}

// ============================================================================
// Enhanced Forecasting
// ============================================================================

/// Enhanced forecast with confidence intervals and decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedForecast {
    /// SKU or metric being forecasted
    pub identifier: String,
    /// Forecast horizon
    pub horizon_days: u32,
    /// Point forecasts
    pub forecasts: Vec<ForecastPoint>,
    /// Model used
    pub model: ForecastModel,
    /// Model accuracy metrics
    pub accuracy: ForecastAccuracy,
    /// Detected seasonality
    pub seasonality: Option<SeasonalityPattern>,
    /// Recent anomalies that may affect forecast
    pub recent_anomalies: Vec<Anomaly>,
}

/// Single forecast point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    /// Date being forecasted
    pub date: NaiveDate,
    /// Point forecast (best estimate)
    pub forecast: Decimal,
    /// Lower bound (e.g., 5th percentile)
    pub lower_bound: Decimal,
    /// Upper bound (e.g., 95th percentile)
    pub upper_bound: Decimal,
    /// Trend component
    pub trend: Decimal,
    /// Seasonal component
    pub seasonal: Decimal,
    /// Confidence level for bounds
    pub confidence_level: Decimal,
}

/// Forecasting model type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ForecastModel {
    /// Simple moving average
    MovingAverage,
    /// Exponential smoothing (ETS)
    ExponentialSmoothing,
    /// Holt-Winters (seasonal exponential smoothing)
    HoltWinters,
    /// Linear regression with trend
    LinearRegression,
    /// Seasonal decomposition
    SeasonalDecomposition,
    /// Ensemble of models
    Ensemble,
}

impl Default for ForecastModel {
    fn default() -> Self {
        Self::HoltWinters
    }
}

/// Forecast accuracy metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForecastAccuracy {
    /// Mean Absolute Error
    pub mae: Decimal,
    /// Mean Absolute Percentage Error
    pub mape: Decimal,
    /// Root Mean Square Error
    pub rmse: Decimal,
    /// Mean Error (bias)
    pub me: Decimal,
    /// R-squared (coefficient of determination)
    pub r_squared: Decimal,
    /// Number of periods used for validation
    pub validation_periods: u32,
}

/// Request for enhanced forecast
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnhancedForecastRequest {
    /// Historical data
    pub historical_data: Vec<TimeSeriesPoint>,
    /// Days to forecast ahead
    pub horizon_days: u32,
    /// Model to use (or auto-select)
    pub model: Option<ForecastModel>,
    /// Confidence level for intervals (e.g., 0.95)
    pub confidence_level: Option<Decimal>,
    /// Include seasonality analysis
    pub include_seasonality: Option<bool>,
    /// Include anomaly detection
    pub include_anomalies: Option<bool>,
}

// ============================================================================
// Forecasting Algorithms
// ============================================================================

/// Simple forecasting utilities
#[derive(Debug)]
pub struct ForecastingEngine;

impl ForecastingEngine {
    /// Calculate simple moving average
    #[must_use]
    pub fn moving_average(data: &[Decimal], window: usize) -> Vec<Decimal> {
        if data.len() < window {
            return vec![];
        }

        data.windows(window)
            .map(|w| {
                let sum: Decimal = w.iter().copied().sum();
                sum / Decimal::from(window as u32)
            })
            .collect()
    }

    /// Calculate exponential moving average
    #[must_use]
    pub fn exponential_moving_average(data: &[Decimal], alpha: Decimal) -> Vec<Decimal> {
        if data.is_empty() {
            return vec![];
        }

        let mut ema = vec![data[0]];
        for i in 1..data.len() {
            let new_ema = alpha * data[i] + (dec!(1) - alpha) * ema[i - 1];
            ema.push(new_ema);
        }
        ema
    }

    /// Detect weekly seasonality from daily data
    #[must_use]
    pub fn detect_weekly_seasonality(data: &[TimeSeriesPoint]) -> Option<SeasonalityPattern> {
        if data.len() < 14 {
            // Need at least 2 weeks of data
            return None;
        }

        // Group by day of week
        let mut day_sums: [Decimal; 7] = [dec!(0); 7];
        let mut day_counts: [u32; 7] = [0; 7];

        for point in data {
            let dow = point.timestamp.weekday().num_days_from_monday() as usize;
            day_sums[dow] += point.value;
            day_counts[dow] += 1;
        }

        // Calculate overall average
        let total: Decimal = day_sums.iter().copied().sum();
        let count: u32 = day_counts.iter().sum();
        if count == 0 {
            return None;
        }
        let overall_avg = total / Decimal::from(count);

        if overall_avg == dec!(0) {
            return None;
        }

        // Calculate seasonal indices
        let day_names =
            ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
        let mut indices = Vec::new();
        let mut variance_sum = dec!(0);

        for i in 0..7 {
            let day_avg = if day_counts[i] > 0 {
                day_sums[i] / Decimal::from(day_counts[i])
            } else {
                overall_avg
            };

            let index = day_avg / overall_avg;
            let deviation = index - dec!(1);
            variance_sum += deviation * deviation;

            indices.push(SeasonalIndex {
                period: day_names[i].to_string(),
                period_num: i as u32,
                index,
                ci_lower: index * dec!(0.9),
                ci_upper: index * dec!(1.1),
            });
        }

        // Calculate strength (coefficient of variation of indices)
        // Use simple approximation instead of sqrt since it requires maths feature
        let strength = variance_sum / Decimal::from(7);

        Some(SeasonalityPattern {
            seasonality_type: SeasonalityType::Weekly,
            indices,
            strength: strength.min(dec!(1)),
            significance: if strength > dec!(0.1) { dec!(0.05) } else { dec!(0.5) },
        })
    }

    /// Detect anomalies using z-score method
    #[must_use]
    pub fn detect_anomalies_zscore(data: &[TimeSeriesPoint], threshold: Decimal) -> Vec<Anomaly> {
        if data.len() < 3 {
            return vec![];
        }

        let values: Vec<Decimal> = data.iter().map(|p| p.value).collect();
        let mean = Self::mean(&values);
        let std_dev = Self::std_dev(&values, mean);

        if std_dev == dec!(0) {
            return vec![];
        }

        let mut anomalies = Vec::new();
        for (i, point) in data.iter().enumerate() {
            let z_score = (point.value - mean) / std_dev;
            let abs_z = if z_score < dec!(0) { -z_score } else { z_score };

            if abs_z > threshold {
                let deviation_percent = ((point.value - mean) / mean * dec!(100)).round_dp(2);

                let anomaly_type =
                    if z_score > dec!(0) { AnomalyType::Spike } else { AnomalyType::Drop };

                let severity = if abs_z > threshold * dec!(2) {
                    AnomalySeverity::Critical
                } else if abs_z > threshold * dec!(1.5) {
                    AnomalySeverity::High
                } else if abs_z > threshold * dec!(1.2) {
                    AnomalySeverity::Medium
                } else {
                    AnomalySeverity::Low
                };

                anomalies.push(Anomaly {
                    id: format!("anomaly-{i}"),
                    timestamp: point.timestamp,
                    actual_value: point.value,
                    expected_value: mean,
                    deviation_percent,
                    anomaly_score: (abs_z / dec!(5)).min(dec!(1)),
                    anomaly_type,
                    severity,
                    context: None,
                });
            }
        }

        anomalies
    }

    /// Calculate mean of values
    #[must_use]
    pub fn mean(values: &[Decimal]) -> Decimal {
        if values.is_empty() {
            return dec!(0);
        }
        let sum: Decimal = values.iter().copied().sum();
        sum / Decimal::from(values.len() as u32)
    }

    /// Calculate standard deviation
    #[must_use]
    pub fn std_dev(values: &[Decimal], mean: Decimal) -> Decimal {
        if values.len() < 2 {
            return dec!(0);
        }
        let variance: Decimal = values
            .iter()
            .map(|v| {
                let diff = *v - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from((values.len() - 1) as u32);

        // Babylonian method approximation for sqrt
        Self::sqrt_approx(variance)
    }

    /// Approximate square root using Babylonian/Newton's method
    fn sqrt_approx(n: Decimal) -> Decimal {
        if n <= dec!(0) {
            return dec!(0);
        }
        // Initial guess
        let mut x = n / dec!(2);
        if x == dec!(0) {
            x = dec!(1);
        }
        // Iterate for convergence
        for _ in 0..20 {
            let next = (x + n / x) / dec!(2);
            if (next - x).abs() < dec!(0.0001) {
                return next.round_dp(4);
            }
            x = next;
        }
        x.round_dp(4)
    }

    /// Linear regression for trend
    #[must_use]
    pub fn linear_trend(values: &[Decimal]) -> (Decimal, Decimal) {
        let n = values.len();
        if n < 2 {
            return (dec!(0), dec!(0));
        }

        let n_dec = Decimal::from(n as u32);
        let sum_x: Decimal = (0..n).map(|i| Decimal::from(i as u32)).sum();
        let sum_y: Decimal = values.iter().copied().sum();
        let sum_xy: Decimal =
            values.iter().enumerate().map(|(i, v)| Decimal::from(i as u32) * *v).sum();
        let sum_xx: Decimal = (0..n)
            .map(|i| {
                let x = Decimal::from(i as u32);
                x * x
            })
            .sum();

        let denominator = n_dec * sum_xx - sum_x * sum_x;
        if denominator == dec!(0) {
            return (Self::mean(values), dec!(0));
        }

        let slope = (n_dec * sum_xy - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / n_dec;

        (intercept, slope)
    }

    /// Generate forecast using Holt's linear trend method
    #[must_use]
    pub fn holt_forecast(
        values: &[Decimal],
        alpha: Decimal,
        beta: Decimal,
        periods: u32,
    ) -> Vec<Decimal> {
        if values.is_empty() {
            return vec![];
        }

        // Initialize
        let mut level = values[0];
        let mut trend = if values.len() > 1 { values[1] - values[0] } else { dec!(0) };

        // Update level and trend for historical data
        for value in values.iter().skip(1) {
            let prev_level = level;
            level = alpha * value + (dec!(1) - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (dec!(1) - beta) * trend;
        }

        // Generate forecasts
        (1..=periods).map(|h| level + Decimal::from(h) * trend).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(val: i64) -> Decimal {
        Decimal::from(val)
    }

    #[test]
    fn test_moving_average() {
        let data = vec![d(10), d(20), d(30), d(40), d(50)];
        let ma = ForecastingEngine::moving_average(&data, 3);
        assert_eq!(ma.len(), 3);
        assert_eq!(ma[0], d(20)); // (10+20+30)/3
        assert_eq!(ma[1], d(30)); // (20+30+40)/3
        assert_eq!(ma[2], d(40)); // (30+40+50)/3
    }

    #[test]
    fn test_mean() {
        let values = vec![d(10), d(20), d(30)];
        assert_eq!(ForecastingEngine::mean(&values), d(20));
    }

    #[test]
    fn test_linear_trend() {
        // Perfect linear data: 10, 20, 30, 40
        let values = vec![d(10), d(20), d(30), d(40)];
        let (intercept, slope) = ForecastingEngine::linear_trend(&values);
        assert_eq!(slope, d(10));
        assert_eq!(intercept, d(10));
    }

    #[test]
    fn test_anomaly_detection() {
        use chrono::TimeZone;

        let data: Vec<TimeSeriesPoint> = vec![
            TimeSeriesPoint {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
                value: d(100),
            },
            TimeSeriesPoint {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 2, 0, 0, 0).unwrap(),
                value: d(102),
            },
            TimeSeriesPoint {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 3, 0, 0, 0).unwrap(),
                value: d(98),
            },
            TimeSeriesPoint {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 4, 0, 0, 0).unwrap(),
                value: d(500),
            }, // Anomaly!
            TimeSeriesPoint {
                timestamp: Utc.with_ymd_and_hms(2024, 1, 5, 0, 0, 0).unwrap(),
                value: d(101),
            },
        ];

        // z-score for 500 is about 1.79 given the data spread, so use threshold 1.5
        let anomalies = ForecastingEngine::detect_anomalies_zscore(&data, dec!(1.5));
        assert!(!anomalies.is_empty(), "Expected to detect 500 as an anomaly");
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::Spike);
    }
}
