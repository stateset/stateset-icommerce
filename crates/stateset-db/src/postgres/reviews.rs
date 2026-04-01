//! PostgreSQL implementation of product review repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateReview, ProductId, Result, Review, ReviewFilter, ReviewId, ReviewRepository,
    ReviewStatus, ReviewSummary, UpdateReview,
};
use uuid::Uuid;

/// PostgreSQL review repository
#[derive(Debug, Clone)]
pub struct PgReviewRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ReviewRow {
    id: Uuid,
    product_id: Uuid,
    customer_id: Uuid,
    rating: i32,
    title: Option<String>,
    body: Option<String>,
    status: String,
    verified_purchase: bool,
    helpful_count: i32,
    report_count: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RatingDistRow {
    rating: i32,
    cnt: i64,
}

impl PgReviewRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_review(row: ReviewRow) -> Result<Review> {
        let status: ReviewStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid review.status '{}': {}", row.status, e))
        })?;

        Ok(Review {
            id: ReviewId::from(row.id),
            product_id: ProductId::from(row.product_id),
            customer_id: row.customer_id.into(),
            rating: row.rating as u8,
            title: row.title,
            body: row.body,
            status,
            verified_purchase: row.verified_purchase,
            helpful_count: row.helpful_count as u32,
            reported_count: row.report_count as u32,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    // ---- async helpers ----

    async fn create_async(&self, input: CreateReview) -> Result<Review> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO reviews (id, product_id, customer_id, rating, title, body, status, verified_purchase, helpful_count, report_count, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, 0, 0, $8, $9)",
        )
        .bind(id)
        .bind(input.product_id.into_uuid())
        .bind(input.customer_id.into_uuid())
        .bind(input.rating as i32)
        .bind(&input.title)
        .bind(&input.body)
        .bind(input.verified_purchase)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn get_async(&self, id: Uuid) -> Result<Option<Review>> {
        let row = sqlx::query_as::<_, ReviewRow>(
            "SELECT id, product_id, customer_id, rating, title, body, status, verified_purchase, helpful_count, report_count, created_at, updated_at
             FROM reviews WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(r) => Ok(Some(Self::row_to_review(r)?)),
            None => Ok(None),
        }
    }

    async fn update_async(&self, id: Uuid, input: UpdateReview) -> Result<Review> {
        let now = Utc::now();

        // Build dynamic SET clause
        let mut param_idx = 2u32;
        let mut query = String::from("UPDATE reviews SET updated_at = $1");
        let mut has_rating = false;
        let mut has_title = false;
        let mut has_body = false;
        let mut has_status = false;

        if input.rating.is_some() {
            query.push_str(&format!(", rating = ${param_idx}"));
            param_idx += 1;
            has_rating = true;
        }
        if input.title.is_some() {
            query.push_str(&format!(", title = ${param_idx}"));
            param_idx += 1;
            has_title = true;
        }
        if input.body.is_some() {
            query.push_str(&format!(", body = ${param_idx}"));
            param_idx += 1;
            has_body = true;
        }
        if input.status.is_some() {
            query.push_str(&format!(", status = ${param_idx}"));
            param_idx += 1;
            has_status = true;
        }

        query.push_str(&format!(" WHERE id = ${param_idx}"));

        let mut q = sqlx::query(&query).bind(now);

        if has_rating {
            q = q.bind(input.rating.expect("checked above") as i32);
        }
        if has_title {
            q = q.bind(input.title.expect("checked above"));
        }
        if has_body {
            q = q.bind(input.body.expect("checked above"));
        }
        if has_status {
            q = q.bind(input.status.expect("checked above").to_string());
        }

        q = q.bind(id);

        q.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn list_async(&self, filter: ReviewFilter) -> Result<Vec<Review>> {
        let mut query = String::from(
            "SELECT id, product_id, customer_id, rating, title, body, status, verified_purchase, helpful_count, report_count, created_at, updated_at
             FROM reviews WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<BindValue> = Vec::new();

        if let Some(product_id) = filter.product_id {
            query.push_str(&format!(" AND product_id = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Uuid(product_id.into_uuid()));
        }
        if let Some(customer_id) = filter.customer_id {
            query.push_str(&format!(" AND customer_id = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Uuid(customer_id.into_uuid()));
        }
        if let Some(status) = filter.status {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Str(status.to_string()));
        }
        if let Some(min_rating) = filter.min_rating {
            query.push_str(&format!(" AND rating >= ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Int(min_rating as i32));
        }
        if let Some(verified_only) = filter.verified_only {
            query.push_str(&format!(" AND verified_purchase = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Bool(verified_only));
        }
        let _ = param_idx;

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, ReviewRow>(&query);
        for bind in &binds {
            q = match bind {
                BindValue::Uuid(v) => q.bind(*v),
                BindValue::Str(v) => q.bind(v.as_str()),
                BindValue::Int(v) => q.bind(*v),
                BindValue::Bool(v) => q.bind(*v),
            };
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_review).collect()
    }

    async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM reviews WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn get_summary_async(&self, product_id: ProductId) -> Result<ReviewSummary> {
        let pid = product_id.into_uuid();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reviews WHERE product_id = $1 AND status = 'approved'",
        )
        .bind(pid)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let avg: f64 = sqlx::query_scalar(
            "SELECT COALESCE(AVG(rating::DOUBLE PRECISION), 0.0) FROM reviews WHERE product_id = $1 AND status = 'approved'",
        )
        .bind(pid)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        let dist_rows = sqlx::query_as::<_, RatingDistRow>(
            "SELECT rating, COUNT(*) AS cnt FROM reviews WHERE product_id = $1 AND status = 'approved' GROUP BY rating",
        )
        .bind(pid)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let mut distribution = [0u32; 5];
        for row in dist_rows {
            let idx = (row.rating - 1).clamp(0, 4) as usize;
            distribution[idx] = row.cnt as u32;
        }

        Ok(ReviewSummary {
            product_id,
            total_reviews: total as u64,
            average_rating: avg,
            rating_distribution: distribution,
        })
    }

    async fn mark_helpful_async(&self, id: Uuid) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE reviews SET helpful_count = helpful_count + 1, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    async fn mark_reported_async(&self, id: Uuid) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE reviews SET report_count = report_count + 1, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }
}

/// Internal enum for heterogeneous bind parameters
enum BindValue {
    Uuid(Uuid),
    Str(String),
    Int(i32),
    Bool(bool),
}

impl ReviewRepository for PgReviewRepository {
    fn create(&self, input: CreateReview) -> Result<Review> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ReviewId) -> Result<Option<Review>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn update(&self, id: ReviewId, input: UpdateReview) -> Result<Review> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: ReviewFilter) -> Result<Vec<Review>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ReviewId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn get_summary(&self, product_id: ProductId) -> Result<ReviewSummary> {
        super::block_on(self.get_summary_async(product_id))
    }

    fn mark_helpful(&self, id: ReviewId) -> Result<()> {
        super::block_on(self.mark_helpful_async(id.into_uuid()))
    }

    fn mark_reported(&self, id: ReviewId) -> Result<()> {
        super::block_on(self.mark_reported_async(id.into_uuid()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_review_parses_valid_status() {
        let now = Utc::now();
        let row = ReviewRow {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            rating: 4,
            title: Some("Great".into()),
            body: Some("Loved it".into()),
            status: "approved".into(),
            verified_purchase: true,
            helpful_count: 3,
            report_count: 1,
            created_at: now,
            updated_at: now,
        };

        let review = PgReviewRepository::row_to_review(row).unwrap();
        assert_eq!(review.rating, 4);
        assert_eq!(review.status, ReviewStatus::Approved);
        assert!(review.verified_purchase);
        assert_eq!(review.helpful_count, 3);
        assert_eq!(review.reported_count, 1);
    }

    #[test]
    fn row_to_review_rejects_invalid_status() {
        let now = Utc::now();
        let row = ReviewRow {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            rating: 3,
            title: None,
            body: None,
            status: "invalid_status".into(),
            verified_purchase: false,
            helpful_count: 0,
            report_count: 0,
            created_at: now,
            updated_at: now,
        };

        let result = PgReviewRepository::row_to_review(row);
        assert!(result.is_err());
    }

    #[test]
    fn row_to_review_handles_nullable_fields() {
        let now = Utc::now();
        let row = ReviewRow {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            rating: 1,
            title: None,
            body: None,
            status: "pending".into(),
            verified_purchase: false,
            helpful_count: 0,
            report_count: 0,
            created_at: now,
            updated_at: now,
        };

        let review = PgReviewRepository::row_to_review(row).unwrap();
        assert!(review.title.is_none());
        assert!(review.body.is_none());
        assert_eq!(review.status, ReviewStatus::Pending);
    }
}
