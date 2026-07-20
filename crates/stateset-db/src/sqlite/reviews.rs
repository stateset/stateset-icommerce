//! SQLite implementation of product review repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateReview, ProductId, Result, Review, ReviewFilter, ReviewId,
    ReviewRepository, ReviewSummary, UpdateReview,
};

#[derive(Debug)]
pub struct SqliteReviewRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteReviewRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_review(row: &rusqlite::Row<'_>) -> rusqlite::Result<Review> {
        Ok(Review {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "review", "id")?.into(),
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "review",
                "product_id",
            )?
            .into(),
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "review",
                "customer_id",
            )?
            .into(),
            rating: row.get::<_, i32>("rating")? as u8,
            title: row.get("title")?,
            body: row.get("body")?,
            status: parse_enum_row(&row.get::<_, String>("status")?, "review", "status")?,
            verified_purchase: row.get::<_, i32>("verified_purchase")? != 0,
            helpful_count: row.get::<_, i32>("helpful_count")? as u32,
            reported_count: row.get::<_, i32>("reported")? as u32,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "review",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "review",
                "updated_at",
            )?,
        })
    }
}

impl ReviewRepository for SqliteReviewRepository {
    fn create(&self, input: CreateReview) -> Result<Review> {
        let id = ReviewId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO reviews (id, product_id, customer_id, rating, title, body, status, verified_purchase, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.product_id.to_string(),
                    input.customer_id.to_string(),
                    input.rating as i32,
                    &input.title,
                    &input.body,
                    "pending",
                    input.verified_purchase as i32,
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM reviews WHERE id = ?", [&id_str], Self::row_to_review)
        })
    }

    fn get(&self, id: ReviewId) -> Result<Option<Review>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM reviews WHERE id = ?",
            [id.to_string()],
            Self::row_to_review,
        ) {
            Ok(review) => Ok(Some(review)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: ReviewId, input: UpdateReview) -> Result<Review> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(rating) = input.rating {
                sets.push("rating = ?".into());
                params.push(Box::new(rating as i32));
            }
            if let Some(ref title) = input.title {
                sets.push("title = ?".into());
                params.push(Box::new(title.clone()));
            }
            if let Some(ref body) = input.body {
                sets.push("body = ?".into());
                params.push(Box::new(body.clone()));
            }
            if let Some(status) = input.status {
                sets.push("status = ?".into());
                params.push(Box::new(status.to_string()));
            }

            let sql = format!("UPDATE reviews SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM reviews WHERE id = ?", [&id_str], Self::row_to_review)
        })
    }

    fn list(&self, filter: ReviewFilter) -> Result<Vec<Review>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM reviews WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(product_id) = filter.product_id {
            sql.push_str(" AND product_id = ?");
            params.push(Box::new(product_id.to_string()));
        }
        if let Some(customer_id) = filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(min_rating) = filter.min_rating {
            sql.push_str(" AND rating >= ?");
            params.push(Box::new(min_rating as i32));
        }
        if let Some(verified_only) = filter.verified_only {
            // `verified_purchase` is stored as INTEGER (0/1); Postgres applies this
            // filter too.
            sql.push_str(" AND verified_purchase = ?");
            params.push(Box::new(verified_only as i32));
        }

        sql.push_str(" ORDER BY created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let reviews = stmt
            .query_map(param_refs.as_slice(), Self::row_to_review)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(reviews)
    }

    fn delete(&self, id: ReviewId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM reviews WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        Ok(())
    }

    fn get_summary(&self, product_id: ProductId) -> Result<ReviewSummary> {
        let conn = self.conn()?;
        let pid = product_id.to_string();

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM reviews WHERE product_id = ? AND status = 'approved'",
                [&pid],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        let avg: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(CAST(rating AS REAL)), 0.0) FROM reviews WHERE product_id = ? AND status = 'approved'",
                [&pid],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        // Compute rating distribution
        let mut distribution = [0u32; 5];
        let mut stmt = conn
            .prepare("SELECT rating, COUNT(*) FROM reviews WHERE product_id = ? AND status = 'approved' GROUP BY rating")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([&pid], |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)))
            .map_err(map_db_error)?;
        for (rating, count) in rows.flatten() {
            let idx = (rating - 1).clamp(0, 4) as usize;
            distribution[idx] = count as u32;
        }

        Ok(ReviewSummary {
            product_id,
            total_reviews: total as u64,
            average_rating: avg,
            rating_distribution: distribution,
        })
    }

    fn mark_helpful(&self, id: ReviewId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE reviews SET helpful_count = helpful_count + 1, updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    fn mark_reported(&self, id: ReviewId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE reviews SET reported = reported + 1, updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::CustomerId;

    fn test_repo() -> SqliteReviewRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).unwrap();
        // V4 reviews table isn't in the base migration set — create it for tests
        let conn = db.conn().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reviews (
                id TEXT PRIMARY KEY,
                product_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                order_id TEXT,
                rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
                title TEXT,
                body TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                helpful_count INTEGER NOT NULL DEFAULT 0,
                reported INTEGER NOT NULL DEFAULT 0,
                verified_purchase INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .unwrap();
        SqliteReviewRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_review() {
        let repo = test_repo();
        let review = repo
            .create(CreateReview {
                product_id: ProductId::new(),
                customer_id: CustomerId::new(),
                rating: 5,
                title: Some("Great product".into()),
                body: Some("Really loved it".into()),
                verified_purchase: true,
            })
            .unwrap();

        assert_eq!(review.rating, 5);
        assert_eq!(review.title.as_deref(), Some("Great product"));
        assert!(review.verified_purchase);

        let fetched = repo.get(review.id).unwrap().unwrap();
        assert_eq!(fetched.id, review.id);
    }

    #[test]
    fn list_reviews_by_product() {
        let repo = test_repo();
        let product_id = ProductId::new();

        for i in 1..=3 {
            repo.create(CreateReview {
                product_id,
                customer_id: CustomerId::new(),
                rating: i as u8 + 2,
                title: None,
                body: None,
                verified_purchase: false,
            })
            .unwrap();
        }

        let reviews =
            repo.list(ReviewFilter { product_id: Some(product_id), ..Default::default() }).unwrap();
        assert_eq!(reviews.len(), 3);
    }

    #[test]
    fn list_filters_by_verified_only() {
        let repo = test_repo();
        let product_id = ProductId::new();
        repo.create(CreateReview {
            product_id,
            customer_id: CustomerId::new(),
            rating: 5,
            title: None,
            body: None,
            verified_purchase: true,
        })
        .unwrap();
        repo.create(CreateReview {
            product_id,
            customer_id: CustomerId::new(),
            rating: 4,
            title: None,
            body: None,
            verified_purchase: false,
        })
        .unwrap();

        // Postgres applies `verified_only`; SQLite must too.
        let verified = repo
            .list(ReviewFilter {
                product_id: Some(product_id),
                verified_only: Some(true),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(verified.len(), 1, "verified_only must filter to verified-purchase reviews");
        assert!(verified[0].verified_purchase);
    }

    #[test]
    fn delete_review() {
        let repo = test_repo();
        let review = repo
            .create(CreateReview {
                product_id: ProductId::new(),
                customer_id: CustomerId::new(),
                rating: 3,
                title: None,
                body: None,
                verified_purchase: false,
            })
            .unwrap();

        repo.delete(review.id).unwrap();
        assert!(repo.get(review.id).unwrap().is_none());
    }

    #[test]
    fn mark_helpful_increments() {
        let repo = test_repo();
        let review = repo
            .create(CreateReview {
                product_id: ProductId::new(),
                customer_id: CustomerId::new(),
                rating: 4,
                title: None,
                body: None,
                verified_purchase: false,
            })
            .unwrap();
        assert_eq!(review.helpful_count, 0);

        repo.mark_helpful(review.id).unwrap();
        repo.mark_helpful(review.id).unwrap();

        let updated = repo.get(review.id).unwrap().unwrap();
        assert_eq!(updated.helpful_count, 2);
    }
}
