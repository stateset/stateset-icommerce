//! Postgres `agent_validation` must emit valid SQL.
//!
//! Same string-continuation token-fusion bug as `agent_reputation`: multi-line SQL
//! literals ended lines with a bare `\` (no leading space), fusing tokens like
//! `... created_at\` + `FROM ...` into `created_atFROM`, so validation
//! request/status reads failed at runtime on Postgres. This exercises
//! `request_validation` (which re-selects the inserted row) + `get_validation_status`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise. Plain `#[test]` because the sync repo methods block on their own
//! runtime.

#![cfg(feature = "postgres")]

use stateset_core::{
    AgentValidationRepository, CreateAgentValidationRequest, CreateAgentValidationResponse,
};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[test]
fn postgres_agent_validation_request_and_status_roundtrip() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping agent_validation SQL test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let db = rt.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let repo = db.agent_validation();

    let unique = uuid::Uuid::new_v4().simple().to_string();
    let request_hash = format!("0xhash{}", &unique[..16]);

    // request_validation re-selects the row it inserts (`... created_at\` + `FROM`).
    let request = repo
        .request_validation(CreateAgentValidationRequest {
            request_hash: request_hash.clone(),
            agent_registry: "reg".into(),
            agent_id: "agent-1".into(),
            validator_address: "0xvalidator".into(),
            request_uri: "ipfs://request".into(),
        })
        .expect("request_validation must succeed (was broken by malformed SQL on Postgres)");
    assert_eq!(request.request_hash, request_hash);

    // respond_validation re-selects the inserted response (`... created_at\` + `FROM`).
    repo.respond_validation(
        &request_hash,
        CreateAgentValidationResponse {
            response: 1,
            response_uri: Some("ipfs://response".into()),
            response_hash: Some("0xresp".into()),
            tag: None,
        },
    )
    .expect("respond_validation must succeed");

    // get_validation_status reads both the request and the latest response
    // (`... request_hash = $1\` + `ORDER BY`).
    let status =
        repo.get_validation_status(&request_hash).expect("get_validation_status must succeed");
    let status = status.expect("a responded validation must have a status");
    assert_eq!(status.response, 1);
    assert_eq!(status.agent_id, "agent-1");
}
