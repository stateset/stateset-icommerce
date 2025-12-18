//! PostgreSQL analytics repository implementation
//!
//! Read-only analytics queries against existing commerce tables.

use super::map_db_error;
use chrono::{DateTime, Datelike, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use stateset_core::{
    AnalyticsQuery, AnalyticsRepository, CommerceError, CustomerMetrics, DemandForecast,
    FulfillmentMetrics, InventoryHealth, InventoryMovement, LowStockItem, OrderStatusBreakdown,
    ProductPerformance, Result, ReturnMetrics, ReturnReasonCount, RevenueByPeriod,
    RevenueForecast, SalesSummary, TimeGranularity, TimePeriod, TopCustomer, TopProduct,
    Trend,
};
use uuid::Uuid;

/// PostgreSQL analytics repository
pub struct PgAnalyticsRepository {
    pool: PgPool,
}

impl PgAnalyticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get date range from query parameters
    fn get_date_range(&self, query: &AnalyticsQuery) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let period = query.period.unwrap_or(TimePeriod::Last30Days);

        match period {
            TimePeriod::Today => (
                now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
                now,
            ),
            TimePeriod::Yesterday => {
                let yesterday = now - Duration::days(1);
                (
                    yesterday
                        .date_naive()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                    yesterday
                        .date_naive()
                        .and_hms_opt(23, 59, 59)
                        .unwrap()
                        .and_utc(),
                )
            }
            TimePeriod::Last7Days => (now - Duration::days(7), now),
            TimePeriod::Last30Days => (now - Duration::days(30), now),
            TimePeriod::ThisMonth => {
                let start = now
                    .date_naive()
                    .with_day(1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
                (start, now)
            }
            TimePeriod::LastMonth => {
                let this_month_start = now.date_naive().with_day(1).unwrap();
                let last_month_end = this_month_start - Duration::days(1);
                let last_month_start = last_month_end.with_day(1).unwrap();
                (
                    last_month_start.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                    last_month_end.and_hms_opt(23, 59, 59).unwrap().and_utc(),
                )
            }
            TimePeriod::ThisQuarter | TimePeriod::LastQuarter => {
                (now - Duration::days(90), now)
            }
            TimePeriod::ThisYear => {
                let start = now
                    .date_naive()
                    .with_month(1)
                    .unwrap()
                    .with_day(1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc();
                (start, now)
            }
            TimePeriod::LastYear => (now - Duration::days(365), now),
            TimePeriod::AllTime => (
                DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                now,
            ),
            TimePeriod::Custom => {
                if let Some(ref range) = query.date_range {
                    (
                        range.start.unwrap_or(now - Duration::days(30)),
                        range.end.unwrap_or(now),
                    )
                } else {
                    (now - Duration::days(30), now)
                }
            }
        }
    }

    // Async implementations
    pub async fn get_sales_summary_async(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        let (start, end) = self.get_date_range(&query);

        let row: (Decimal, i64, Decimal, i64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(total_amount), 0) as revenue,
                COUNT(*) as order_count,
                COALESCE(SUM(total_amount) / NULLIF(COUNT(*), 0), 0) as avg_order,
                COUNT(DISTINCT customer_id) as unique_customers
            FROM orders
            WHERE created_at >= $1 AND created_at <= $2
              AND status NOT IN ('cancelled', 'refunded')
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let (revenue, order_count, avg_order, unique_customers) = row;

        let items_sold: i64 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(oi.quantity), 0)
            FROM order_items oi
            JOIN orders o ON oi.order_id = o.id
            WHERE o.created_at >= $1 AND o.created_at <= $2
              AND o.status NOT IN ('cancelled', 'refunded')
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        Ok(SalesSummary {
            total_revenue: revenue,
            order_count: order_count as u64,
            average_order_value: avg_order,
            items_sold: items_sold as u64,
            unique_customers: unique_customers as u64,
            revenue_change_percent: None,
            order_count_change_percent: None,
            period_start: Some(start),
            period_end: Some(end),
        })
    }

    pub async fn get_revenue_by_period_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<RevenueByPeriod>> {
        let (start, end) = self.get_date_range(&query);
        let granularity = query.granularity.unwrap_or(TimeGranularity::Day);

        let date_format = match granularity {
            TimeGranularity::Hour => "YYYY-MM-DD HH24:00",
            TimeGranularity::Day => "YYYY-MM-DD",
            TimeGranularity::Week => "IYYY-\"W\"IW",
            TimeGranularity::Month => "YYYY-MM",
            TimeGranularity::Quarter => "YYYY-\"Q\"Q",
            TimeGranularity::Year => "YYYY",
        };

        let sql = format!(
            r#"
            SELECT
                to_char(created_at, '{}') as period,
                COALESCE(SUM(total_amount), 0) as revenue,
                COUNT(*) as order_count,
                MIN(created_at) as period_start
            FROM orders
            WHERE created_at >= $1 AND created_at <= $2
              AND status NOT IN ('cancelled', 'refunded')
            GROUP BY to_char(created_at, '{}')
            ORDER BY period
            "#,
            date_format, date_format
        );

        let rows: Vec<(String, Decimal, i64, DateTime<Utc>)> = sqlx::query_as(&sql)
            .bind(start)
            .bind(end)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(|(period, revenue, order_count, period_start)| RevenueByPeriod {
                period,
                revenue,
                order_count: order_count as u64,
                period_start,
            })
            .collect())
    }

    pub async fn get_top_products_async(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        let (start, end) = self.get_date_range(&query);
        let limit = query.limit.unwrap_or(10) as i64;

        let rows: Vec<(Option<Uuid>, String, String, i64, Decimal, i64, Decimal)> = sqlx::query_as(
            r#"
            SELECT
                oi.product_id,
                oi.sku,
                oi.name,
                SUM(oi.quantity)::bigint as units_sold,
                SUM(oi.total) as revenue,
                COUNT(DISTINCT oi.order_id)::bigint as order_count,
                AVG(oi.unit_price) as avg_price
            FROM order_items oi
            JOIN orders o ON oi.order_id = o.id
            WHERE o.created_at >= $1 AND o.created_at <= $2
              AND o.status NOT IN ('cancelled', 'refunded')
            GROUP BY oi.product_id, oi.sku, oi.name
            ORDER BY revenue DESC
            LIMIT $3
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(
                |(product_id, sku, name, units_sold, revenue, order_count, avg_price)| TopProduct {
                    product_id,
                    sku,
                    name,
                    units_sold: units_sold as u64,
                    revenue,
                    order_count: order_count as u64,
                    average_price: avg_price,
                },
            )
            .collect())
    }

    pub async fn get_product_performance_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<ProductPerformance>> {
        let top_products = self.get_top_products_async(query).await?;
        Ok(top_products
            .into_iter()
            .map(|p| ProductPerformance {
                product_id: p.product_id.unwrap_or_default(),
                sku: p.sku,
                name: p.name,
                units_sold: p.units_sold,
                revenue: p.revenue,
                previous_units_sold: 0,
                previous_revenue: Decimal::ZERO,
                units_growth_percent: Decimal::ZERO,
                revenue_growth_percent: Decimal::ZERO,
            })
            .collect())
    }

    pub async fn get_customer_metrics_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<CustomerMetrics> {
        let (start, end) = self.get_date_range(&query);

        let total_customers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customers")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let new_customers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM customers WHERE created_at >= $1 AND created_at <= $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let returning_customers: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM (
                SELECT customer_id FROM orders
                GROUP BY customer_id
                HAVING COUNT(*) > 1
            ) t
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let avg_ltv: Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(AVG(total), 0) FROM (
                SELECT customer_id, SUM(total_amount) as total
                FROM orders
                WHERE status NOT IN ('cancelled', 'refunded')
                GROUP BY customer_id
            ) t
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Decimal::ZERO);

        let avg_orders: Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(AVG(cnt), 0) FROM (
                SELECT customer_id, COUNT(*)::numeric as cnt
                FROM orders
                GROUP BY customer_id
            ) t
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Decimal::ZERO);

        Ok(CustomerMetrics {
            total_customers: total_customers as u64,
            new_customers: new_customers as u64,
            returning_customers: returning_customers as u64,
            average_lifetime_value: avg_ltv,
            average_orders_per_customer: avg_orders,
            retention_rate_percent: None,
        })
    }

    pub async fn get_top_customers_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<TopCustomer>> {
        let (start, end) = self.get_date_range(&query);
        let limit = query.limit.unwrap_or(10) as i64;

        let rows: Vec<(
            Uuid,
            String,
            String,
            Decimal,
            i64,
            Decimal,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            r#"
            SELECT
                c.id,
                c.email,
                COALESCE(c.first_name || ' ' || c.last_name, c.email) as name,
                COALESCE(SUM(o.total_amount), 0) as total_spent,
                COUNT(o.id)::bigint as order_count,
                COALESCE(AVG(o.total_amount), 0) as avg_order,
                MIN(o.created_at) as first_order,
                MAX(o.created_at) as last_order
            FROM customers c
            LEFT JOIN orders o ON c.id = o.customer_id
                AND o.status NOT IN ('cancelled', 'refunded')
                AND o.created_at >= $1 AND o.created_at <= $2
            GROUP BY c.id, c.email, c.first_name, c.last_name
            ORDER BY total_spent DESC
            LIMIT $3
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    customer_id,
                    email,
                    name,
                    total_spent,
                    order_count,
                    avg_order,
                    first_order,
                    last_order,
                )| TopCustomer {
                    customer_id,
                    email,
                    name,
                    total_spent,
                    order_count: order_count as u64,
                    average_order_value: avg_order,
                    first_order_date: first_order,
                    last_order_date: last_order,
                },
            )
            .collect())
    }

    pub async fn get_inventory_health_async(&self) -> Result<InventoryHealth> {
        let total_skus: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let (in_stock, low_stock, out_of_stock): (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN ib.on_hand > COALESCE(ii.reorder_point, 10) THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN ib.on_hand <= COALESCE(ii.reorder_point, 10) AND ib.on_hand > 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN ib.on_hand <= 0 THEN 1 ELSE 0 END), 0)
            FROM inventory_items ii
            LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0, 0, 0));

        let total_value: Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(ib.on_hand * COALESCE(pv.cost_price, pv.price, 0)), 0)
            FROM inventory_items ii
            LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
            LEFT JOIN product_variants pv ON ii.sku = pv.sku
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Decimal::ZERO);

        Ok(InventoryHealth {
            total_skus: total_skus as u64,
            in_stock_skus: in_stock as u64,
            low_stock_skus: low_stock as u64,
            out_of_stock_skus: out_of_stock as u64,
            total_value,
            turnover_ratio: None,
        })
    }

    pub async fn get_low_stock_items_async(
        &self,
        threshold: Option<Decimal>,
    ) -> Result<Vec<LowStockItem>> {
        let threshold_val = threshold.unwrap_or(Decimal::from(10));

        let rows: Vec<(String, String, Decimal, Decimal, Decimal, Option<Decimal>)> =
            sqlx::query_as(
                r#"
                SELECT
                    ii.sku,
                    ii.name,
                    COALESCE(ib.on_hand, 0) as on_hand,
                    COALESCE(ib.allocated, 0) as allocated,
                    COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0) as available,
                    ii.reorder_point
                FROM inventory_items ii
                LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
                WHERE COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0) <= $1
                ORDER BY available ASC
                "#,
            )
            .bind(threshold_val)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(
                |(sku, name, on_hand, allocated, available, reorder_point)| LowStockItem {
                    sku,
                    name,
                    on_hand,
                    allocated,
                    available,
                    reorder_point,
                    average_daily_sales: None,
                    days_of_stock: None,
                },
            )
            .collect())
    }

    pub async fn get_inventory_movement_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<Vec<InventoryMovement>> {
        let (start, end) = self.get_date_range(&query);

        let rows: Vec<(String, String, i64, i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                ii.sku,
                ii.name,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity)::bigint ELSE 0 END), 0) as sold,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'adjustment_in' THEN it.quantity::bigint ELSE 0 END), 0) as received,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'return' THEN it.quantity::bigint ELSE 0 END), 0) as returned,
                COALESCE(SUM(CASE WHEN it.transaction_type IN ('adjustment_in', 'adjustment_out') THEN it.quantity::bigint ELSE 0 END), 0) as adjusted,
                COALESCE(SUM(it.quantity)::bigint, 0) as net_change
            FROM inventory_items ii
            LEFT JOIN inventory_transactions it ON ii.id = it.item_id
                AND it.created_at >= $1 AND it.created_at <= $2
            GROUP BY ii.id, ii.sku, ii.name
            HAVING COALESCE(SUM(it.quantity), 0) != 0
            ORDER BY ABS(COALESCE(SUM(it.quantity), 0)) DESC
            LIMIT 50
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(
                |(sku, name, sold, received, returned, adjusted, net_change)| InventoryMovement {
                    sku,
                    name,
                    units_sold: sold as u64,
                    units_received: received as u64,
                    units_returned: returned as u64,
                    units_adjusted: adjusted,
                    net_change,
                },
            )
            .collect())
    }

    pub async fn get_order_status_breakdown_async(
        &self,
        query: AnalyticsQuery,
    ) -> Result<OrderStatusBreakdown> {
        let (start, end) = self.get_date_range(&query);

        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*)::bigint as cnt
            FROM orders
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY status
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut breakdown = OrderStatusBreakdown::default();
        for (status, count) in rows {
            let count = count as u64;
            breakdown.total += count;
            match status.as_str() {
                "pending" => breakdown.pending = count,
                "confirmed" => breakdown.confirmed = count,
                "processing" => breakdown.processing = count,
                "shipped" => breakdown.shipped = count,
                "delivered" => breakdown.delivered = count,
                "cancelled" => breakdown.cancelled = count,
                "refunded" => breakdown.refunded = count,
                _ => {}
            }
        }

        Ok(breakdown)
    }

    pub async fn get_fulfillment_metrics_async(
        &self,
        _query: AnalyticsQuery,
    ) -> Result<FulfillmentMetrics> {
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let shipped_today: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE status = 'shipped' AND updated_at >= $1",
        )
        .bind(today_start)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let awaiting_shipment: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE status IN ('confirmed', 'processing')",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        Ok(FulfillmentMetrics {
            avg_time_to_ship_hours: None,
            avg_time_to_deliver_hours: None,
            on_time_shipping_percent: None,
            on_time_delivery_percent: None,
            shipped_today: shipped_today as u64,
            awaiting_shipment: awaiting_shipment as u64,
        })
    }

    pub async fn get_return_metrics_async(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        let (start, end) = self.get_date_range(&query);

        let total_returns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM returns WHERE created_at >= $1 AND created_at <= $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let total_orders: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE created_at >= $1 AND created_at <= $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(1);

        let return_rate = if total_orders > 0 {
            Decimal::from(total_returns * 100) / Decimal::from(total_orders)
        } else {
            Decimal::ZERO
        };

        let total_refunded: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(refund_amount), 0) FROM returns WHERE created_at >= $1 AND created_at <= $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Decimal::ZERO);

        let reason_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT reason, COUNT(*)::bigint as cnt
            FROM returns
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY reason
            ORDER BY cnt DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let by_reason: Vec<ReturnReasonCount> = reason_rows
            .into_iter()
            .map(|(reason, count)| {
                let percentage = if total_returns > 0 {
                    Decimal::from(count * 100) / Decimal::from(total_returns)
                } else {
                    Decimal::ZERO
                };
                ReturnReasonCount {
                    reason,
                    count: count as u64,
                    percentage,
                }
            })
            .collect();

        Ok(ReturnMetrics {
            total_returns: total_returns as u64,
            return_rate_percent: return_rate,
            total_refunded,
            by_reason,
            top_returned_products: vec![],
        })
    }

    pub async fn get_demand_forecast_async(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        let days_back = 30i64;
        let start = Utc::now() - Duration::days(days_back);

        let sku_filter = match &skus {
            Some(sku_list) if !sku_list.is_empty() => {
                format!(
                    "AND ii.sku IN ({})",
                    sku_list
                        .iter()
                        .map(|s| format!("'{}'", s.replace('\'', "''")))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            _ => String::new(),
        };

        let sql = format!(
            r#"
            SELECT
                ii.sku,
                ii.name,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity) ELSE 0 END), 0)::float / {} as avg_daily,
                (COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0))::float as current_stock
            FROM inventory_items ii
            LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
            LEFT JOIN inventory_transactions it ON ii.id = it.item_id AND it.created_at >= $1
            {}
            GROUP BY ii.id, ii.sku, ii.name, ib.on_hand, ib.allocated
            HAVING COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity) ELSE 0 END), 0) / {} > 0
               OR (COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0)) < 50
            ORDER BY avg_daily DESC
            LIMIT 50
            "#,
            days_back, sku_filter, days_back
        );

        let rows: Vec<(String, String, f64, f64)> = sqlx::query_as(&sql)
            .bind(start)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows
            .into_iter()
            .map(|(sku, name, avg_daily, current_stock)| {
                let avg_daily_dec =
                    Decimal::from_f64_retain(avg_daily).unwrap_or(Decimal::ZERO);
                let current_stock_dec =
                    Decimal::from_f64_retain(current_stock).unwrap_or(Decimal::ZERO);
                let forecasted = avg_daily_dec * Decimal::from(days_ahead);

                let days_until_stockout = if avg_daily > 0.0 {
                    Some((current_stock / avg_daily) as i32)
                } else {
                    None
                };

                let trend = if avg_daily > 1.0 {
                    Trend::Rising
                } else if avg_daily < 0.5 {
                    Trend::Falling
                } else {
                    Trend::Stable
                };

                DemandForecast {
                    sku,
                    name,
                    average_daily_demand: avg_daily_dec,
                    forecasted_demand: forecasted,
                    confidence: Decimal::new(7, 1),
                    current_stock: current_stock_dec,
                    days_until_stockout,
                    recommended_reorder_qty: if days_until_stockout.map(|d| d < 14).unwrap_or(false)
                    {
                        Some(avg_daily_dec * Decimal::from(30))
                    } else {
                        None
                    },
                    recommended_reorder_date: None,
                    trend,
                }
            })
            .collect())
    }

    pub async fn get_revenue_forecast_async(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
        let days_back = match granularity {
            TimeGranularity::Day => 90,
            TimeGranularity::Week => 180,
            TimeGranularity::Month => 365,
            _ => 365,
        };

        let start = Utc::now() - Duration::days(days_back);
        let date_format = match granularity {
            TimeGranularity::Day => "YYYY-MM-DD",
            TimeGranularity::Week => "IYYY-\"W\"IW",
            TimeGranularity::Month => "YYYY-MM",
            _ => "YYYY-MM",
        };

        let sql = format!(
            r#"
            SELECT AVG(period_revenue) FROM (
                SELECT SUM(total_amount) as period_revenue
                FROM orders
                WHERE created_at >= $1
                  AND status NOT IN ('cancelled', 'refunded')
                GROUP BY to_char(created_at, '{}')
            ) t
            "#,
            date_format
        );

        let avg_revenue: Option<Decimal> = sqlx::query_scalar(&sql)
            .bind(start)
            .fetch_one(&self.pool)
            .await
            .ok();

        let avg_revenue_dec = avg_revenue.unwrap_or(Decimal::ZERO);
        let variance = Decimal::new(15, 2);
        let one = Decimal::ONE;

        let results: Vec<RevenueForecast> = (1..=periods_ahead)
            .map(|i| {
                let period_label = format!("Period +{}", i);
                let lower = avg_revenue_dec * (one - variance);
                let upper = avg_revenue_dec * (one + variance);

                RevenueForecast {
                    period: period_label,
                    forecasted_revenue: avg_revenue_dec,
                    lower_bound: lower,
                    upper_bound: upper,
                    confidence_level: Decimal::new(8, 1),
                    based_on_periods: (days_back / 30) as u32,
                }
            })
            .collect();

        Ok(results)
    }
}

