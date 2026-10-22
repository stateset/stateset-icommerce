use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stateset_crypto::{ZERO_HASH, hash::compute_payload_plain_hash};
use uuid::Uuid;

use crate::commitment::CommitmentManifest;
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::event::{KernelMetadata, SyncEvent};
use crate::transport::{
    PullPage, PullResult, PushAcknowledgement, PushRejection, PushResult, RemoteHead, Transport,
    derive_next_cursor,
};

const DEFAULT_VES_VERSION: u32 = 1;

/// HTTP transport for the documented StateSet sequencer REST API.
///
/// This mirrors the JS REST client by pushing signed VES envelopes to
/// `/api/v1/ves/events/ingest` and pulling canonical events from
/// `/api/v1/events`.
#[derive(Clone)]
pub struct SequencerHttpTransport {
    client: Client,
    base_url: String,
    agent_id: String,
    tenant_id: String,
    store_id: String,
    api_key: Option<String>,
    bearer_token: Option<String>,
    agent_key_id: u32,
    trusted_agent_keys: Vec<TrustedAgentKey>,
}

/// An agent signing key pinned for verifying pulled events.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrustedAgentKey {
    source_agent_id: String,
    agent_key_id: u32,
    public_key: [u8; 32],
}

impl std::fmt::Debug for SequencerHttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |secret: &Option<String>| secret.as_ref().map(|_| "<redacted>");
        f.debug_struct("SequencerHttpTransport")
            .field("base_url", &self.base_url)
            .field("agent_id", &self.agent_id)
            .field("tenant_id", &self.tenant_id)
            .field("store_id", &self.store_id)
            .field("api_key", &redact(&self.api_key))
            .field("bearer_token", &redact(&self.bearer_token))
            .field("agent_key_id", &self.agent_key_id)
            .field("trusted_agent_keys", &self.trusted_agent_keys.len())
            .finish_non_exhaustive()
    }
}

impl SequencerHttpTransport {
    /// Create a transport from explicit identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] when the base URL or identifiers are invalid.
    pub fn new(
        base_url: impl Into<String>,
        agent_id: impl Into<String>,
        tenant_id: impl Into<String>,
        store_id: impl Into<String>,
    ) -> Result<Self, SyncError> {
        Self::with_client(
            Client::new(),
            base_url,
            agent_id.into(),
            tenant_id.into(),
            store_id.into(),
        )
    }

    /// Create a transport using the identifiers in [`SyncConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidConfig`] when the base URL or config identifiers are invalid.
    pub fn from_config(
        base_url: impl Into<String>,
        config: &SyncConfig,
    ) -> Result<Self, SyncError> {
        Self::new(
            base_url,
            config.agent_id.clone(),
            config.tenant_id.clone(),
            config.store_id.clone(),
        )
    }

