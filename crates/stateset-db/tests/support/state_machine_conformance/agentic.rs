//! Agent-commerce machines: A2A agent messages.

use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

use super::harness::{Conformance, Report, SetStatus};

pub(crate) fn a2a_message(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let tenant = format!("sm-{}", Uuid::new_v4().simple());
    let repo = db.a2a_messages();
    // Two attempts: the first failure leaves the message pending for a retry,
    // the second exhausts it.
    let create = || {
        repo.send_message(SendA2AAgentMessage {
            tenant_id: tenant.clone(),
            conversation_id: None,
            from_agent_id: Uuid::new_v4(),
            to_agent_id: Uuid::new_v4(),
            message_type: "quote.request".into(),
            payload: serde_json::json!({}),
            max_attempts: Some(2),
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_message(&tenant, id).unwrap().unwrap().status.to_string();
    let fail_once = |id| repo.fail_message(&tenant, id, "unreachable").map(drop);
    Conformance::new("a2a_message", create, status)
        .op("acknowledged", "acknowledge_message", |id| {
            repo.acknowledge_message(&tenant, id).map(drop)
        })
        .op("pending", "fail_message (attempts left)", fail_once)
        .op("failed", "fail_message until attempts run out", |id| {
            fail_once(id)?;
            if repo.get_message(&tenant, id).unwrap().unwrap().status
                == A2AAgentMessageStatus::Pending
            {
                fail_once(id)?;
            }
            Ok(())
        })
        .seeded("delivered", |id| set("a2a_agent_messages", &id.to_string(), "delivered"))
        .seeded("expired", |id| set("a2a_agent_messages", &id.to_string(), "expired"))
        .check()
}
