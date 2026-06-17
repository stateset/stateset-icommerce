//! SQLite implementation of the channel repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_json_row, parse_uuid_opt_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use stateset_core::{
    Channel, ChannelFilter, ChannelId, ChannelProductMapping, ChannelProductSyncItem,
    ChannelRepository, ChannelStatus, ChannelType, CommerceError, CreateChannel, Result,
    UpdateChannel,
};

#[derive(Debug)]
pub struct SqliteChannelRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteChannelRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_channel(row: &rusqlite::Row<'_>) -> rusqlite::Result<Channel> {
        let tags_json: String = row.get("tags")?;
        let metadata_json: String = row.get("metadata")?;
        Ok(Channel {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "channel", "id")?.into(),
            name: row.get("name")?,
            channel_type: parse_enum_row::<ChannelType>(
                &row.get::<_, String>("channel_type")?,
                "channel",
                "channel_type",
            )?,
            integration: row.get("integration")?,
            status: parse_enum_row::<ChannelStatus>(
                &row.get::<_, String>("status")?,
                "channel",
                "status",
            )?,
            api_locked: row.get::<_, i32>("api_locked")? != 0,
            default_warehouse_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("default_warehouse_id")?,
                "channel",
                "default_warehouse_id",
            )?
            .map(Into::into),
            tags: parse_json_row(&tags_json, "channel", "tags")?,
            metadata: parse_json_row(&metadata_json, "channel", "metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "channel",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "channel",
                "updated_at",
            )?,
        })
    }

    fn row_to_mapping(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChannelProductMapping> {
        Ok(ChannelProductMapping {
            channel_id: parse_uuid_row(
                &row.get::<_, String>("channel_id")?,
                "mapping",
                "channel_id",
            )?
            .into(),
            channel_sku: row.get("channel_sku")?,
            product_id: parse_uuid_row(
                &row.get::<_, String>("product_id")?,
                "mapping",
                "product_id",
            )?
            .into(),
            internal_sku: row.get("internal_sku")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "mapping",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "mapping",
                "updated_at",
            )?,
        })
    }

    fn json_err(e: serde_json::Error) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
            e.to_string(),
        )))
    }
}