    fn with_client(
        client: Client,
        base_url: impl Into<String>,
        agent_id: String,
        tenant_id: String,
        store_id: String,
    ) -> Result<Self, SyncError> {
        if agent_id.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "sequencer transport agent_id must not be empty".into(),
            ));
        }
        if tenant_id.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "sequencer transport tenant_id must not be empty".into(),
            ));
        }
        if store_id.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "sequencer transport store_id must not be empty".into(),
            ));
        }

        Ok(Self {
            client,
            base_url: normalize_base_url(&base_url.into())?,
            agent_id,
            tenant_id,
            store_id,
            api_key: None,
            bearer_token: None,
            agent_key_id: 0,
            trusted_agent_keys: Vec::new(),
        })
    }

    /// Configure an API key for the transport.
    ///
    /// Keys with the `ss_` prefix are sent via `x-api-key`; all others use
    /// `Authorization: Bearer`.
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Configure a bearer token for the transport.
    #[must_use]
    pub fn with_bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.bearer_token = Some(bearer_token.into());
        self
    }

    /// Pin an agent's Ed25519 public key for verifying pulled events.
    ///
    /// Once at least one key is pinned, every pulled event must carry a
    /// signature from a pinned `(source_agent_id, agent_key_id)` that
    /// verifies over the VES event signing hash, computed with this
    /// transport's tenant and store. Any other event fails the whole page, so
    /// a sequencer (or anything between it and this client) cannot inject or
    /// rewrite events. With no pinned keys pulled signatures are not checked;
    /// payload hashes always are.
    #[must_use]
    pub fn with_trusted_agent_key(
        mut self,
        source_agent_id: impl Into<String>,
        agent_key_id: u32,
        public_key: [u8; 32],
    ) -> Self {
        self.trusted_agent_keys.push(TrustedAgentKey {
            source_agent_id: source_agent_id.into(),
            agent_key_id,
            public_key,
        });
        self
    }

    /// Configure the agent key id that corresponds to event signatures.
    #[must_use]
    pub const fn with_agent_key_id(mut self, agent_key_id: u32) -> Self {
        self.agent_key_id = agent_key_id;
        self
    }

    /// Return the normalized sequencer base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Return the configured source agent id.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// Return the configured tenant id.
    #[must_use]
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Return the configured store id.
    #[must_use]
    pub fn store_id(&self) -> &str {
        &self.store_id
    }

    /// Return the configured agent key id used for event signatures.
    #[must_use]
    pub const fn agent_key_id(&self) -> u32 {
        self.agent_key_id
    }

    /// Return whether an API key is configured for outbound requests.
    #[must_use]
    pub const fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Return whether a bearer token is configured for outbound requests.
    #[must_use]
    pub const fn has_bearer_token(&self) -> bool {
        self.bearer_token.is_some()
    }

    /// Probe the sequencer health endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] when the sequencer is unreachable.
    pub async fn healthcheck(&self) -> Result<(), SyncError> {
        let request = self.authorized(self.client.request(Method::GET, self.endpoint("/health")?));
        self.send(request).await?;
        Ok(())
    }

    fn endpoint(&self, path: &str) -> Result<Url, SyncError> {
        Url::parse(&format!("{}{}", self.base_url, path)).map_err(|error| {
            SyncError::Transport(format!("failed to build sequencer endpoint `{path}`: {error}"))
        })
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(api_key) = self.api_key.as_deref() {
            if api_key.starts_with("ss_") {
                return request.header("x-api-key", api_key);
            }
            return request.bearer_auth(api_key);
        }
        if let Some(bearer_token) = self.bearer_token.as_deref() {
            return request.bearer_auth(bearer_token);
        }
        request
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, SyncError> {
        let response = request
            .send()
            .await
            .map_err(|error| SyncError::Transport(format!("sequencer request failed: {error}")))?;
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let body = match response.text().await {
            Ok(text) => text,
            Err(error) => format!("<unreadable response body: {error}>"),
        };
        Err(SyncError::Transport(format!("sequencer request failed: {status} {body}")))
    }

    fn build_envelope(&self, event: &SyncEvent) -> Result<VesEventEnvelope, SyncError> {
        if event.is_canonical_remote() {
            return Err(SyncError::InvalidEvent(
                "cannot push a canonical remote event back to the sequencer".into(),
            ));
        }

        let Some(signature) = event.signature.clone() else {
            return Err(SyncError::InvalidEvent(format!(
                "event {} is missing an agent signature required for sequencer push",
                event.id
            )));
        };

        let payload_plain_hash =
            compute_payload_plain_hash(&event.payload, None).map(hex::encode).map_err(|error| {
                SyncError::InvalidEvent(format!(
                    "failed to compute VES payload hash for event {}: {error}",
                    event.id
                ))
            })?;

        Ok(VesEventEnvelope {
            event_id: event.id,
            command_id: event.command_id.clone(),
            tenant_id: self.tenant_id.clone(),
            store_id: self.store_id.clone(),
            entity_type: event.entity_type.clone(),
            entity_id: event.entity_id.clone(),
            event_type: event.event_type.clone(),
            ves_version: DEFAULT_VES_VERSION,
            payload: Some(event.payload.clone()),
            payload_kind: 0,
            payload_encrypted: None,
            payload_plain_hash,
            payload_cipher_hash: hex::encode(ZERO_HASH),
            agent_key_id: event.agent_key_id.unwrap_or(self.agent_key_id),
            agent_signature: signature,
            agent_signature_scheme: event.agent_signature_scheme,
            agent_signature_bundle: event.agent_signature_bundle.clone(),
            source_agent_id: event.source_agent_id.clone().unwrap_or_else(|| self.agent_id.clone()),
            base_version: event.base_version,
            kernel: event.kernel.clone().filter(|kernel| !kernel.is_empty()),
            created_at: event.timestamp.to_rfc3339(),
        })
    }

    fn synthesise_acknowledgements(
        &self,
        events: &[SyncEvent],
        response: &IngestResponse,
    ) -> Result<Vec<PushAcknowledgement>, SyncError> {
        let accepted = response.events_accepted.min(events.len());
        if accepted == 0 {
            return Ok(Vec::new());
        }

        let rejections = response.push_rejections()?;
        let Some(sequence_start) = response.sequence_start else {
            if rejections.is_empty() {
                return Ok(Vec::new());
            }
            return Err(SyncError::Transport(
                "sequencer returned partial acceptance without sequence_start metadata".into(),
            ));
        };

        let rejected_ids =
            rejections.iter().map(|rejection| rejection.event_id).collect::<HashSet<_>>();
        let receipt_handle = response.receipt_handle();
        let mut acknowledgements = Vec::with_capacity(accepted);
        let mut remote_sequence = sequence_start;

        for event in events {
            if rejected_ids.contains(&event.id) {
                continue;
            }
            if acknowledgements.len() == accepted {
                break;
            }

            let mut acknowledgement = PushAcknowledgement::new(event.id, remote_sequence);
            if let Some(receipt_handle) = receipt_handle.as_deref() {
                acknowledgement = acknowledgement.with_receipt(receipt_handle);
            }
            acknowledgements.push(acknowledgement);
            remote_sequence = remote_sequence.saturating_add(1);
        }

        if acknowledgements.len() != accepted {
            return Err(SyncError::Transport(format!(
                "sequencer accepted {accepted} events but only {} local events could be matched",
                acknowledgements.len()
            )));
        }

        Ok(acknowledgements)
    }

    async fn fetch_pull_page(&self, since: u64, limit: usize) -> Result<PullPage, SyncError> {
        let mut url = self.endpoint("/api/v1/events")?;
        url.query_pairs_mut()
            .append_pair("tenant_id", &self.tenant_id)
            .append_pair("store_id", &self.store_id)
            .append_pair("from", &since.saturating_add(1).to_string())
            .append_pair("limit", &limit.to_string());

        let request = self.authorized(self.client.request(Method::GET, url));
        let response = self.send(request).await?;
        let response: PullResponse = response.json().await.map_err(|error| {
            SyncError::Transport(format!("failed to decode sequencer pull response: {error}"))
        })?;

        let events = response
            .events
            .into_iter()
            .map(|event| self.map_pulled_event(event))
            .collect::<Result<Vec<_>, _>>()?;
        let observed_cursor = derive_next_cursor(since, &events);
        let remote_head =
            response.head_sequence.unwrap_or_else(|| observed_cursor.unwrap_or(since));
        let next_cursor =
            if response.has_more { response.next_sequence.or(observed_cursor) } else { None };

        Ok(PullPage {
            result: PullResult { events, remote_head, has_more: response.has_more },
            next_cursor,
            observed_cursor,
        })
    }

    async fn fetch_head_state(&self) -> Result<RemoteHead, SyncError> {
        let mut url = self.endpoint("/api/v1/head")?;
        url.query_pairs_mut()
            .append_pair("tenant_id", &self.tenant_id)
            .append_pair("store_id", &self.store_id);

        let request = self.authorized(self.client.request(Method::GET, url));
        let response = self.send(request).await?;
        let response: HeadResponse = response.json().await.map_err(|error| {
            SyncError::Transport(format!("failed to decode sequencer head response: {error}"))
        })?;

        let mut head = RemoteHead::new(response.head_sequence.unwrap_or(0));
        if let Some(state_root) = response.state_root {
            head = head.with_state_root(state_root);
        }
        if let Some(latest_commitment) = response.latest_commitment {
            if let Some(commitment_id) = latest_commitment.commitment_id() {
                head = head.with_last_commitment_id(commitment_id);
            }
            if let Some(manifest) =
                latest_commitment.to_manifest(head.state_root.as_deref(), Some(head.remote_head))?
            {
                head = head.with_commitment_manifest(manifest);
            }
        }
        Ok(head)
    }

    /// Verify a pulled event's Ed25519 signature against the pinned keys.
    fn verify_pulled_signature(
        &self,
        event: &SyncEvent,
        created_at: &str,
    ) -> Result<(), SyncError> {
        let reject = |reason: &str| {
            Err(SyncError::Transport(format!(
                "sequencer event {} failed signature verification: {reason}",
                event.id
            )))
        };
        let (Some(source_agent_id), Some(agent_key_id)) =
            (event.source_agent_id.as_deref(), event.agent_key_id)
        else {
            return reject("missing source_agent_id or agent_key_id");
        };
        let Some(key) = self
            .trusted_agent_keys
            .iter()
            .find(|key| key.source_agent_id == source_agent_id && key.agent_key_id == agent_key_id)
        else {
            return reject(&format!("agent `{source_agent_id}` key {agent_key_id} is not pinned"));
        };
        let Some(signature) = event.signature.as_deref() else {
            return reject("missing agent_signature");
        };
        let Ok(signature) = <[u8; 64]>::try_from(
            hex::decode(strip_hex_prefix(signature)).unwrap_or_default().as_slice(),
        ) else {
            return reject("agent_signature is not 64 hex-encoded bytes");
        };
        let Ok(payload_plain_hash) =
            <[u8; 32]>::try_from(hex::decode(&event.hash).unwrap_or_default().as_slice())
        else {
            return reject("payload hash is not 32 hex-encoded bytes");
        };
        let event_id = event.id.to_string();
        let signing_hash = stateset_crypto::hash::compute_event_signing_hash(
            &stateset_crypto::hash::EventSigningParams {
                ves_version: DEFAULT_VES_VERSION,
                tenant_id: &self.tenant_id,
                store_id: &self.store_id,
                event_id: &event_id,
                source_agent_id,
                agent_key_id,
                entity_type: &event.entity_type,
                entity_id: &event.entity_id,
                event_type: &event.event_type,
                created_at,
                payload_kind: 0,
                payload_plain_hash: &payload_plain_hash,
                payload_cipher_hash: &ZERO_HASH,
            },
        );
        match signing_hash {
            Ok(hash)
                if stateset_crypto::sign::verify_event_signature(
                    &hash,
                    &signature,
                    &key.public_key,
                ) =>
            {
                Ok(())
            }
            Ok(_) => reject("signature does not verify"),
            Err(error) => reject(&format!("cannot compute signing hash: {error}")),
        }
    }

    fn map_pulled_event(&self, event: PulledEvent) -> Result<SyncEvent, SyncError> {
        let sequence = event.envelope.sequence_number.ok_or_else(|| {
            SyncError::Transport("sequencer event was missing sequence_number".into())
        })?;
        if sequence == 0 {
            return Err(SyncError::Transport("sequencer event had sequence_number=0".into()));
        }

        let timestamp_raw =
            event.envelope.created_at.as_deref().or(event.sequenced_at.as_deref()).ok_or_else(
                || {
                    SyncError::Transport(
                        "sequencer event was missing created_at and sequenced_at".into(),
                    )
                },
            )?;
        let timestamp_raw = timestamp_raw.to_string();
        let timestamp = DateTime::parse_from_rfc3339(&timestamp_raw)
            .map(|parsed| parsed.with_timezone(&Utc))
            .map_err(|error| {
                SyncError::Transport(format!(
                    "sequencer event had an invalid timestamp `{timestamp_raw}`: {error}"
                ))
            })?;

        let mut mapped = SyncEvent::with_id(
            event.envelope.event_id,
            sequence,
            event.envelope.event_type,
            event.envelope.entity_type,
            event.envelope.entity_id,
            event.envelope.payload,
            timestamp,
        )
        .with_remote_sequence(sequence);

        mapped.signature = event.envelope.agent_signature;
        mapped.agent_signature_scheme = event.envelope.agent_signature_scheme;
        mapped.agent_signature_bundle = event.envelope.agent_signature_bundle;
        mapped.command_id = event.envelope.command_id;
        mapped.base_version = event.envelope.base_version;
        mapped.source_agent_id = event.envelope.source_agent_id;
        mapped.agent_key_id = event.envelope.agent_key_id;
        mapped.kernel = event.envelope.kernel.filter(|kernel| !kernel.is_empty());
        // `SyncEvent::with_id` hashed the payload we actually received. A
        // sequencer-supplied hash is only a claim: it must match.
        if let Some(claimed) = event.envelope.payload_plain_hash.or(event.envelope.payload_hash)
            && !strip_hex_prefix(&claimed).eq_ignore_ascii_case(&mapped.hash)
        {
            return Err(SyncError::Transport(format!(
                "sequencer event {} payload hash {claimed} does not match its payload ({})",
                mapped.id, mapped.hash
            )));
        }
        if !self.trusted_agent_keys.is_empty() {
            self.verify_pulled_signature(&mapped, &timestamp_raw)?;
        }

        Ok(mapped)
    }
}

