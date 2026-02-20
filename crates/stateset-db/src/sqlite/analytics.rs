//! SQLite analytics repository implementation

use super::map_db_error;
use super::parse_helpers::parse_decimal as parse_decimal_with_context;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::ToSql;
use rust_decimal::Decimal;
use stateset_core::{
    AnalyticsQuery, AnalyticsRepository, CustomerMetrics, DemandForecast, FulfillmentMetrics,
    InventoryHealth, InventoryMovement, LowStockItem, OrderStatusBreakdown, ProductId,
    ProductPerformance, Result, ReturnMetrics, ReturnReasonCount, RevenueByPeriod,
    RevenueForecast, SalesSummary, TimeGranularity, TimePeriod, TopCustomer, TopProduct,
    TopReturnedProduct, Trend,
    validate_batch_size,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteAnalyticsRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteAnalyticsRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| stateset_core::CommerceError::DatabaseError(e.to_string()))
    }

    fn start_of_day(date: NaiveDate) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
    }

    fn end_of_day(date: NaiveDate) -> DateTime<Utc> {
        Self::start_of_day(date) + Duration::days(1) - Duration::seconds(1)
    }

    fn first_day_of_month(date: NaiveDate) -> NaiveDate {
        date.with_day(1).unwrap_or(date)
    }

    fn first_day_of_year(date: NaiveDate) -> NaiveDate {
        date.with_month(1)
            .and_then(|d| d.with_day(1))
            .unwrap_or_else(|| Self::first_day_of_month(date))
    }

    fn all_time_start() -> DateTime<Utc> {
        if let Some(date) = NaiveDate::from_ymd_opt(2000, 1, 1) {
            return Self::start_of_day(date);
        }
        if let Some(epoch) = DateTime::<Utc>::from_timestamp(0, 0) {
            return epoch;
        }
        Utc::now()
    }

    /// Get date range from query parameters
    fn get_date_range(&self, query: &AnalyticsQuery) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let period = query.period.unwrap_or(TimePeriod::Last30Days);

        match period {
            TimePeriod::Today => (Self::start_of_day(now.date_naive()), now),
            TimePeriod::Yesterday => {
                let yesterday = now - Duration::days(1);
                (
                    Self::start_of_day(yesterday.date_naive()),
                    Self::end_of_day(yesterday.date_naive()),
                )
            }
            TimePeriod::Last7Days => (now - Duration::days(7), now),
            TimePeriod::Last30Days => (now - Duration::days(30), now),
            TimePeriod::ThisMonth => {
                let start = Self::start_of_day(Self::first_day_of_month(now.date_naive()));
                (start, now)
            }
            TimePeriod::LastMonth => {
                let this_month_start = Self::first_day_of_month(now.date_naive());
                let last_month_end = this_month_start - Duration::days(1);
                let last_month_start = Self::first_day_of_month(last_month_end);
                (Self::start_of_day(last_month_start), Self::end_of_day(last_month_end))
            }
            TimePeriod::ThisQuarter | TimePeriod::LastQuarter => {
                // Simplified: just use 90 days
                (now - Duration::days(90), now)
            }
            TimePeriod::ThisYear => {
                let start = Self::start_of_day(Self::first_day_of_year(now.date_naive()));
                (start, now)
            }
            TimePeriod::LastYear => (now - Duration::days(365), now),
            TimePeriod::AllTime => (Self::all_time_start(), now),
            TimePeriod::Custom => {
                if let Some(ref range) = query.date_range {
                    (range.start.unwrap_or(now - Duration::days(30)), range.end.unwrap_or(now))
                } else {
                    (now - Duration::days(30), now)
                }
            }
            _ => (now - Duration::days(30), now),
        }
    }
}