impl AnalyticsRepository for PgAnalyticsRepository {
    fn get_sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        tokio::runtime::Handle::current().block_on(self.get_sales_summary_async(query))
    }

    fn get_revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        tokio::runtime::Handle::current().block_on(self.get_revenue_by_period_async(query))
    }

    fn get_top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        tokio::runtime::Handle::current().block_on(self.get_top_products_async(query))
    }

    fn get_product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>> {
        tokio::runtime::Handle::current().block_on(self.get_product_performance_async(query))
    }

    fn get_customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics> {
        tokio::runtime::Handle::current().block_on(self.get_customer_metrics_async(query))
    }

    fn get_top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        tokio::runtime::Handle::current().block_on(self.get_top_customers_async(query))
    }

    fn get_inventory_health(&self) -> Result<InventoryHealth> {
        tokio::runtime::Handle::current().block_on(self.get_inventory_health_async())
    }

    fn get_low_stock_items(&self, threshold: Option<Decimal>) -> Result<Vec<LowStockItem>> {
        tokio::runtime::Handle::current().block_on(self.get_low_stock_items_async(threshold))
    }

    fn get_inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>> {
        tokio::runtime::Handle::current().block_on(self.get_inventory_movement_async(query))
    }

    fn get_order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown> {
        tokio::runtime::Handle::current().block_on(self.get_order_status_breakdown_async(query))
    }

    fn get_fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        tokio::runtime::Handle::current().block_on(self.get_fulfillment_metrics_async(query))
    }

    fn get_return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        tokio::runtime::Handle::current().block_on(self.get_return_metrics_async(query))
    }

    fn get_demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        tokio::runtime::Handle::current().block_on(self.get_demand_forecast_async(skus, days_ahead))
    }

    fn get_revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
        tokio::runtime::Handle::current()
            .block_on(self.get_revenue_forecast_async(periods_ahead, granularity))
    }
}
