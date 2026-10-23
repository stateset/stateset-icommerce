//! A private, freshly created Postgres database for one test, dropped
//! afterwards, so assertions about "every row" cannot see other suites' rows.

use sqlx::Connection;

/// A freshly created, empty database, dropped when the value is dropped.
pub(crate) struct FreshDatabase {
    admin_url: String,
    name: String,
    pub(crate) url: String,
}

impl FreshDatabase {
    pub(crate) async fn create(admin_url: &str) -> Self {
        let name = format!("stateset_fresh_{}", uuid::Uuid::new_v4().simple());
        let mut conn = sqlx::PgConnection::connect(admin_url).await.expect("connect admin");
        sqlx::raw_sql(&format!("CREATE DATABASE \"{name}\""))
            .execute(&mut conn)
            .await
            .expect("create fresh database");
        conn.close().await.ok();
        let url = with_database(admin_url, &name);
        Self { admin_url: admin_url.to_string(), name, url }
    }
}

impl Drop for FreshDatabase {
    fn drop(&mut self) {
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        // Drop from a separate runtime: the test's runtime is shutting down.
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
            rt.block_on(async {
                let mut conn = sqlx::PgConnection::connect(&admin_url).await.ok()?;
                // Terminate stragglers first: `DROP ... WITH (FORCE)` needs PG13+.
                sqlx::query(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
                     WHERE datname = $1 AND pid <> pg_backend_pid()",
                )
                .bind(&name)
                .execute(&mut conn)
                .await
                .ok()?;
                sqlx::raw_sql(&format!("DROP DATABASE IF EXISTS \"{name}\""))
                    .execute(&mut conn)
                    .await
                    .ok()
            })
        })
        .join();
    }
}

/// `url` with its database name replaced by `name`.
fn with_database(url: &str, name: &str) -> String {
    let (base, query) = url.split_once('?').map_or((url, None), |(b, q)| (b, Some(q)));
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let path_start = base[scheme_end..].find('/').map_or(base.len(), |i| scheme_end + i);
    let mut out = format!("{}/{name}", &base[..path_start]);
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    out
}
