//! Substring search treats the user's term literally on both backends.
//!
//! Search filters build `LIKE '%term%'` patterns. Without escaping, a term
//! containing `%` or `_` acted as a wildcard: searching `50%` also matched
//! `500 off`. Both backends now escape the term (SQLite additionally needs an
//! explicit `ESCAPE '\'`, Postgres uses backslash by default).

use stateset_core::{CreateSegment, SegmentFilter, SegmentType};
use uuid::Uuid;

const fn segment(name: String) -> CreateSegment {
    CreateSegment { name, description: None, segment_type: SegmentType::Static, rules: Vec::new() }
}

/// `(literal, decoy)` names sharing a unique tag so a shared database's other
/// rows cannot match.
fn names() -> (String, String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (format!("{tag} 50% off"), format!("{tag} 500 off"), format!("{tag} 50%"))
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_search_treats_percent_and_underscore_literally() {
    use stateset_core::SegmentRepository;
    use stateset_db::SqliteDatabase;

    let db = SqliteDatabase::in_memory().expect("sqlite");
    let (literal, decoy, term) = names();
    db.segments().create(segment(literal.clone())).expect("create literal");
    db.segments().create(segment(decoy)).expect("create decoy");
    let found = db
        .segments()
        .list(SegmentFilter { name: Some(term), ..Default::default() })
        .expect("search");
    assert_eq!(found.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), vec![literal]);

    let underscore = db
        .segments()
        .list(SegmentFilter { name: Some("5_% off".into()), ..Default::default() })
        .expect("search");
    assert!(underscore.is_empty(), "`_` must not match any single character");
}

#[cfg(feature = "postgres")]
#[test]
fn postgres_search_treats_percent_and_underscore_literally() {
    use stateset_core::SegmentRepository;
    use stateset_db::PostgresDatabase;

    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    // Connect on a runtime, then use the sync repository API outside it (its
    // blocking bridge refuses to run inside one).
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let db = runtime.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let (literal, decoy, term) = names();
    db.segments().create(segment(literal.clone())).expect("create literal");
    db.segments().create(segment(decoy)).expect("create decoy");
    let found = db
        .segments()
        .list(SegmentFilter { name: Some(term), ..Default::default() })
        .expect("search");
    assert_eq!(found.iter().map(|s| s.name.clone()).collect::<Vec<_>>(), vec![literal]);
}