fn parse_decimal_value(value: &str, field: &str) -> Result<Decimal> {
    parse_decimal_with_context(value, "analytics", field)
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

        // Calculate previous period for comparison
        let period_duration = end - start;
        let prev_end = start;
        let prev_start = prev_end - period_duration;
        let prev_start_str = prev_start.to_rfc3339();
        let prev_end_str = prev_end.to_rfc3339();

        // Get previous period metrics
        let (prev_revenue, prev_order_count): (String, i64) = conn
            .query_row(
                r#"
                SELECT
                    CAST(COALESCE(SUM(total_amount), 0) AS TEXT) as revenue,
                    COUNT(*) as order_count
                FROM orders
                WHERE created_at >= ?1 AND created_at < ?2
                  AND status NOT IN ('cancelled', 'refunded')
                "#,
                [&prev_start_str, &prev_end_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or(("0".to_string(), 0));

        let current_revenue = parse_decimal_value(&revenue, "revenue")?;
        let previous_revenue = parse_decimal_value(&prev_revenue, "previous_revenue")?;

        // Calculate percentage changes
        let revenue_change_percent = if previous_revenue != Decimal::ZERO {
            Some(((current_revenue - previous_revenue) / previous_revenue) * Decimal::from(100))
        } else if current_revenue != Decimal::ZERO {
            Some(Decimal::from(100)) // 100% increase from zero
        } else {
            Some(Decimal::ZERO)
        };

        let order_count_change_percent = if prev_order_count > 0 {
            let change =
                ((order_count - prev_order_count) as f64 / prev_order_count as f64) * 100.0;
            Decimal::from_f64_retain(change)
        } else if order_count > 0 {
            Some(Decimal::from(100))
        } else {
            Some(Decimal::ZERO)
        };

        Ok(SalesSummary {
            total_revenue: current_revenue,
            order_count: order_count as u64,
            average_order_value: parse_decimal_value(&avg_order, "average_order_value")?,
            items_sold: items_sold as u64,
            unique_customers: unique_customers as u64,
            revenue_change_percent,
            order_count_change_percent,
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
        let period_expr = match granularity {
            TimeGranularity::Hour => "strftime('%Y-%m-%d %H:00', created_at)".to_string(),
            TimeGranularity::Day => "strftime('%Y-%m-%d', created_at)".to_string(),
            TimeGranularity::Week => "strftime('%Y-W%W', created_at)".to_string(),
            TimeGranularity::Month => "strftime('%Y-%m', created_at)".to_string(),
            TimeGranularity::Quarter => {
                // SQLite doesn't have a built-in quarter function. Derive it from the month:
                // Q = ((month - 1) / 3) + 1, resulting in 1..=4.
                "strftime('%Y', created_at) || '-Q' || ((CAST(strftime('%m', created_at) AS INTEGER) - 1) / 3 + 1)".to_string()
            }
            TimeGranularity::Year => "strftime('%Y', created_at)".to_string(),
            _ => "strftime('%Y-%m-%d', created_at)".to_string(),
        };

        let mut stmt = conn
            .prepare(&format!(
                r#"
                SELECT
                    {} as period,
                    CAST(COALESCE(SUM(total_amount), 0) AS TEXT) as revenue,
                    COUNT(*) as order_count,
                    MIN(created_at) as period_start
                FROM orders
                WHERE created_at >= ?1 AND created_at <= ?2
                  AND status NOT IN ('cancelled', 'refunded')
                GROUP BY {}
                ORDER BY period
                "#,
                period_expr, period_expr
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
            let revenue = parse_decimal_value(&revenue, "revenue")?;
            results.push(RevenueByPeriod {
                period,
                revenue,
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
                    CAST(COALESCE(SUM(oi.total), 0) AS TEXT) as revenue,
                    COUNT(DISTINCT oi.order_id) as order_count,
                    CAST(COALESCE(AVG(oi.unit_price), 0) AS TEXT) as avg_price
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
            let revenue = parse_decimal_value(&revenue, "revenue")?;
            let average_price = parse_decimal_value(&avg_price, "average_price")?;
            results.push(TopProduct {
                product_id: product_id.and_then(|s| Uuid::parse_str(&s).ok().map(ProductId::from)),
                sku,
                name,
                units_sold: units_sold as u64,
                revenue,
                order_count: order_count as u64,
                average_price,
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
        let total_customers: i64 =
            conn.query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0)).unwrap_or(0);

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
                SELECT CAST(COALESCE(AVG(total), 0) AS TEXT) FROM (
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
                SELECT CAST(COALESCE(AVG(cnt), 0) AS TEXT) FROM (
                    SELECT customer_id, COUNT(*) as cnt
                    FROM orders
                    GROUP BY customer_id
                )
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        let average_lifetime_value = parse_decimal_value(&avg_ltv, "average_lifetime_value")?;
        let average_orders_per_customer =
            parse_decimal_value(&avg_orders, "average_orders_per_customer")?;

        Ok(CustomerMetrics {
            total_customers: total_customers as u64,
            new_customers: new_customers as u64,
            returning_customers: returning_customers as u64,
            average_lifetime_value,
            average_orders_per_customer,
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
                    CAST(COALESCE(SUM(o.total_amount), 0) AS TEXT) as total_spent,
                    COUNT(o.id) as order_count,
                    CAST(COALESCE(AVG(o.total_amount), 0) AS TEXT) as avg_order,
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
            let total_spent = parse_decimal_value(&total_spent, "total_spent")?;
            let average_order_value = parse_decimal_value(&avg_order, "average_order_value")?;
            results.push(TopCustomer {
                customer_id: Uuid::parse_str(&id).unwrap_or_default(),
                email,
                name,
                total_spent,
                order_count: order_count as u64,
                average_order_value,
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
                SELECT CAST(COALESCE(SUM(ib.on_hand * COALESCE(pv.cost_price, pv.price, 0)), 0) AS TEXT)
                FROM inventory_items ii
                LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
                LEFT JOIN product_variants pv ON ii.sku = pv.sku
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());

        let total_value = parse_decimal_value(&total_value, "total_value")?;

        Ok(InventoryHealth {
            total_skus: total_skus as u64,
            in_stock_skus: in_stock as u64,
            low_stock_skus: low_stock as u64,
            out_of_stock_skus: out_of_stock as u64,
            total_value,
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
                    CAST(COALESCE(ib.on_hand, 0) AS TEXT) as on_hand,
                    CAST(COALESCE(ib.allocated, 0) AS TEXT) as allocated,
                    CAST(COALESCE(ib.on_hand, 0) - COALESCE(ib.allocated, 0) AS TEXT) as available,
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
            let on_hand = parse_decimal_value(&on_hand, "on_hand")?;
            let allocated = parse_decimal_value(&allocated, "allocated")?;
            let available = parse_decimal_value(&available, "available")?;
            let reorder_point =
                reorder_point.map(|s| parse_decimal_value(&s, "reorder_point")).transpose()?;
            results.push(LowStockItem {
                sku,
                name,
                on_hand,
                allocated,
                available,
                reorder_point,
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
        let today_start = Self::start_of_day(Utc::now().date_naive()).to_rfc3339();
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
                "SELECT CAST(COALESCE(SUM(refund_amount), 0) AS TEXT) FROM returns WHERE created_at >= ?1 AND created_at <= ?2",
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
            by_reason.push(ReturnReasonCount { reason, count: count as u64, percentage });
        }

        // Get top returned products
        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                    ri.sku,
                    ri.name,
                    SUM(ri.quantity) as units_returned,
                    COALESCE(
                        (SELECT SUM(oi.quantity) FROM order_items oi WHERE oi.sku = ri.sku),
                        0
                    ) as units_sold
                FROM return_items ri
                JOIN returns r ON ri.return_id = r.id
                WHERE r.created_at >= ?1 AND r.created_at <= ?2
                GROUP BY ri.sku
                ORDER BY units_returned DESC
                LIMIT 10
                "#,
            )
            .map_err(map_db_error)?;

        let product_rows = stmt
            .query_map([&start_str, &end_str], |row| {
                let sku: String = row.get(0)?;
                let name: String = row.get(1)?;
                let units_returned: i64 = row.get(2)?;
                let units_sold: i64 = row.get(3)?;
                Ok((sku, name, units_returned, units_sold))
            })
            .map_err(map_db_error)?;

        let mut top_returned_products = Vec::new();
        for row in product_rows {
            let (sku, name, units_returned, units_sold) = row.map_err(map_db_error)?;
            let return_rate = if units_sold > 0 {
                Decimal::from(units_returned * 100) / Decimal::from(units_sold)
            } else {
                Decimal::ZERO
            };
            top_returned_products.push(TopReturnedProduct {
                sku,
                name,
                units_returned: units_returned as u64,
                units_sold: units_sold as u64,
                return_rate_percent: return_rate,
            });
        }

        let total_refunded = parse_decimal_value(&total_refunded, "total_refunded")?;

        Ok(ReturnMetrics {
            total_returns: total_returns as u64,
            return_rate_percent: return_rate,
            total_refunded,
            by_reason,
            top_returned_products,
        })
    }

    fn get_demand_forecast(
        &self,
        skus: Option<Vec<String>>,
        days_ahead: u32,
    ) -> Result<Vec<DemandForecast>> {
        let conn = self.conn()?;
        let days_back = 30; // Use 30 days of history
        let start = (Utc::now() - Duration::days(days_back)).to_rfc3339();

        // Build SKU filter
        let mut params: Vec<Box<dyn ToSql>> = vec![Box::new(start.clone())];
        let where_clause = match &skus {
            Some(sku_list) if !sku_list.is_empty() => {
                let placeholders = sku_list.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                for sku in sku_list {
                    params.push(Box::new(sku.clone()));
                }
                format!("WHERE ii.sku IN ({})", placeholders)
            }
            _ => String::new(),
        };

        let query = format!(
            r#"
            SELECT
                ii.sku,
                ii.name,
                COALESCE(SUM(CASE WHEN it.transaction_type = 'sale' THEN ABS(it.quantity) ELSE 0 END), 0) / {} as avg_daily,
                COALESCE(ib.quantity_on_hand, 0) - COALESCE(ib.quantity_allocated, 0) as current_stock
            FROM inventory_items ii
            LEFT JOIN inventory_balances ib ON ii.id = ib.item_id
            LEFT JOIN inventory_transactions it ON ii.id = it.item_id AND it.created_at >= ?
            {}
            GROUP BY ii.id
            HAVING avg_daily > 0 OR current_stock < 50
            ORDER BY avg_daily DESC
            LIMIT 50
            "#,
            days_back, where_clause
        );

        let mut stmt = conn.prepare(&query).map_err(map_db_error)?;

        let params_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
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
            let current_stock_dec =
                Decimal::from_f64_retain(current_stock).unwrap_or(Decimal::ZERO);
            let forecasted = avg_daily_dec * Decimal::from(days_ahead);

            let days_until_stockout =
                if avg_daily > 0.0 { Some((current_stock / avg_daily) as i32) } else { None };

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

    fn get_revenue_forecast(
        &self,
        periods_ahead: u32,
        granularity: TimeGranularity,
    ) -> Result<Vec<RevenueForecast>> {
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

    fn get_sales_summary_batch(&self, queries: Vec<AnalyticsQuery>) -> Result<Vec<SalesSummary>> {
        validate_batch_size(&queries)?;
        let mut results = Vec::with_capacity(queries.len());
        for query in queries {
            results.push(self.get_sales_summary(query)?);
        }
        Ok(results)
    }
}