impl ChannelRepository for SqliteChannelRepository {
    fn create(&self, input: CreateChannel) -> Result<Channel> {
        let id = ChannelId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&input.tags)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let metadata_json = serde_json::to_string(&input.metadata)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO channels (id, name, channel_type, integration, status, api_locked, default_warehouse_id, tags, metadata, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 'active', 0, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    input.channel_type.to_string(),
                    &input.integration,
                    input.default_warehouse_id.map(|w| w.to_string()),
                    &tags_json,
                    &metadata_json,
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM channels WHERE id = ?", [&id_str], Self::row_to_channel)
        })
    }

    fn get(&self, id: ChannelId) -> Result<Option<Channel>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM channels WHERE id = ?",
            [id.to_string()],
            Self::row_to_channel,
        ) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: ChannelId, input: UpdateChannel) -> Result<Channel> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            // Reject mutations on locked channels (reads stay allowed elsewhere).
            let locked: i32 = tx
                .query_row("SELECT api_locked FROM channels WHERE id = ?", [&id_str], |r| r.get(0))
                .optional()?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if locked != 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("channel is API-locked".into()),
                )));
            }

            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref integration) = input.integration {
                sets.push("integration = ?".into());
                params.push(Box::new(integration.clone()));
            }
            if let Some(status) = input.status {
                sets.push("status = ?".into());
                params.push(Box::new(status.to_string()));
            }
            if let Some(wh) = input.default_warehouse_id {
                sets.push("default_warehouse_id = ?".into());
                params.push(Box::new(wh.to_string()));
            }
            if let Some(ref tags) = input.tags {
                sets.push("tags = ?".into());
                params.push(Box::new(serde_json::to_string(tags).map_err(Self::json_err)?));
            }
            if let Some(ref metadata) = input.metadata {
                sets.push("metadata = ?".into());
                params.push(Box::new(serde_json::to_string(metadata).map_err(Self::json_err)?));
            }

            let sql = format!("UPDATE channels SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM channels WHERE id = ?", [&id_str], Self::row_to_channel)
        })
    }

    fn list(&self, filter: ChannelFilter) -> Result<Vec<Channel>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM channels WHERE status != 'deleted'".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(t) = filter.channel_type {
            sql.push_str(" AND channel_type = ?");
            params.push(Box::new(t.to_string()));
        }
        if let Some(s) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(s.to_string()));
        }
        if let Some(ref integration) = filter.integration {
            sql.push_str(" AND integration = ?");
            params.push(Box::new(integration.clone()));
        }
        if let Some(locked) = filter.api_locked {
            sql.push_str(" AND api_locked = ?");
            params.push(Box::new(locked as i32));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_channel)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: ChannelId) -> Result<()> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            let locked: i32 = tx
                .query_row("SELECT api_locked FROM channels WHERE id = ?", [&id_str], |r| r.get(0))
                .optional()?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if locked != 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("channel is API-locked".into()),
                )));
            }
            tx.execute("UPDATE channels SET status = 'deleted' WHERE id = ?", [&id_str])?;
            Ok(())
        })
    }

    fn set_lock(&self, id: ChannelId, locked: bool) -> Result<Channel> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE channels SET api_locked = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![locked as i32, &now_str, &id_str],
            )?;
            tx.query_row("SELECT * FROM channels WHERE id = ?", [&id_str], Self::row_to_channel)
        })
    }

    fn sync_products(&self, id: ChannelId, items: Vec<ChannelProductSyncItem>) -> Result<u64> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut affected: u64 = 0;
            for item in &items {
                if item.delete {
                    affected += tx.execute(
                        "DELETE FROM channel_product_mappings WHERE channel_id = ? AND channel_sku = ?",
                        rusqlite::params![&id_str, &item.channel_sku],
                    )? as u64;
                } else {
                    let product_id = item.product_id.ok_or_else(|| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(
                            CommerceError::ValidationError(
                                "product_id is required when delete is false".into(),
                            ),
                        ))
                    })?;
                    let internal_sku = item.internal_sku.clone().unwrap_or_default();
                    affected += tx.execute(
                        "INSERT INTO channel_product_mappings (channel_id, channel_sku, product_id, internal_sku, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?)
                         ON CONFLICT(channel_id, channel_sku) DO UPDATE SET
                            product_id = excluded.product_id,
                            internal_sku = excluded.internal_sku,
                            updated_at = excluded.updated_at",
                        rusqlite::params![
                            &id_str,
                            &item.channel_sku,
                            product_id.to_string(),
                            &internal_sku,
                            &now_str,
                            &now_str,
                        ],
                    )? as u64;
                }
            }
            Ok(affected)
        })
    }

    fn list_product_mappings(&self, id: ChannelId) -> Result<Vec<ChannelProductMapping>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM channel_product_mappings WHERE channel_id = ? ORDER BY channel_sku",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_mapping)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::ProductId;

    fn test_repo() -> SqliteChannelRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteChannelRepository::new(db.pool().clone())
    }

    fn new_channel(repo: &SqliteChannelRepository, name: &str, t: ChannelType) -> Channel {
        repo.create(CreateChannel {
            name: name.into(),
            channel_type: t,
            integration: Some("shopify".into()),
            default_warehouse_id: None,
            tags: vec!["retail".into()],
            metadata: serde_json::json!({"k":"v"}),
        })
        .expect("create channel")
    }

    #[test]
    fn create_and_get() {
        let repo = test_repo();
        let c = new_channel(&repo, "Shopify US", ChannelType::SalesChannel);
        assert_eq!(c.name, "Shopify US");
        assert!(!c.api_locked);
        let fetched = repo.get(c.id).expect("get").expect("found");
        assert_eq!(fetched.id, c.id);
        assert_eq!(fetched.tags, vec!["retail".to_string()]);
    }

    #[test]
    fn lock_blocks_update_and_delete() {
        let repo = test_repo();
        let c = new_channel(&repo, "Locked", ChannelType::SalesChannel);
        repo.set_lock(c.id, true).expect("lock");
        let upd = repo.update(c.id, UpdateChannel { name: Some("x".into()), ..Default::default() });
        assert!(upd.is_err(), "update on locked channel must fail");
        assert!(repo.delete(c.id).is_err(), "delete on locked channel must fail");
        repo.set_lock(c.id, false).expect("unlock");
        assert!(
            repo.update(c.id, UpdateChannel { name: Some("ok".into()), ..Default::default() })
                .is_ok()
        );
    }

    #[test]
    fn list_filters_and_excludes_deleted() {
        let repo = test_repo();
        new_channel(&repo, "A", ChannelType::SalesChannel);
        let b = new_channel(&repo, "B", ChannelType::FulfillmentChannel);
        let all = repo.list(ChannelFilter::default()).expect("list");
        assert_eq!(all.len(), 2);
        let only_fc = repo
            .list(ChannelFilter {
                channel_type: Some(ChannelType::FulfillmentChannel),
                ..Default::default()
            })
            .expect("list fc");
        assert_eq!(only_fc.len(), 1);
        repo.delete(b.id).expect("delete");
        assert_eq!(repo.list(ChannelFilter::default()).expect("list").len(), 1);
    }

    #[test]
    fn sync_products_upsert_and_delete() {
        let repo = test_repo();
        let c = new_channel(&repo, "Sync", ChannelType::SalesChannel);
        let pid = ProductId::new();
        let n = repo
            .sync_products(
                c.id,
                vec![ChannelProductSyncItem {
                    channel_sku: "EXT-1".into(),
                    product_id: Some(pid),
                    internal_sku: Some("SKU-1".into()),
                    delete: false,
                }],
            )
            .expect("sync");
        assert_eq!(n, 1);
        let mappings = repo.list_product_mappings(c.id).expect("mappings");
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].internal_sku, "SKU-1");

        // delete it
        repo.sync_products(
            c.id,
            vec![ChannelProductSyncItem {
                channel_sku: "EXT-1".into(),
                product_id: None,
                internal_sku: None,
                delete: true,
            }],
        )
        .expect("sync delete");
        assert_eq!(repo.list_product_mappings(c.id).expect("mappings").len(), 0);
    }

    #[test]
    fn sync_requires_product_when_not_deleting() {
        let repo = test_repo();
        let c = new_channel(&repo, "Sync2", ChannelType::SalesChannel);
        let res = repo.sync_products(
            c.id,
            vec![ChannelProductSyncItem {
                channel_sku: "EXT-9".into(),
                product_id: None,
                internal_sku: None,
                delete: false,
            }],
        );
        assert!(res.is_err());
    }
}
