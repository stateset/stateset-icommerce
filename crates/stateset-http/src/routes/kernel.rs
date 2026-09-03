//! Kernel receipt audit-chain endpoints.
//!
//! Every governed kernel command seals its receipt into an append-only hash
//! chain. These read-only endpoints recompute that chain and mint portable
//! checkpoints operators can retain outside the database. Both backends seal
//! the same chain, so both answer these endpoints.
//!
//! # Authorization
//!
//! These are ordinary `/api/v1` routes: they run behind bearer authentication
//! and the fail-closed authorization middleware, which maps them to the
//! `kernel` resource with `Action::Read`, in addition to `x-tenant-id`
//! routing. No further gate is added here — the chain exposes only hashes,
//! counts and a head pointer (never receipt payloads), and operators need to
//! be able to verify it. A deployment that wants to restrict verification to
//! auditors grants `kernel:read` to that role alone.

use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};

/// Result of recomputing the kernel receipt audit chain.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KernelAuditVerificationResponse {
    /// Whether every link and every materialized receipt verified.
    pub valid: bool,
    /// Number of sealed receipts in the chain.
    pub entries: u64,
    /// Hash of the newest sealed receipt, if any.
    pub head_hash: Option<String>,
    /// First chain position that failed verification, if any.
    pub first_invalid_sequence: Option<i64>,
}

/// Portable, self-hashed checkpoint of the audit chain head.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KernelAuditCheckpointResponse {
    pub contract_version: String,
    pub algorithm: String,
    pub entries: u64,
    pub head_hash: Option<String>,
    pub generated_at: String,
    pub checkpoint_hash: String,
}

/// Build the kernel sub-router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/kernel/audit", get(verify_audit_chain))
        .route("/kernel/audit/checkpoint", get(audit_checkpoint))
}

/// `GET /api/v1/kernel/audit` — recompute the receipt audit chain.
#[utoipa::path(get, operation_id = "kernel_verify_audit_chain", path = "/api/v1/kernel/audit", tag = "kernel",
    responses((status = 200, description = "Audit chain verification", body = KernelAuditVerificationResponse),
        (status = 500, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn verify_audit_chain(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<KernelAuditVerificationResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    // Verification is synchronous on both backends (the Postgres path bridges
    // to async through the shared runtime, which must not be entered from a
    // Tokio worker), so it runs on a blocking thread.
    let verification = state
        .run_blocking(tenant_id.as_deref(), |commerce| {
            Ok(commerce.kernel_audit()?.verify_chain()?)
        })
        .await?;
    Ok(Json(KernelAuditVerificationResponse {
        valid: verification.valid,
        entries: verification.entries,
        head_hash: verification.head_hash,
        first_invalid_sequence: verification.first_invalid_sequence,
    }))
}

/// `GET /api/v1/kernel/audit/checkpoint` — mint a portable chain checkpoint.
///
/// Fails with `400` when the local chain does not verify: a checkpoint of a
/// broken chain would launder the break.
#[utoipa::path(get, operation_id = "kernel_audit_checkpoint", path = "/api/v1/kernel/audit/checkpoint", tag = "kernel",
    responses((status = 200, description = "Portable audit checkpoint", body = KernelAuditCheckpointResponse),
        (status = 400, body = ErrorBody), (status = 500, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn audit_checkpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<KernelAuditCheckpointResponse>, HttpError> {
    let tenant_id = tenant_id_from_headers(&headers);
    let checkpoint = state
        .run_blocking(tenant_id.as_deref(), |commerce| Ok(commerce.kernel_audit()?.checkpoint()?))
        .await?;
    Ok(Json(KernelAuditCheckpointResponse {
        contract_version: checkpoint.contract_version,
        algorithm: checkpoint.algorithm,
        entries: checkpoint.entries,
        head_hash: checkpoint.head_hash,
        generated_at: checkpoint.generated_at.to_rfc3339(),
        checkpoint_hash: checkpoint.checkpoint_hash,
    }))
}
