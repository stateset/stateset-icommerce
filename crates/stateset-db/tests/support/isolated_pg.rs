//! A private, freshly migrated Postgres database for one test.
//!
//! Tests that assert a store-wide property — above all that the whole kernel
//! receipt audit chain verifies — cannot share a database with every other
//! suite: any concurrently running test that is mid-way through a deliberate
//! tamper, or that leaves an unrelated chain gap, fails them. This helper
//! creates `ss_iso_<uuid>` next to the configured database, migrates it, and
//! drops it again when the test is done.

use stateset_db::PostgresDatabase;
use uuid::Uuid;

/// An isolated database and the admin URL needed to drop it.
pub(crate) struct IsolatedPostgres {
    pub(crate) db: PostgresDatabase,
    pub(crate) admin_url: String,
    pub(crate) name: String,
}

/// Create and migrate a private database derived from `base_url`.
pub(crate) async fn isolated_postgres(base_url: &str) -> IsolatedPostgres {
    let name = format!("ss_iso_{}", Uuid::new_v4().simple());
    let admin = sqlx::PgPool::connect(base_url).await.expect("connect to the base database");
    sqlx::query(&format!("CREATE DATABASE \"{name}\""))
        .execute(&admin)
        .await
        .expect("create isolated database");
    admin.close().await;
    let db = PostgresDatabase::connect(with_database(base_url, &name))
        .await
        .expect("connect and migrate the isolated database");
    IsolatedPostgres { db, admin_url: base_url.to_string(), name }
}

impl IsolatedPostgres {
    /// Close the pool and drop the database.
    // Unused by suites that move the handle into a blocking task.
    #[allow(dead_code)]
    pub(crate) async fn drop_database(self) {
        self.db.pool().close().await;
        drop_isolated_database(&self.admin_url, &self.name).await;
    }
}

/// Drop an isolated database by name (for tests that moved the handle away).
pub(crate) async fn drop_isolated_database(admin_url: &str, name: &str) {
    if let Ok(admin) = sqlx::PgPool::connect(admin_url).await {
        // `DROP DATABASE ... WITH (FORCE)` needs Postgres 13; terminating the
        // remaining sessions first works on every supported version.
        let _ = sqlx::query(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE datname = $1 AND pid <> pg_backend_pid()",
        )
        .bind(name)
        .execute(&admin)
        .await;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{name}\"")).execute(&admin).await;
        admin.close().await;
    }
}

/// `base_url` with its database name replaced by `name`, keeping any query.
fn with_database(base_url: &str, name: &str) -> String {
    let (head, query) = match base_url.split_once('?') {
        Some((head, query)) => (head, Some(query)),
        None => (base_url, None),
    };
    let authority_end = head.find("://").map_or(0, |i| i + 3);
    let path_start = head[authority_end..].find('/').map_or(head.len(), |i| authority_end + i);
    let mut url = format!("{}/{name}", &head[..path_start]);
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    url
}
