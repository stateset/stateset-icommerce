//! Postgres `agent_reputation` must emit valid SQL.
//!
//! Several queries were written as multi-line string literals whose lines ended with
//! a bare `\` (Rust string-continuation), with no space before it — so
//! `... agent_feedback\` + newline + `WHERE ...` fused into `agent_feedbackWHERE`,
//! producing a syntax error. Every feedback write/read failed at runtime on Postgres
//! while SQLite worked. This exercises `give_feedback` + `read_all_feedback` end to
//! end.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise. Uses a plain `#[test]` (not `#[tokio::test]`) because the sync repo
//! methods block on their own runtime and refuse to run inside an ambient one.

#![cfg(feature = "postgres")]

use stateset_core::{AgentFeedbackFilter, AgentReputationRepository, CreateAgentFeedback};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[test]
fn postgres_agent_reputation_feedback_roundtrips() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping agent_reputation SQL test");
        return;
    };
    // Keep the connect runtime alive for the whole test; the sync repo methods drive
    // their queries on a separate static runtime.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let db = rt.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let repo = db.agent_reputation();

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let registry = format!("reg-{}", &unique[..12]);

    // give_feedback uses `... FROM agent_feedback\` + `WHERE ...` (was fused).
    let feedback = repo
        .give_feedback(CreateAgentFeedback {
            agent_registry: registry.clone(),
            agent_id: "agent-1".into(),
            client_address: "0xclient".into(),
            value: 100,
            value_decimals: 0,
            tag1: None,
            tag2: None,
            endpoint: None,
            feedback_uri: None,
            feedback_hash: None,
        })
        .expect("give_feedback must succeed (was broken by malformed SQL on Postgres)");
    assert_eq!(feedback.feedback_index, 1);

    // read_all_feedback uses `... revoked_at\` + `FROM ...` (was fused).
    let all = repo
        .read_all_feedback(AgentFeedbackFilter {
            agent_registry: Some(registry),
            agent_id: Some("agent-1".into()),
            ..Default::default()
        })
        .expect("read_all_feedback must succeed");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].feedback_index, feedback.feedback_index);
}