#[async_trait]
impl Transport for SequencerHttpTransport {
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        if events.is_empty() {
            return Ok(PushResult::accepted_only(0, 0));
        }

        let body = IngestRequest {
            agent_id: self.agent_id.clone(),
            events: events
                .iter()
                .map(|event| self.build_envelope(event))
                .collect::<Result<Vec<_>, _>>()?,
        };

        let request = self
            .authorized(
                self.client.request(Method::POST, self.endpoint("/api/v1/ves/events/ingest")?),
            )
            .json(&body);
        let response = self.send(request).await?;
        let response: IngestResponse = response.json().await.map_err(|error| {
            SyncError::Transport(format!("failed to decode sequencer ingest response: {error}"))
        })?;

        let rejections = response.push_rejections()?;
        let mut acknowledgements = response
            .receipts
            .iter()
            .filter_map(ReceiptInfo::to_acknowledgement)
            .collect::<Vec<_>>();
        if acknowledgements.is_empty() {
            acknowledgements = self.synthesise_acknowledgements(events, &response)?;
        }

        let mut result =
            PushResult::accepted_only(response.events_accepted, response.remote_head());
        if !acknowledgements.is_empty() {
            result = result.with_acknowledgements(acknowledgements);
        }
        if !rejections.is_empty() {
            result = result.with_rejections(rejections);
        }
        Ok(result)
    }

    async fn fetch_head(&self) -> Result<RemoteHead, SyncError> {
        self.fetch_head_state().await
    }

    async fn pull_events(&self, since: u64, limit: usize) -> Result<PullResult, SyncError> {
        Ok(self.fetch_pull_page(since, limit).await?.result)
    }

    async fn pull_events_page(&self, since: u64, limit: usize) -> Result<PullPage, SyncError> {
        self.fetch_pull_page(since, limit).await
    }
}

