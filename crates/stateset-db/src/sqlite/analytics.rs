//! SQLite analytics repository implementation

use super::{map_db_error, parse_decimal};
use chrono::{DateTime, Datelike, Duration, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AnalyticsQuery, AnalyticsRepository, CustomerMetrics, DemandForecast, FulfillmentMetrics,
    InventoryHealth, InventoryMovement, LowStockItem, OrderStatusBreakdown, ProductPerformance,
    Result, ReturnMetrics, ReturnReasonCount, RevenueByPeriod, RevenueForecast, SalesSummary,
    TimeGranularity, TimePeriod, TopCustomer, TopProduct, Trend,
};
use uuid::Uuid;

pub struct SqliteAnalyticsRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAnalyticsRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    /// Get date range from query parameters
    fn get_date_range(&self, query: &AnalyticsQuery) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let period = query.period.unwrap_or(TimePeriod::Last30Days);

        match period {
            TimePeriod::Today => (now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(), now),
            TimePeriod::Yesterday => {
                let yesterday = now - Duration::days(1);
                (
                    yesterday.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
                    yesterday.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc(),
                )
            }
            TimePeriod::Last7Days => (now - Duration::days(7), now),
            TimePeriod::Last30Days => (now - Duration::days(30), now),
            TimePeriod::ThisMonth => {
                let start = now.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc();
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
                // Simplified: just use 90 days
                (now - Duration::days(90), now)
            }
            TimePeriod::ThisYear => {
                let start = now.date_naive().with_month(1).unwrap().with_day(1).unwrap()
                    .and_hms_opt(0, 0, 0).unwrap().and_utc();
                (start, now)
            }
            TimePeriod::LastYear => (now - Duration::days(365), now),
            TimePeriod::AllTime => (
                DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").unwrap().with_timezone(&Utc),
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
}

impl AnalyticsRepository for SqliteAnalyticsRepository {
    fn get_sales_summary(&self, query: AnalyticsQuery) -> Result<SalesSummary> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        // Get current period metrics
        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    CAST(COALESCE(SUM(total_amount), 0) AS TEXT) as revenue,
                    COUNT(*) as order_count,
                    CAST(COALESCE(SUM(total_amount) / NULLIF(COUNT(*), 0), 0) AS TEXT) as avg_order,
                    COUNT(DISTINCT customer_id) as unique_customers
                FROM orders
                WHERE created_at >= ?1 AND created_at <= ?2
                  AND status NOT IN ('cancelled', 'refunded')
                "#,
            )
            .map_err(map_db_error)?;

        let (revenue, order_count, avg_order, unique_customers): (String, i64, String, i64) = stmt
            .query_row([&start_str, &end_str], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(map_db_error)?;

        // Get items sold
        let items_sold: i64 = conn
            .query_row(
                r#"
                SELECT COALESCE(SUM(oi.quantity), 0)
                FROM order_items oi
                JOIN orders o ON oi.order_id = o.id
                WHERE o.created_at >= ?1 AND o.created_at <= ?2
                  AND o.status NOT IN ('cancelled', 'refunded')
                "#,
                [&start_str, &end_str],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(SalesSummary {
            total_revenue: parse_decimal(&revenue),
            order_count: order_count as u64,
            average_order_value: parse_decimal(&avg_order),
            items_sold: items_sold as u64,
            unique_customers: unique_customers as u64,
            revenue_change_percent: None, // TODO: Compare with previous period
            order_count_change_percent: None,
            period_start: Some(start),
            period_end: Some(end),
        })
    }

    fn get_revenue_by_period(&self, query: AnalyticsQuery) -> Result<Vec<RevenueByPeriod>> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        let granularity = query.granularity.unwrap_or(TimeGranularity::Day);
        let date_format = match granularity {
            TimeGranularity::Hour => "%Y-%m-%d %H:00",
            TimeGranularity::Day => "%Y-%m-%d",
            TimeGranularity::Week => "%Y-W%W",
            TimeGranularity::Month => "%Y-%m",
            TimeGranularity::Quarter => "%Y-Q",
            TimeGranularity::Year => "%Y",
        };

        let mut stmt = conn
            .prepare(&format!(
                r#"
                SELECT
                    strftime('{}', created_at) as period,
                    COALESCE(SUM(total_amount), 0) as revenue,
                    COUNT(*) as order_count,
                    MIN(created_at) as period_start
                FROM orders
                WHERE created_at >= ?1 AND created_at <= ?2
                  AND status NOT IN ('cancelled', 'refunded')
                GROUP BY strftime('{}', created_at)
                ORDER BY period
                "#,
                date_format, date_format
            ))
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str, &end_str], |row| {
                let period: String = row.get(0)?;
                let revenue: String = row.get(1)?;
                let order_count: i64 = row.get(2)?;
                let period_start: String = row.get(3)?;
                Ok((period, revenue, order_count, period_start))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (period, revenue, order_count, period_start) = row.map_err(map_db_error)?;
            results.push(RevenueByPeriod {
                period,
                revenue: parse_decimal(&revenue),
                order_count: order_count as u64,
                period_start: DateTime::parse_from_rfc3339(&period_start)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(start),
            });
        }

        Ok(results)
    }

    fn get_top_products(&self, query: AnalyticsQuery) -> Result<Vec<TopProduct>> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();
        let limit = query.limit.unwrap_or(10) as i64;

        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    oi.product_id,
                    oi.sku,
                    oi.name,
                    SUM(oi.quantity) as units_sold,
                    SUM(oi.total) as revenue,
                    COUNT(DISTINCT oi.order_id) as order_count,
                    AVG(oi.unit_price) as avg_price
                FROM order_items oi
                JOIN orders o ON oi.order_id = o.id
                WHERE o.created_at >= ?1 AND o.created_at <= ?2
                  AND o.status NOT IN ('cancelled', 'refunded')
                GROUP BY oi.sku
                ORDER BY revenue DESC
                LIMIT ?3
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str as &dyn rusqlite::ToSql, &end_str, &limit], |row| {
                let product_id: Option<String> = row.get(0)?;
                let sku: String = row.get(1)?;
                let name: String = row.get(2)?;
                let units_sold: i64 = row.get(3)?;
                let revenue: String = row.get(4)?;
                let order_count: i64 = row.get(5)?;
                let avg_price: String = row.get(6)?;
                Ok((product_id, sku, name, units_sold, revenue, order_count, avg_price))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (product_id, sku, name, units_sold, revenue, order_count, avg_price) =
                row.map_err(map_db_error)?;
            results.push(TopProduct {
                product_id: product_id.and_then(|s| Uuid::parse_str(&s).ok()),
                sku,
                name,
                units_sold: units_sold as u64,
                revenue: parse_decimal(&revenue),
                order_count: order_count as u64,
                average_price: parse_decimal(&avg_price),
            });
        }

        Ok(results)
    }

    fn get_product_performance(&self, query: AnalyticsQuery) -> Result<Vec<ProductPerformance>> {
        // Simplified implementation - just returns top products with growth data
        let top_products = self.get_top_products(query)?;
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

    fn get_customer_metrics(&self, query: AnalyticsQuery) -> Result<CustomerMetrics> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        // Total customers
        let total_customers: i64 = conn
            .query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))
            .unwrap_or(0);

        // New customers in period
        let new_customers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customers WHERE created_at >= ?1 AND created_at <= ?2",
                [&start_str, &end_str],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Returning customers (more than 1 order)
        let returning_customers: i64 = conn
            .query_row(
                r#"
                SELECT COUNT(*) FROM (
                    SELECT customer_id FROM orders
                    GROUP BY customer_id
                    HAVING COUNT(*) > 1
                )
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Average lifetime value
        let avg_ltv: String = conn
            .query_row(
                r#"
                SELECT COALESCE(AVG(total), 0) FROM (
                    SELECT customer_id, SUM(total_amount) as total
                    FROM orders
                    WHERE status NOT IN ('cancelled', 'refunded')
                    GROUP BY customer_id
                )
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        // Average orders per customer
        let avg_orders: String = conn
            .query_row(
                r#"
                SELECT COALESCE(AVG(cnt), 0) FROM (
                    SELECT customer_id, COUNT(*) as cnt
                    FROM orders
                    GROUP BY customer_id
                )
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        Ok(CustomerMetrics {
            total_customers: total_customers as u64,
            new_customers: new_customers as u64,
            returning_customers: returning_customers as u64,
            average_lifetime_value: parse_decimal(&avg_ltv),
            average_orders_per_customer: parse_decimal(&avg_orders),
            retention_rate_percent: None,
        })
    }

    fn get_top_customers(&self, query: AnalyticsQuery) -> Result<Vec<TopCustomer>> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();
        let limit = query.limit.unwrap_or(10) as i64;

        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    c.id,
                    c.email,
                    COALESCE(c.first_name || ' ' || c.last_name, c.email) as name,
                    COALESCE(SUM(o.total_amount), 0) as total_spent,
                    COUNT(o.id) as order_count,
                    COALESCE(AVG(o.total_amount), 0) as avg_order,
                    MIN(o.created_at) as first_order,
                    MAX(o.created_at) as last_order
                FROM customers c
                LEFT JOIN orders o ON c.id = o.customer_id
                    AND o.status NOT IN ('cancelled', 'refunded')
                    AND o.created_at >= ?1 AND o.created_at <= ?2
                GROUP BY c.id
                ORDER BY total_spent DESC
                LIMIT ?3
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str as &dyn rusqlite::ToSql, &end_str, &limit], |row| {
                let id: String = row.get(0)?;
                let email: String = row.get(1)?;
                let name: String = row.get(2)?;
                let total_spent: String = row.get(3)?;
                let order_count: i64 = row.get(4)?;
                let avg_order: String = row.get(5)?;
                let first_order: Option<String> = row.get(6)?;
                let last_order: Option<String> = row.get(7)?;
                Ok((id, email, name, total_spent, order_count, avg_order, first_order, last_order))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (id, email, name, total_spent, order_count, avg_order, first_order, last_order) =
                row.map_err(map_db_error)?;
            results.push(TopCustomer {
                customer_id: Uuid::parse_str(&id).unwrap_or_default(),
                email,
                name,
                total_spent: parse_decimal(&total_spent),
                order_count: order_count as u64,
                average_order_value: parse_decimal(&avg_order),
                first_order_date: first_order
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                last_order_date: last_order
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            });
        }

        Ok(results)
    }

    fn get_inventory_health(&self) -> Result<InventoryHealth> {
        let conn = self.conn()?;

        let total_skus: i64 = conn
            .query_row("SELECT COUNT(*) FROM inventory_items", [], |row| row.get(0))
            .unwrap_or(0);

        // Get stock levels
        let (in_stock, low_stock, out_of_stock): (i64, i64, i64) = conn
            .query_row(
                r#"
                SELECT
                    SUM(CASE WHEN ib.on_hand > COALESCE(ii.reorder_point, 10) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ib.on_hand <= COALESCE(ii.reorder_point, 10) AND ib.on_hand > 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ib.on_hand <= 0 THEN 1 ELSE 0 END)
                FROM inventory_items ii
                LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
                "#,
                [],
                |row| Ok((row.get(0).unwrap_or(0), row.get(1).unwrap_or(0), row.get(2).unwrap_or(0))),
            )
            .unwrap_or((0, 0, 0));

        // Total inventory value (rough estimate)
        let total_value: String = conn
            .query_row(
                r#"
                SELECT COALESCE(SUM(ib.on_hand * COALESCE(pv.cost_price, pv.price, 0)), 0)
                FROM inventory_items ii
                LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
                LEFT JOIN product_variants pv ON ii.sku = pv.sku
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        Ok(InventoryHealth {
            total_skus: total_skus as u64,
            in_stock_skus: in_stock as u64,
            low_stock_skus: low_stock as u64,
            out_of_stock_skus: out_of_stock as u64,
            total_value: parse_decimal(&total_value),
            turnover_ratio: None,
        })
    }

    fn get_low_stock_items(&self, threshold: Option<Decimal>) -> Result<Vec<LowStockItem>> {
        let conn = self.conn()?;
        let threshold_val = threshold.unwrap_or(Decimal::from(10)).to_string();

        let mut stmt = conn
            .prepare(
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
                WHERE COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0) <= ?1
                ORDER BY available ASC
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&threshold_val], |row| {
                let sku: String = row.get(0)?;
                let name: String = row.get(1)?;
                let on_hand: String = row.get(2)?;
                let allocated: String = row.get(3)?;
                let available: String = row.get(4)?;
                let reorder_point: Option<String> = row.get(5)?;
                Ok((sku, name, on_hand, allocated, available, reorder_point))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (sku, name, on_hand, allocated, available, reorder_point) =
                row.map_err(map_db_error)?;
            results.push(LowStockItem {
                sku,
                name,
                on_hand: parse_decimal(&on_hand),
                allocated: parse_decimal(&allocated),
                available: parse_decimal(&available),
                reorder_point: reorder_point.map(|s| parse_decimal(&s)),
                average_daily_sales: None,
                days_of_stock: None,
            });
        }

        Ok(results)
    }

    fn get_inventory_movement(&self, query: AnalyticsQuery) -> Result<Vec<InventoryMovement>> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    ii.sku,
                    ii.name,
                    COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity) ELSE 0 END), 0) as sold,
                    COALESCE(SUM(CASE WHEN it.transaction_type = 'adjustment_in' THEN it.quantity ELSE 0 END), 0) as received,
                    COALESCE(SUM(CASE WHEN it.transaction_type = 'return' THEN it.quantity ELSE 0 END), 0) as returned,
                    COALESCE(SUM(CASE WHEN it.transaction_type IN ('adjustment_in', 'adjustment_out') THEN it.quantity ELSE 0 END), 0) as adjusted,
                    COALESCE(SUM(it.quantity), 0) as net_change
                FROM inventory_items ii
                LEFT JOIN inventory_transactions it ON ii.id = it.item_id
                    AND it.created_at >= ?1 AND it.created_at <= ?2
                GROUP BY ii.id
                HAVING net_change != 0
                ORDER BY ABS(net_change) DESC
                LIMIT 50
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str, &end_str], |row| {
                let sku: String = row.get(0)?;
                let name: String = row.get(1)?;
                let sold: i64 = row.get(2)?;
                let received: i64 = row.get(3)?;
                let returned: i64 = row.get(4)?;
                let adjusted: i64 = row.get(5)?;
                let net_change: i64 = row.get(6)?;
                Ok((sku, name, sold, received, returned, adjusted, net_change))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (sku, name, sold, received, returned, adjusted, net_change) =
                row.map_err(map_db_error)?;
            results.push(InventoryMovement {
                sku,
                name,
                units_sold: sold as u64,
                units_received: received as u64,
                units_returned: returned as u64,
                units_adjusted: adjusted,
                net_change,
            });
        }

        Ok(results)
    }

    fn get_order_status_breakdown(&self, query: AnalyticsQuery) -> Result<OrderStatusBreakdown> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT status, COUNT(*) as cnt
                FROM orders
                WHERE created_at >= ?1 AND created_at <= ?2
                GROUP BY status
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str, &end_str], |row| {
                let status: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((status, count))
            })
            .map_err(map_db_error)?;

        let mut breakdown = OrderStatusBreakdown::default();
        for row in rows {
            let (status, count) = row.map_err(map_db_error)?;
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

    fn get_fulfillment_metrics(&self, query: AnalyticsQuery) -> Result<FulfillmentMetrics> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let _start_str = start.to_rfc3339();
        let _end_str = end.to_rfc3339();

        // Shipped today
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let shipped_today: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM orders WHERE status = 'shipped' AND updated_at >= ?1",
                [&today_start],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Awaiting shipment
        let awaiting_shipment: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM orders WHERE status IN ('confirmed', 'processing')",
                [],
                |row| row.get(0),
            )
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

    fn get_return_metrics(&self, query: AnalyticsQuery) -> Result<ReturnMetrics> {
        let conn = self.conn()?;
        let (start, end) = self.get_date_range(&query);
        let start_str = start.to_rfc3339();
        let end_str = end.to_rfc3339();

        // Total returns
        let total_returns: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM returns WHERE created_at >= ?1 AND created_at <= ?2",
                [&start_str, &end_str],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Total orders for return rate
        let total_orders: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM orders WHERE created_at >= ?1 AND created_at <= ?2",
                [&start_str, &end_str],
                |row| row.get(0),
            )
            .unwrap_or(1);

        let return_rate = if total_orders > 0 {
            Decimal::from(total_returns * 100) / Decimal::from(total_orders)
        } else {
            Decimal::ZERO
        };

        // Total refunded
        let total_refunded: String = conn
            .query_row(
                "SELECT COALESCE(SUM(refund_amount), 0) FROM returns WHERE created_at >= ?1 AND created_at <= ?2",
                [&start_str, &end_str],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        // Returns by reason
        let mut stmt = conn
            .prepare(
                r#"
                SELECT reason, COUNT(*) as cnt
                FROM returns
                WHERE created_at >= ?1 AND created_at <= ?2
                GROUP BY reason
                ORDER BY cnt DESC
                "#,
            )
            .map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start_str, &end_str], |row| {
                let reason: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((reason, count))
            })
            .map_err(map_db_error)?;

        let mut by_reason = Vec::new();
        for row in rows {
            let (reason, count) = row.map_err(map_db_error)?;
            let percentage = if total_returns > 0 {
                Decimal::from(count * 100) / Decimal::from(total_returns)
            } else {
                Decimal::ZERO
            };
            by_reason.push(ReturnReasonCount {
                reason,
                count: count as u64,
                percentage,
            });
        }

        Ok(ReturnMetrics {
            total_returns: total_returns as u64,
            return_rate_percent: return_rate,
            total_refunded: parse_decimal(&total_refunded),
            by_reason,
            top_returned_products: vec![], // TODO: Implement
        })
    }

    fn get_demand_forecast(&self, skus: Option<Vec<String>>, days_ahead: u32) -> Result<Vec<DemandForecast>> {
        let conn = self.conn()?;
        let days_back = 30; // Use 30 days of history
        let start = (Utc::now() - Duration::days(days_back)).to_rfc3339();

        // Build SKU filter
        let sku_filter = match &skus {
            Some(sku_list) if !sku_list.is_empty() => {
                format!("AND ii.sku IN ({})", sku_list.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(","))
            }
            _ => String::new(),
        };

        let query = format!(
            r#"
            SELECT
                ii.sku,
                ii.name,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity) ELSE 0 END), 0) / {} as avg_daily,
                COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0) as current_stock
            FROM inventory_items ii
            LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
            LEFT JOIN inventory_transactions it ON ii.id = it.item_id AND it.created_at >= ?1
            {}
            GROUP BY ii.id
            HAVING avg_daily > 0 OR current_stock < 50
            ORDER BY avg_daily DESC
            LIMIT 50
            "#,
            days_back, sku_filter
        );

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;

        let rows = stmt
            .query_map([&start], |row| {
                let sku: String = row.get(0)?;
                let name: String = row.get(1)?;
                let avg_daily: f64 = row.get(2)?;
                let current_stock: f64 = row.get(3)?;
                Ok((sku, name, avg_daily, current_stock))
            })
            .map_err(map_db_error)?;

        let mut results = Vec::new();
        for row in rows {
            let (sku, name, avg_daily, current_stock) = row.map_err(map_db_error)?;
            let avg_daily_dec = Decimal::from_f64_retain(avg_daily).unwrap_or(Decimal::ZERO);
            let current_stock_dec = Decimal::from_f64_retain(current_stock).unwrap_or(Decimal::ZERO);
            let forecasted = avg_daily_dec * Decimal::from(days_ahead);

            let days_until_stockout = if avg_daily > 0.0 {
                Some((current_stock / avg_daily) as i32)
            } else {
                None
            };

            // Simple trend detection
            let trend = if avg_daily > 1.0 {
                Trend::Rising
            } else if avg_daily < 0.5 {
                Trend::Falling
            } else {
                Trend::Stable
            };

            results.push(DemandForecast {
                sku,
                name,
                average_daily_demand: avg_daily_dec,
                forecasted_demand: forecasted,
                confidence: Decimal::new(7, 1), // Simple confidence score: 0.7
                current_stock: current_stock_dec,
                days_until_stockout,
                recommended_reorder_qty: if days_until_stockout.map(|d| d < 14).unwrap_or(false) {
                    Some(avg_daily_dec * Decimal::from(30)) // 30 days supply
                } else {
                    None
                },
                recommended_reorder_date: None,
                trend,
            });
        }

        Ok(results)
    }

    fn get_revenue_forecast(&self, periods_ahead: u32, granularity: TimeGranularity) -> Result<Vec<RevenueForecast>> {
        let conn = self.conn()?;

        // Get historical revenue by period
        let days_back = match granularity {
            TimeGranularity::Day => 90,
            TimeGranularity::Week => 180,
            TimeGranularity::Month => 365,
            _ => 365,
        };

        let start = (Utc::now() - Duration::days(days_back)).to_rfc3339();
        let date_format = match granularity {
            TimeGranularity::Day => "%Y-%m-%d",
            TimeGranularity::Week => "%Y-W%W",
            TimeGranularity::Month => "%Y-%m",
            _ => "%Y-%m",
        };

        // Get average revenue per period
        let avg_revenue: f64 = conn
            .query_row(
                &format!(
                    r#"
                    SELECT AVG(period_revenue) FROM (
                        SELECT SUM(total_amount) as period_revenue
                        FROM orders
                        WHERE created_at >= ?1
                          AND status NOT IN ('cancelled', 'refunded')
                        GROUP BY strftime('{}', created_at)
                    )
                    "#,
                    date_format
                ),
                [&start],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let avg_revenue_dec = Decimal::from_f64_retain(avg_revenue).unwrap_or(Decimal::ZERO);

        // Generate forecast periods
        let mut results = Vec::new();
        let variance = Decimal::new(15, 2); // 0.15 = 15% variance
        let one = Decimal::ONE;
        for i in 1..=periods_ahead {
            let period_label = format!("Period +{}", i);
            let lower = avg_revenue_dec * (one - variance);
            let upper = avg_revenue_dec * (one + variance);

            results.push(RevenueForecast {
                period: period_label,
                forecasted_revenue: avg_revenue_dec,
                lower_bound: lower,
                upper_bound: upper,
                confidence_level: Decimal::new(8, 1), // 0.8
                based_on_periods: (days_back / 30) as u32,
            });
        }

        Ok(results)
    }
}
