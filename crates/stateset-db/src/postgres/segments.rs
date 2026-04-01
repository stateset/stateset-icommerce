//! PostgreSQL implementation of customer segment repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateSegment, CustomerId, Result, Segment, SegmentFilter, SegmentId,
    SegmentMembership, SegmentRepository, SegmentRule, SegmentType, UpdateSegment,
};
use uuid::Uuid;

/// PostgreSQL segment repository
#[derive(Debug, Clone)]
pub struct PgSegmentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct SegmentRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    segment_type: String,
    rules: serde_json::Value,
    member_count: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct MembershipRow {
    segment_id: Uuid,
    customer_id: Uuid,
    joined_at: DateTime<Utc>,
}

impl PgSegmentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_segment(row: SegmentRow) -> Result<Segment> {
        let segment_type: SegmentType = row.segment_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid segment.segment_type '{}': {}",
                row.segment_type, e
            ))
        })?;

        let rules: Vec<SegmentRule> = serde_json::from_value(row.rules)
            .map_err(|e| CommerceError::DatabaseError(format!("Invalid segment.rules: {e}")))?;

        Ok(Segment {
            id: SegmentId::from(row.id),
            name: row.name,
            description: row.description,
            segment_type,
            rules,
            member_count: row.member_count as u64,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_membership(row: MembershipRow) -> SegmentMembership {
        SegmentMembership {
            segment_id: SegmentId::from(row.segment_id),
            customer_id: CustomerId::from(row.customer_id),
            joined_at: row.joined_at,
        }
    }

    // ---- async helpers ----

    async fn create_async(&self, input: CreateSegment) -> Result<Segment> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let rules_json = serde_json::to_value(&input.rules)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO segments (id, name, description, segment_type, rules, member_count, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, 0, $6, $7)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.segment_type.to_string())
        .bind(&rules_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn get_async(&self, id: Uuid) -> Result<Option<Segment>> {
        let row = sqlx::query_as::<_, SegmentRow>(
            "SELECT id, name, description, segment_type, rules, member_count, created_at, updated_at
             FROM segments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(r) => Ok(Some(Self::row_to_segment(r)?)),
            None => Ok(None),
        }
    }

    async fn update_async(&self, id: Uuid, input: UpdateSegment) -> Result<Segment> {
        let now = Utc::now();

        let mut param_idx = 2u32;
        let mut query = String::from("UPDATE segments SET updated_at = $1");
        let mut has_name = false;
        let mut has_description = false;
        let mut has_rules = false;

        if input.name.is_some() {
            query.push_str(&format!(", name = ${param_idx}"));
            param_idx += 1;
            has_name = true;
        }
        if input.description.is_some() {
            query.push_str(&format!(", description = ${param_idx}"));
            param_idx += 1;
            has_description = true;
        }
        if input.rules.is_some() {
            query.push_str(&format!(", rules = ${param_idx}"));
            param_idx += 1;
            has_rules = true;
        }

        query.push_str(&format!(" WHERE id = ${param_idx}"));

        let mut q = sqlx::query(&query).bind(now);

        if has_name {
            q = q.bind(input.name.expect("checked above"));
        }
        if has_description {
            q = q.bind(input.description.expect("checked above"));
        }
        if has_rules {
            let rules_json = serde_json::to_value(input.rules.expect("checked above"))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            q = q.bind(rules_json);
        }

        q = q.bind(id);

        q.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn list_async(&self, filter: SegmentFilter) -> Result<Vec<Segment>> {
        let mut query = String::from(
            "SELECT id, name, description, segment_type, rules, member_count, created_at, updated_at
             FROM segments WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut binds: Vec<BindValue> = Vec::new();

        if let Some(segment_type) = filter.segment_type {
            query.push_str(&format!(" AND segment_type = ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Str(segment_type.to_string()));
        }
        if let Some(ref name) = filter.name {
            query.push_str(&format!(" AND name ILIKE ${param_idx}"));
            param_idx += 1;
            binds.push(BindValue::Str(format!("%{name}%")));
        }
        let _ = param_idx;

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {offset}"));
        }

        let mut q = sqlx::query_as::<_, SegmentRow>(&query);
        for bind in &binds {
            q = match bind {
                BindValue::Str(v) => q.bind(v.as_str()),
            };
        }

        let rows = q.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_segment).collect()
    }

    async fn delete_async(&self, id: Uuid) -> Result<()> {
        // segment_memberships cascade via ON DELETE CASCADE
        sqlx::query("DELETE FROM segments WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    async fn add_member_async(
        &self,
        segment_id: Uuid,
        customer_id: Uuid,
    ) -> Result<SegmentMembership> {
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO segment_memberships (segment_id, customer_id, joined_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (segment_id, customer_id) DO NOTHING",
        )
        .bind(segment_id)
        .bind(customer_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Update cached member count
        sqlx::query(
            "UPDATE segments SET member_count = (SELECT COUNT(*) FROM segment_memberships WHERE segment_id = $1), updated_at = $2 WHERE id = $1",
        )
        .bind(segment_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, MembershipRow>(
            "SELECT segment_id, customer_id, joined_at FROM segment_memberships WHERE segment_id = $1 AND customer_id = $2",
        )
        .bind(segment_id)
        .bind(customer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Self::row_to_membership(row))
    }

    async fn remove_member_async(&self, segment_id: Uuid, customer_id: Uuid) -> Result<()> {
        let now = Utc::now();

        sqlx::query("DELETE FROM segment_memberships WHERE segment_id = $1 AND customer_id = $2")
            .bind(segment_id)
            .bind(customer_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        sqlx::query(
            "UPDATE segments SET member_count = (SELECT COUNT(*) FROM segment_memberships WHERE segment_id = $1), updated_at = $2 WHERE id = $1",
        )
        .bind(segment_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    async fn list_members_async(
        &self,
        segment_id: Uuid,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        let mut query = String::from(
            "SELECT segment_id, customer_id, joined_at FROM segment_memberships WHERE segment_id = $1 ORDER BY joined_at DESC",
        );

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {offset}"));
        }

        let rows = sqlx::query_as::<_, MembershipRow>(&query)
            .bind(segment_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_membership).collect())
    }

    async fn is_member_async(&self, segment_id: Uuid, customer_id: Uuid) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM segment_memberships WHERE segment_id = $1 AND customer_id = $2",
        )
        .bind(segment_id)
        .bind(customer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(count > 0)
    }

    async fn count_members_async(&self, segment_id: Uuid) -> Result<u64> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM segment_memberships WHERE segment_id = $1")
                .bind(segment_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

/// Internal enum for heterogeneous bind parameters
enum BindValue {
    Str(String),
}

impl SegmentRepository for PgSegmentRepository {
    fn create(&self, input: CreateSegment) -> Result<Segment> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: SegmentId) -> Result<Option<Segment>> {
        super::block_on(self.get_async(id.into_uuid()))
    }

    fn update(&self, id: SegmentId, input: UpdateSegment) -> Result<Segment> {
        super::block_on(self.update_async(id.into_uuid(), input))
    }

    fn list(&self, filter: SegmentFilter) -> Result<Vec<Segment>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: SegmentId) -> Result<()> {
        super::block_on(self.delete_async(id.into_uuid()))
    }

    fn add_member(
        &self,
        segment_id: SegmentId,
        customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        super::block_on(self.add_member_async(segment_id.into_uuid(), customer_id.into_uuid()))
    }

    fn remove_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<()> {
        super::block_on(self.remove_member_async(segment_id.into_uuid(), customer_id.into_uuid()))
    }

    fn list_members(
        &self,
        segment_id: SegmentId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        super::block_on(self.list_members_async(segment_id.into_uuid(), limit, offset))
    }

    fn is_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<bool> {
        super::block_on(self.is_member_async(segment_id.into_uuid(), customer_id.into_uuid()))
    }

    fn count_members(&self, segment_id: SegmentId) -> Result<u64> {
        super::block_on(self.count_members_async(segment_id.into_uuid()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_segment_parses_static_type() {
        let now = Utc::now();
        let row = SegmentRow {
            id: Uuid::new_v4(),
            name: "VIP Customers".into(),
            description: Some("High-value customers".into()),
            segment_type: "static".into(),
            rules: serde_json::json!([]),
            member_count: 42,
            created_at: now,
            updated_at: now,
        };

        let segment = PgSegmentRepository::row_to_segment(row).expect("should parse");
        assert_eq!(segment.name, "VIP Customers");
        assert_eq!(segment.segment_type, SegmentType::Static);
        assert_eq!(segment.member_count, 42);
        assert!(segment.rules.is_empty());
    }

    #[test]
    fn row_to_segment_parses_dynamic_type_with_rules() {
        let now = Utc::now();
        let rules = serde_json::json!([
            {"field": "total_orders", "operator": "gt", "value": "5"}
        ]);
        let row = SegmentRow {
            id: Uuid::new_v4(),
            name: "Frequent Buyers".into(),
            description: None,
            segment_type: "dynamic".into(),
            rules,
            member_count: 0,
            created_at: now,
            updated_at: now,
        };

        let segment = PgSegmentRepository::row_to_segment(row).expect("should parse");
        assert_eq!(segment.segment_type, SegmentType::Dynamic);
        assert_eq!(segment.rules.len(), 1);
        assert_eq!(segment.rules[0].field, "total_orders");
    }

    #[test]
    fn row_to_segment_rejects_invalid_type() {
        let now = Utc::now();
        let row = SegmentRow {
            id: Uuid::new_v4(),
            name: "Bad".into(),
            description: None,
            segment_type: "invalid_type".into(),
            rules: serde_json::json!([]),
            member_count: 0,
            created_at: now,
            updated_at: now,
        };

        let result = PgSegmentRepository::row_to_segment(row);
        assert!(result.is_err());
    }

    #[test]
    fn row_to_membership_converts_correctly() {
        let now = Utc::now();
        let seg_uuid = Uuid::new_v4();
        let cust_uuid = Uuid::new_v4();
        let row = MembershipRow { segment_id: seg_uuid, customer_id: cust_uuid, joined_at: now };

        let membership = PgSegmentRepository::row_to_membership(row);
        assert_eq!(membership.segment_id.into_uuid(), seg_uuid);
        assert_eq!(membership.customer_id.into_uuid(), cust_uuid);
        assert_eq!(membership.joined_at, now);
    }
}