fn normalize_base_url(raw: &str) -> Result<String, SyncError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SyncError::InvalidConfig("sequencer base URL must not be empty".into()));
    }

    let normalized = if let Some(rest) = trimmed.strip_prefix("grpc://") {
        format!("http://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("grpcs://") {
        format!("https://{rest}")
    } else {
        trimmed.to_string()
    };

    let mut parsed = Url::parse(&normalized).map_err(|error| {
        SyncError::InvalidConfig(format!("invalid sequencer URL `{trimmed}`: {error}"))
    })?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(SyncError::InvalidConfig(format!(
                "unsupported sequencer URL scheme `{other}`"
            )));
        }
    }
    if parsed.host_str().is_none() {
        return Err(SyncError::InvalidConfig("sequencer URL must include a host".into()));
    }

    let path = parsed.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        parsed.set_path("");
    } else {
        parsed.set_path(&path);
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestRequest {
    agent_id: String,
    events: Vec<VesEventEnvelope>,
}

#[derive(Debug, Serialize)]
struct VesEventEnvelope {
    event_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_id: Option<String>,
    tenant_id: String,
    store_id: String,
    entity_type: String,
    entity_id: String,
    event_type: String,
    ves_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
    payload_kind: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_encrypted: Option<Value>,
    payload_plain_hash: String,
    payload_cipher_hash: String,
    agent_key_id: u32,
    agent_signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_signature_scheme: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_signature_bundle: Option<Value>,
    source_agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernel: Option<KernelMetadata>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IngestResponse {
    #[serde(default, alias = "batch_id")]
    batch_id: Option<String>,
    #[serde(default, alias = "events_accepted")]
    events_accepted: usize,
    #[serde(default, alias = "events_rejected")]
    events_rejected: usize,
    #[serde(default, alias = "sequence_start")]
    sequence_start: Option<u64>,
    #[serde(default, alias = "sequence_end")]
    sequence_end: Option<u64>,
    #[serde(default, alias = "head_sequence")]
    head_sequence: Option<u64>,
    #[serde(default)]
    rejections: Vec<RejectionInfo>,
    #[serde(default)]
    receipts: Vec<ReceiptInfo>,
    #[serde(default)]
    receipt: Option<BatchReceipt>,
}

impl IngestResponse {
    fn remote_head(&self) -> u64 {
        self.head_sequence.or(self.sequence_end).unwrap_or(0)
    }

    fn receipt_handle(&self) -> Option<String> {
        self.receipt
            .as_ref()
            .and_then(BatchReceipt::receipt_handle)
            .or_else(|| self.batch_id.clone())
    }

    fn push_rejections(&self) -> Result<Vec<PushRejection>, SyncError> {
        let mut rejections = Vec::with_capacity(self.rejections.len());
        for rejection in &self.rejections {
            rejections.push(rejection.to_push_rejection()?);
        }

        if self.events_rejected > 0 && rejections.len() < self.events_rejected {
            return Err(SyncError::Transport(format!(
                "sequencer reported {} rejected events without enough per-event ids",
                self.events_rejected
            )));
        }

        Ok(rejections)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RejectionInfo {
    #[serde(default, alias = "event_id")]
    event_id: Option<String>,
    #[serde(default, alias = "code", alias = "error_code")]
    code: Option<String>,
    #[serde(default, alias = "reason", alias = "message", alias = "error")]
    reason: Option<String>,
    #[serde(default, alias = "retryable")]
    retryable: Option<bool>,
}

impl RejectionInfo {
    fn to_push_rejection(&self) -> Result<PushRejection, SyncError> {
        let event_id = self.event_id.as_deref().ok_or_else(|| {
            SyncError::Transport("sequencer rejection was missing event_id".into())
        })?;
        let event_id = Uuid::parse_str(event_id).map_err(|error| {
            SyncError::Transport(format!(
                "sequencer rejection contained an invalid event id `{event_id}`: {error}"
            ))
        })?;

        let mut rejection = PushRejection::new(event_id);
        if let Some(code) = self.code.as_deref() {
            rejection = rejection.with_code(code);
        }
        if let Some(reason) = self.reason.as_deref() {
            rejection = rejection.with_reason(reason);
        }
        if let Some(retryable) = self.retryable {
            rejection = rejection.with_retryable(retryable);
        }
        Ok(rejection)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptInfo {
    #[serde(default, alias = "event_id")]
    event_id: Option<Uuid>,
    #[serde(
        default,
        alias = "sequence_number",
        alias = "remoteSequence",
        alias = "remote_sequence"
    )]
    sequence_number: Option<u64>,
    #[serde(default, alias = "receipt_hash")]
    receipt_hash: Option<String>,
    #[serde(default, alias = "batch_id")]
    batch_id: Option<String>,
}

impl ReceiptInfo {
    fn receipt_handle(&self) -> Option<String> {
        self.receipt_hash.clone().or_else(|| self.batch_id.clone())
    }

    fn to_acknowledgement(&self) -> Option<PushAcknowledgement> {
        let event_id = self.event_id?;
        let remote_sequence = self.sequence_number?;
        if remote_sequence == 0 {
            return None;
        }

        let mut acknowledgement = PushAcknowledgement::new(event_id, remote_sequence);
        if let Some(receipt_handle) = self.receipt_handle() {
            acknowledgement = acknowledgement.with_receipt(receipt_handle);
        }
        Some(acknowledgement)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchReceipt {
    #[serde(default, alias = "batch_id")]
    batch_id: Option<String>,
    #[serde(default, alias = "receipt_hash")]
    receipt_hash: Option<String>,
}

impl BatchReceipt {
    fn receipt_handle(&self) -> Option<String> {
        self.receipt_hash.clone().or_else(|| self.batch_id.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeadResponse {
    #[serde(default, alias = "head_sequence")]
    head_sequence: Option<u64>,
    #[serde(default, alias = "state_root")]
    state_root: Option<String>,
    #[serde(default, alias = "latest_commitment")]
    latest_commitment: Option<LatestCommitment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatestCommitment {
    #[serde(default, alias = "batch_id")]
    batch_id: Option<String>,
    #[serde(
        default,
        alias = "previous_batch_id",
        alias = "previous_commitment_id",
        alias = "previousBatchId"
    )]
    previous_batch_id: Option<String>,
    #[serde(default, alias = "state_root", alias = "root")]
    state_root: Option<String>,
    #[serde(
        default,
        alias = "head_sequence",
        alias = "remote_head",
        alias = "sequence_end",
        alias = "remoteHead"
    )]
    remote_head: Option<u64>,
    #[serde(default, alias = "signerId", alias = "signer_id")]
    signer_id: Option<String>,
    #[serde(default, alias = "signatureScheme", alias = "signature_scheme")]
    signature_scheme: Option<String>,
    #[serde(
        default,
        alias = "signerPublicKey",
        alias = "signer_public_key",
        alias = "public_key",
        alias = "publicKey"
    )]
    signer_public_key: Option<String>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default, alias = "issuedAt", alias = "issued_at")]
    issued_at: Option<String>,
}

impl LatestCommitment {
    fn commitment_id(&self) -> Option<String> {
        self.batch_id.clone()
    }

    fn to_manifest(
        &self,
        fallback_state_root: Option<&str>,
        fallback_remote_head: Option<u64>,
    ) -> Result<Option<CommitmentManifest>, SyncError> {
        let Some(commitment_id) = self.commitment_id() else {
            return Ok(None);
        };

        let provided_signature_fields = [
            self.signer_id.as_ref(),
            self.signer_public_key.as_ref(),
            self.signature.as_ref(),
            self.issued_at.as_ref(),
        ]
        .into_iter()
        .filter(|field| field.is_some())
        .count();
        if provided_signature_fields == 0 {
            return Ok(None);
        }

        let state_root = self.state_root.clone().or_else(|| fallback_state_root.map(str::to_owned)).ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing state_root required for signed manifest"
            ))
        })?;
        let remote_head = self.remote_head.or(fallback_remote_head).ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing remote_head required for signed manifest"
            ))
        })?;
        let signer_id = self.signer_id.clone().ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing signer_id required for signed manifest"
            ))
        })?;
        let signer_public_key = self.signer_public_key.clone().ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing signer_public_key required for signed manifest"
            ))
        })?;
        let signature = self.signature.clone().ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing signature required for signed manifest"
            ))
        })?;
        let issued_at_raw = self.issued_at.as_deref().ok_or_else(|| {
            SyncError::Transport(format!(
                "latest commitment `{commitment_id}` is missing issued_at required for signed manifest"
            ))
        })?;
        let issued_at = DateTime::parse_from_rfc3339(issued_at_raw)
            .map(|parsed| parsed.with_timezone(&Utc))
            .map_err(|error| {
                SyncError::Transport(format!(
                    "latest commitment `{commitment_id}` had invalid issued_at `{issued_at_raw}`: {error}"
                ))
            })?;

        Ok(Some(CommitmentManifest {
            commitment_id,
            previous_commitment_id: self.previous_batch_id.clone(),
            state_root,
            remote_head,
            signer_id,
            signature_scheme: self
                .signature_scheme
                .clone()
                .unwrap_or_else(|| "ed25519".to_string()),
            signer_public_key: Some(signer_public_key),
            signature: Some(signature),
            issued_at,
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullResponse {
    #[serde(default)]
    events: Vec<PulledEvent>,
    #[serde(default, alias = "head_sequence")]
    head_sequence: Option<u64>,
    #[serde(default, alias = "has_more")]
    has_more: bool,
    #[serde(default, alias = "next_sequence")]
    next_sequence: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PulledEvent {
    envelope: PulledEnvelope,
    #[serde(default, alias = "sequencedAt")]
    sequenced_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PulledEnvelope {
    event_id: Uuid,
    entity_type: String,
    entity_id: String,
    event_type: String,
    #[serde(default, alias = "commandId", alias = "command_id")]
    command_id: Option<String>,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    payload_plain_hash: Option<String>,
    #[serde(default)]
    payload_hash: Option<String>,
    #[serde(default)]
    agent_signature: Option<String>,
    #[serde(default, alias = "agentSignatureScheme", alias = "agent_signature_scheme")]
    agent_signature_scheme: Option<i32>,
    #[serde(default, alias = "agentSignatureBundle", alias = "agent_signature_bundle")]
    agent_signature_bundle: Option<Value>,
    #[serde(default, alias = "sourceAgentId", alias = "source_agent_id")]
    source_agent_id: Option<String>,
    #[serde(default, alias = "agentKeyId", alias = "agent_key_id")]
    agent_key_id: Option<u32>,
    #[serde(default, alias = "baseVersion", alias = "base_version")]
    base_version: Option<u64>,
    #[serde(default)]
    kernel: Option<KernelMetadata>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    sequence_number: Option<u64>,
}

fn strip_hex_prefix(value: &str) -> &str {
    value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use serde_json::json;
    use stateset_crypto::sign::generate_keypair;

    use super::*;
    use crate::commitment::{CommitmentManifest, sign_commitment_manifest};
    use crate::event::{BudgetCheckpoint, PolicyCheckpoint, PolicyDecision, SequenceAuthority};

    #[derive(Debug)]
    struct CapturedRequest {
        request_line: String,
        headers: String,
        body: String,
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    fn content_length(headers: &str) -> usize {
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    return value.trim().parse::<usize>().ok();
                }
                None
            })
            .unwrap_or(0)
    }

    fn spawn_single_response_server(
        status: &str,
        response_body: serde_json::Value,
    ) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        let response_body = serde_json::to_string(&response_body).unwrap();
        let status = status.to_string();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = Vec::new();
            let header_end = loop {
                let mut chunk = [0_u8; 1024];
                let bytes_read = stream.read(&mut chunk).unwrap();
                if bytes_read == 0 {
                    break buffer.len();
                }
                buffer.extend_from_slice(&chunk[..bytes_read]);
                if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
                    break position + 4;
                }
            };

            let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
            let expected_body_bytes = content_length(&header_text);
            while buffer.len() < header_end + expected_body_bytes {
                let mut chunk = [0_u8; 1024];
                let bytes_read = stream.read(&mut chunk).unwrap();
                if bytes_read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..bytes_read]);
            }

            let body = String::from_utf8_lossy(
                &buffer[header_end..buffer.len().min(header_end + expected_body_bytes)],
            )
            .to_string();
            let request_line = header_text.lines().next().unwrap_or_default().to_string();
            tx.send(CapturedRequest { request_line, headers: header_text, body }).unwrap();

            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        (format!("http://{address}"), rx)
    }

    fn signed_event(label: &str) -> SyncEvent {
        SyncEvent::new(
            format!("order.{label}"),
            "order",
            format!("ORD-{label}"),
            json!({ "label": label }),
        )
        .with_signature(format!("sig-{label}"))
    }

    #[test]
    fn normalize_base_url_supports_grpc_schemes() {
        assert_eq!(
            normalize_base_url("grpc://sequencer.stateset.local:50051").unwrap(),
            "http://sequencer.stateset.local:50051"
        );
        assert_eq!(
            normalize_base_url("grpcs://sequencer.stateset.local").unwrap(),
            "https://sequencer.stateset.local"
        );
    }

    #[test]
    fn transport_accessors_reflect_builder_configuration() {
        let config = SyncConfig::new("agent-1", "tenant-1", "store-1");
        let transport =
            SequencerHttpTransport::from_config("https://sequencer.stateset.com/", &config)
                .unwrap()
                .with_agent_key_id(7)
                .with_api_key("ss_example_key")
                .with_bearer_token("bearer-token");

        assert_eq!(transport.base_url(), "https://sequencer.stateset.com");
        assert_eq!(transport.agent_id(), "agent-1");
        assert_eq!(transport.tenant_id(), "tenant-1");
        assert_eq!(transport.store_id(), "store-1");
        assert_eq!(transport.agent_key_id(), 7);
        assert!(transport.has_api_key());
        assert!(transport.has_bearer_token());
    }

    #[tokio::test]
    async fn push_events_posts_ves_envelopes_and_synthesizes_acknowledgements() {
        let (base_url, requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "batchId": "B-1",
                "eventsAccepted": 2,
                "eventsRejected": 0,
                "sequenceStart": 40,
                "sequenceEnd": 41,
                "headSequence": 41,
                "receipt": {
                    "batchId": "B-1",
                    "receiptHash": "batch-rcpt"
                }
            }),
        );

        let event_a = signed_event("created")
            .with_agent_signature_scheme(3)
            .with_agent_signature_bundle(json!({"ml_dsa_65_signature": "mlsig-created"}))
            .with_command_id("cmd-1")
            .with_base_version(3)
            .with_source_agent_id("agent-created")
            .with_policy_checkpoint(
                PolicyCheckpoint::new("orders", PolicyDecision::Allowed)
                    .with_reason("within threshold"),
            )
            .with_budget_checkpoint(
                BudgetCheckpoint::new("budget-1", 9900, "USD").with_remaining_amount_minor(100),
            )
            .with_agent_key_id(99);
        let event_b = signed_event("confirmed");
        let transport = SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1")
            .unwrap()
            .with_api_key("ss_test_key")
            .with_agent_key_id(7);

        let result = transport.push_events(&[event_a.clone(), event_b.clone()]).await.unwrap();
        assert_eq!(result.accepted, 2);
        assert_eq!(result.remote_head, 41);
        assert_eq!(result.acknowledgements.len(), 2);
        assert_eq!(result.acknowledgements[0].event_id, event_a.id);
        assert_eq!(result.acknowledgements[0].remote_sequence, 40);
        assert_eq!(result.acknowledgements[0].receipt.as_deref(), Some("batch-rcpt"));
        assert_eq!(result.acknowledgements[1].event_id, event_b.id);
        assert_eq!(result.acknowledgements[1].remote_sequence, 41);

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.starts_with("POST /api/v1/ves/events/ingest "));
        assert!(captured.headers.to_ascii_lowercase().contains("x-api-key: ss_test_key"));

        let body: serde_json::Value = serde_json::from_str(&captured.body).unwrap();
        assert_eq!(body["agentId"], json!("agent-1"));
        assert_eq!(body["events"][0]["event_id"], json!(event_a.id));
        assert_eq!(body["events"][0]["command_id"], json!("cmd-1"));
        assert_eq!(body["events"][0]["base_version"], json!(3));
        assert_eq!(body["events"][0]["agent_key_id"], json!(99));
        assert_eq!(body["events"][0]["agent_signature"], json!("sig-created"));
        assert_eq!(body["events"][0]["agent_signature_scheme"], json!(3));
        assert_eq!(
            body["events"][0]["agent_signature_bundle"],
            json!({"ml_dsa_65_signature": "mlsig-created"})
        );
        assert_eq!(body["events"][0]["source_agent_id"], json!("agent-created"));
        assert_eq!(body["events"][0]["kernel"]["policy"]["domain"], json!("orders"));
        assert_eq!(body["events"][0]["kernel"]["policy"]["decision"], json!("allowed"));
        assert_eq!(body["events"][0]["kernel"]["budget"]["remaining_amount_minor"], json!(100));
        assert_eq!(body["events"][0]["payload_cipher_hash"], json!("0".repeat(64)));
        assert_eq!(body["events"][1]["agent_signature"], json!("sig-confirmed"));
        assert_eq!(body["events"][1]["agent_key_id"], json!(7));
        assert_eq!(body["events"][1]["source_agent_id"], json!("agent-1"));
    }

    #[tokio::test]
    async fn push_events_preserves_explicit_rejections() {
        let rejected_event = signed_event("rejected");
        let accepted_event = signed_event("accepted");
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "batchId": "B-2",
                "eventsAccepted": 1,
                "eventsRejected": 1,
                "sequenceStart": 100,
                "sequenceEnd": 100,
                "headSequence": 100,
                "rejections": [
                    {
                        "eventId": rejected_event.id,
                        "code": "invalid_signature",
                        "reason": "signature verification failed",
                        "retryable": false
                    }
                ]
            }),
        );

        let transport =
            SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1").unwrap();

        let result =
            transport.push_events(&[accepted_event.clone(), rejected_event.clone()]).await.unwrap();
        assert_eq!(result.accepted, 1);
        assert_eq!(result.acknowledgements.len(), 1);
        assert_eq!(result.acknowledgements[0].event_id, accepted_event.id);
        assert_eq!(result.acknowledgements[0].remote_sequence, 100);
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(result.rejections[0].event_id, rejected_event.id);
        assert_eq!(result.rejections[0].code.as_deref(), Some("invalid_signature"));
        assert_eq!(result.rejections[0].retryable, Some(false));
    }

    #[tokio::test]
    async fn push_events_rejects_ambiguous_rejection_payloads() {
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "eventsAccepted": 0,
                "eventsRejected": 1,
                "rejections": [
                    {
                        "reason": "signature verification failed"
                    }
                ]
            }),
        );

        let transport =
            SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1").unwrap();

        let error = transport.push_events(&[signed_event("bad")]).await.unwrap_err();
        assert!(matches!(error, SyncError::Transport(_)));
        assert!(error.to_string().contains("missing event_id"));
    }

    #[tokio::test]
    async fn fetch_head_uses_head_endpoint_and_maps_metadata() {
        let (base_url, requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "head_sequence": 42,
                "state_root": "root-42",
                "latest_commitment": {
                    "batch_id": "BATCH-42"
                }
            }),
        );

        let transport = SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1")
            .unwrap()
            .with_api_key("ss_test_key");

        let head = transport.fetch_head().await.unwrap();
        assert_eq!(head.remote_head, 42);
        assert_eq!(head.state_root.as_deref(), Some("root-42"));
        assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-42"));

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.contains("GET /api/v1/head?"));
        assert!(captured.request_line.contains("tenant_id=tenant-1"));
        assert!(captured.request_line.contains("store_id=store-1"));
        assert!(captured.headers.to_ascii_lowercase().contains("x-api-key: ss_test_key"));
    }

    #[tokio::test]
    async fn fetch_head_maps_signed_commitment_manifest() {
        let (private_key, public_key) = generate_keypair();
        let state_root = "43".repeat(32);
        let manifest = sign_commitment_manifest(
            CommitmentManifest::new("BATCH-43", state_root.clone(), 43, "sequencer-a"),
            &private_key,
            &public_key,
        )
        .unwrap();
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "head_sequence": 43,
                "state_root": state_root,
                "latest_commitment": {
                    "batch_id": manifest.commitment_id,
                    "state_root": manifest.state_root,
                    "remote_head": manifest.remote_head,
                    "signer_id": manifest.signer_id,
                    "signature_scheme": manifest.signature_scheme,
                    "signer_public_key": manifest.signer_public_key,
                    "signature": manifest.signature,
                    "issued_at": manifest.issued_at.to_rfc3339(),
                }
            }),
        );

        let transport =
            SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1").unwrap();
        let head = transport.fetch_head().await.unwrap();

        assert_eq!(head.remote_head, 43);
        assert_eq!(head.last_commitment_id.as_deref(), Some("BATCH-43"));
        assert_eq!(
            head.commitment_manifest.as_ref().map(|manifest| manifest.signer_id.as_str()),
            Some("sequencer-a")
        );
    }

    #[tokio::test]
    async fn pull_events_maps_canonical_remote_sequences() {
        let event_id = Uuid::new_v4();
        let (base_url, requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-9",
                            "event_type": "order.shipped",
                            "command_id": "cmd-7",
                            "payload": { "status": "shipped" },
                            "payload_plain_hash": SyncEvent::compute_hash(&json!({ "status": "shipped" })),
                            "agent_signature": "deadbeef",
                            "agent_signature_scheme": 3,
                            "agent_signature_bundle": { "ml_dsa_65_signature": "cafebabe" },
                            "source_agent_id": "agent-remote",
                            "agent_key_id": 21,
                            "base_version": 5,
                            "kernel": {
                                "policy": {
                                    "domain": "orders",
                                    "decision": "allowed",
                                    "reason": "approved upstream"
                                },
                                "budget": {
                                    "budget_id": "budget-remote",
                                    "reserved_amount_minor": 4200,
                                    "currency": "USD",
                                    "remaining_amount_minor": 800
                                }
                            },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 7
                        },
                        "sequenced_at": "2024-03-01T00:00:02Z"
                    }
                ],
                "head_sequence": 9,
                "has_more": true
            }),
        );

        let transport = SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1")
            .unwrap()
            .with_bearer_token("jwt-token");

        let result = transport.pull_events(6, 50).await.unwrap();
        assert_eq!(result.remote_head, 9);
        assert!(result.has_more);
        assert_eq!(result.events.len(), 1);

        let event = &result.events[0];
        assert_eq!(event.id, event_id);
        assert_eq!(event.sequence, 7);
        assert_eq!(event.sequence_authority, SequenceAuthority::CanonicalRemote);
        assert_eq!(event.canonical_sequence(), Some(7));
        assert_eq!(event.signature.as_deref(), Some("deadbeef"));
        assert_eq!(event.agent_signature_scheme, Some(3));
        assert_eq!(event.agent_signature_bundle, Some(json!({"ml_dsa_65_signature": "cafebabe"})));
        assert_eq!(event.command_id.as_deref(), Some("cmd-7"));
        assert_eq!(event.base_version, Some(5));
        assert_eq!(event.source_agent_id.as_deref(), Some("agent-remote"));
        assert_eq!(event.agent_key_id, Some(21));
        assert_eq!(
            event
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.policy.as_ref())
                .map(|policy| (policy.domain.as_str(), policy.reason.as_deref())),
            Some(("orders", Some("approved upstream")))
        );
        assert_eq!(
            event.kernel.as_ref().and_then(|kernel| kernel.budget.as_ref()).map(|budget| (
                budget.budget_id.as_str(),
                budget.reserved_amount_minor,
                budget.remaining_amount_minor
            )),
            Some(("budget-remote", 4200, Some(800)))
        );
        assert_eq!(event.hash, SyncEvent::compute_hash(&json!({ "status": "shipped" })));

        let captured = requests.recv().unwrap();
        assert!(captured.request_line.contains("GET /api/v1/events?"));
        assert!(captured.request_line.contains("tenant_id=tenant-1"));
        assert!(captured.request_line.contains("store_id=store-1"));
        assert!(captured.request_line.contains("from=7"));
        assert!(captured.request_line.contains("limit=50"));
        assert!(captured.headers.to_ascii_lowercase().contains("authorization: bearer jwt-token"));
    }

    fn pulled_page(envelope: serde_json::Value) -> serde_json::Value {
        json!({ "events": [{ "envelope": envelope }], "head_sequence": 1, "has_more": false })
    }

    #[tokio::test]
    async fn pull_rejects_an_event_whose_claimed_hash_does_not_match_its_payload() {
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            pulled_page(json!({
                "event_id": Uuid::new_v4(),
                "entity_type": "order",
                "entity_id": "ORD-1",
                "event_type": "order.updated",
                "payload": { "total": "999.00" },
                "payload_plain_hash": SyncEvent::compute_hash(&json!({ "total": "1.00" })),
                "created_at": "2024-03-01T00:00:00Z",
                "sequence_number": 1
            })),
        );
        let transport =
            SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1").unwrap();
        let error = transport.pull_events(0, 10).await.unwrap_err();
        assert!(error.to_string().contains("does not match its payload"), "{error}");
    }

    /// A pulled envelope signed by `private_key` over this tenant/store.
    fn signed_envelope(
        private_key: &[u8; 32],
        tenant_id: &str,
        store_id: &str,
        source_agent_id: &str,
        signed_payload: &serde_json::Value,
        delivered_payload: &serde_json::Value,
    ) -> serde_json::Value {
        let event_id = Uuid::new_v4();
        let created_at = "2024-03-01T00:00:00Z";
        let plain_hash = SyncEvent::compute_hash(signed_payload);
        let plain_bytes: [u8; 32] = hex::decode(&plain_hash).unwrap().try_into().unwrap();
        let event_id_text = event_id.to_string();
        let signing_hash = stateset_crypto::hash::compute_event_signing_hash(
            &stateset_crypto::hash::EventSigningParams {
                ves_version: DEFAULT_VES_VERSION,
                tenant_id,
                store_id,
                event_id: &event_id_text,
                source_agent_id,
                agent_key_id: 3,
                entity_type: "order",
                entity_id: "ORD-1",
                event_type: "order.updated",
                created_at,
                payload_kind: 0,
                payload_plain_hash: &plain_bytes,
                payload_cipher_hash: &ZERO_HASH,
            },
        )
        .unwrap();
        let signature = stateset_crypto::sign::sign_event_hash(&signing_hash, private_key).unwrap();
        json!({
            "event_id": event_id,
            "entity_type": "order",
            "entity_id": "ORD-1",
            "event_type": "order.updated",
            "payload": delivered_payload,
            "agent_signature": format!("0x{}", hex::encode(signature)),
            "source_agent_id": source_agent_id,
            "agent_key_id": 3,
            "created_at": created_at,
            "sequence_number": 1
        })
    }

    #[tokio::test]
    async fn pinned_agent_keys_verify_pulled_signatures_and_refuse_forgeries() {
        let tenant = Uuid::new_v4().to_string();
        let store = Uuid::new_v4().to_string();
        let agent = Uuid::new_v4().to_string();
        let (private_key, public_key) = stateset_crypto::sign::generate_keypair();
        let payload = json!({ "status": "shipped" });
        let transport_for = |base_url: String| {
            SequencerHttpTransport::new(base_url, "agent-local", &tenant, &store)
                .unwrap()
                .with_trusted_agent_key(&agent, 3, public_key)
        };

        // Genuine event: accepted.
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            pulled_page(signed_envelope(&private_key, &tenant, &store, &agent, &payload, &payload)),
        );
        let pulled = transport_for(base_url).pull_events(0, 10).await.unwrap();
        assert_eq!(pulled.events.len(), 1);

        // Payload rewritten after signing (hash recomputed, signature stale).
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            pulled_page(signed_envelope(
                &private_key,
                &tenant,
                &store,
                &agent,
                &payload,
                &json!({ "status": "refunded" }),
            )),
        );
        let error = transport_for(base_url).pull_events(0, 10).await.unwrap_err();
        assert!(error.to_string().contains("does not verify"), "{error}");

        // Signed for another tenant: the signing hash binds tenant and store.
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            pulled_page(signed_envelope(
                &private_key,
                &Uuid::new_v4().to_string(),
                &store,
                &agent,
                &payload,
                &payload,
            )),
        );
        let error = transport_for(base_url).pull_events(0, 10).await.unwrap_err();
        assert!(error.to_string().contains("does not verify"), "{error}");

        // An agent whose key is not pinned.
        let stranger = Uuid::new_v4().to_string();
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            pulled_page(signed_envelope(
                &private_key,
                &tenant,
                &store,
                &stranger,
                &payload,
                &payload,
            )),
        );
        let error = transport_for(base_url).pull_events(0, 10).await.unwrap_err();
        assert!(error.to_string().contains("is not pinned"), "{error}");
    }

    #[tokio::test]
    async fn pull_events_page_uses_server_next_sequence_as_continuation_cursor() {
        let event_id = Uuid::new_v4();
        let (base_url, _requests) = spawn_single_response_server(
            "200 OK",
            json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-10",
                            "event_type": "order.paid",
                            "payload": { "status": "paid" },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 10
                        },
                        "sequenced_at": "2024-03-01T00:00:03Z"
                    }
                ],
                "head_sequence": 12,
                "has_more": true,
                "next_sequence": 11
            }),
        );

        let transport =
            SequencerHttpTransport::new(base_url, "agent-1", "tenant-1", "store-1").unwrap();

        let page = transport.pull_events_page(9, 50).await.unwrap();
        assert_eq!(page.result.remote_head, 12);
        assert!(page.result.has_more);
        assert_eq!(page.observed_cursor, Some(10));
        assert_eq!(page.next_cursor, Some(11));
    }

    #[tokio::test]
    async fn push_rejects_unsigned_events() {
        let transport = SequencerHttpTransport::new(
            "https://sequencer.stateset.local",
            "agent-1",
            "tenant-1",
            "store-1",
        )
        .unwrap();
        let event = SyncEvent::new("order.created", "order", "ORD-1", json!({}));

        let error = transport.push_events(&[event]).await.unwrap_err();
        assert!(matches!(error, SyncError::InvalidEvent(_)));
    }
}
