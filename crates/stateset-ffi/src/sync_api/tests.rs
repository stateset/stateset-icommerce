//! Unit tests for the sync runtime C API.

use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use async_trait::async_trait;
use serde_json::{Value, json};
use stateset_sdk::sync::{PullResult, PushRejection, PushResult, RemoteHead, Transport};
use tempfile::NamedTempFile;

use super::*;

#[derive(Debug)]
struct CapturedRequest {
    request_line: String,
    body: String,
}

#[derive(Debug)]
struct StubResponse {
    status: String,
    body: Value,
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

fn spawn_response_server(
    responses: Vec<StubResponse>,
) -> (String, mpsc::Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        for response in responses {
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
            tx.send(CapturedRequest { request_line, body }).unwrap();

            let response_body = serde_json::to_string(&response.body).unwrap();
            let payload = format!(
                "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response.status,
                response_body.len(),
                response_body
            );
            stream.write_all(payload.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
    });

    (format!("http://{address}"), rx)
}

fn runtime_config_json() -> String {
    runtime_config_json_for("https://sequencer.stateset.com")
}

fn runtime_config_json_for(base_url: &str) -> String {
    serde_json::to_string(&SyncRuntimeConfig::new(
        base_url,
        // The stub sequencer returns head metadata without a signed
        // manifest; opt out of the fail-closed default so these transport
        // round-trip tests can accept it.
        stateset_sdk::sync::SyncConfig::new("agent-ffi", "tenant-ffi", "store-ffi")
            .with_unauthenticated_remote_head_allowed(),
    ))
    .unwrap()
}

fn signed_event_json(label: &str) -> String {
    serde_json::to_string(
        &SyncEvent::new(
            format!("order.{label}"),
            "order",
            format!("ORD-FFI-{label}"),
            json!({ "label": label }),
        )
        .with_signature(format!("sig-{label}"))
        .with_command_id(format!("cmd-{label}"))
        .with_source_agent_id("agent-ffi")
        .with_agent_key_id(7),
    )
    .unwrap()
}

#[derive(Debug, Clone, Default)]
struct RejectingTransport;

#[async_trait]
impl Transport for RejectingTransport {
    async fn push_events(
        &self,
        events: &[SyncEvent],
    ) -> Result<PushResult, stateset_sdk::sync::SyncError> {
        let rejections = events
            .iter()
            .map(|event| {
                PushRejection::new(event.id)
                    .with_code("invalid_event")
                    .with_reason("event rejected")
                    .with_retryable(false)
            })
            .collect();
        Ok(PushResult::accepted_only(0, 0).with_rejections(rejections))
    }

    async fn pull_events(
        &self,
        _since: u64,
        _limit: usize,
    ) -> Result<PullResult, stateset_sdk::sync::SyncError> {
        Ok(PullResult { events: Vec::new(), remote_head: 0, has_more: false })
    }

    async fn fetch_head(&self) -> Result<RemoteHead, stateset_sdk::sync::SyncError> {
        Ok(RemoteHead::new(0))
    }
}

#[allow(clippy::await_holding_lock)]
async fn seed_dead_letter(handle: SyncRuntimeHandle) -> Uuid {
    let lease = begin_sync_runtime_use(handle).unwrap();
    let runtime = lease.runtime();
    let mut runtime = lock_sync_runtime(runtime);
    let event =
        SyncEvent::new("payment.failed", "payment", "PAY-FFI-1", json!({"status": "failed"}));
    let event_id = event.id;
    runtime.record(event).unwrap();
    runtime.engine_mut().push(&RejectingTransport).await.unwrap();
    event_id
}

#[test]
fn sync_runtime_init_from_json_and_destroy() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let result = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(result.code, FfiErrorCode::Ok);
    assert!(!result.value.is_null());

    unsafe { stateset_sync_runtime_destroy(result.value) };
}

#[test]
fn sync_runtime_init_from_file_via_c_api() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), runtime_config_json()).unwrap();
    let path = CString::new(file.path().display().to_string()).unwrap();

    let result = unsafe { stateset_sync_runtime_init_from_file(path.as_ptr()) };
    assert_eq!(result.code, FfiErrorCode::Ok);
    assert!(!result.value.is_null());

    unsafe { stateset_sync_runtime_destroy(result.value) };
}

#[test]
fn sync_runtime_record_and_snapshot_json_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_type = CString::new("order.created").unwrap();
    let entity_type = CString::new("order").unwrap();
    let entity_id = CString::new("ORD-FFI-1").unwrap();
    let payload = CString::new(json!({"total": 99, "currency": "USD"}).to_string()).unwrap();

    let record = unsafe {
        stateset_sync_runtime_record_json(
            init.value,
            event_type.as_ptr(),
            entity_type.as_ptr(),
            entity_id.as_ptr(),
            payload.as_ptr(),
        )
    };
    assert_eq!(record.code, FfiErrorCode::Ok);
    assert_eq!(record.value, 1);

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };

    let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
    assert_eq!(snapshot["status"]["pending"], 1);
    assert_eq!(snapshot["status"]["local_head"], 1);
    assert_eq!(snapshot["status"]["dead_letters"], 0);
    assert_eq!(snapshot["confirmations"].as_array().unwrap().len(), 0);
    assert_eq!(snapshot["dead_letters"].as_array().unwrap().len(), 0);
    assert_eq!(snapshot["buffered_events"].as_array().unwrap().len(), 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_snapshot_pretty_json_includes_newlines() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json_pretty(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };

    assert!(snapshot_text.contains('\n'));
    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_healthcheck_via_c_api() {
    let (base_url, requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({ "ok": true }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let health = unsafe { stateset_sync_runtime_healthcheck(init.value) };
    assert_eq!(health.code, FfiErrorCode::Ok);
    assert_eq!(health.value, 1);

    let captured = requests.recv().unwrap();
    assert!(captured.request_line.starts_with("GET /health "));

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_refresh_remote_head_json_via_c_api() {
    let (base_url, requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "head_sequence": 42,
            "state_root": "root-42",
            "latest_commitment": {
                "batch_id": "BATCH-42"
            }
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let remote_head_ptr = unsafe { stateset_sync_runtime_refresh_remote_head_json(init.value) };
    assert!(!remote_head_ptr.is_null());
    let remote_head_text = unsafe { CStr::from_ptr(remote_head_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(remote_head_ptr) };

    let remote_head: Value = serde_json::from_str(&remote_head_text).unwrap();
    assert_eq!(remote_head["remote_head"], 42);
    assert_eq!(remote_head["state_root"], "root-42");
    assert_eq!(remote_head["last_commitment_id"], "BATCH-42");

    let captured = requests.recv().unwrap();
    assert!(captured.request_line.contains("GET /api/v1/head?"));

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_record_event_and_push_json_via_c_api() {
    let (base_url, requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "batchId": "B-FFI-1",
            "eventsAccepted": 1,
            "eventsRejected": 0,
            "sequenceStart": 11,
            "sequenceEnd": 11,
            "headSequence": 11
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_json = signed_event_json("created");
    let event: SyncEvent = serde_json::from_str(&event_json).unwrap();
    let event_json = CString::new(event_json).unwrap();
    let record =
        unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
    assert_eq!(record.code, FfiErrorCode::Ok);
    assert_eq!(record.value, 1);

    let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
    assert!(!push_ptr.is_null());
    let push_text = unsafe { CStr::from_ptr(push_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(push_ptr) };

    let push: Value = serde_json::from_str(&push_text).unwrap();
    assert_eq!(push["accepted"], 1);
    assert_eq!(push["remote_head"], 11);
    assert_eq!(push["acknowledgements"][0]["event_id"], json!(event.id));
    assert_eq!(push["acknowledgements"][0]["remote_sequence"], 11);

    let captured = requests.recv().unwrap();
    assert!(captured.request_line.starts_with("POST /api/v1/ves/events/ingest "));
    let body: Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(body["events"][0]["event_id"], json!(event.id));
    assert_eq!(body["events"][0]["agent_signature"], json!("sig-created"));
    assert_eq!(body["events"][0]["command_id"], json!("cmd-created"));

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
    let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
    assert_eq!(snapshot["status"]["pending"], 0);
    assert_eq!(snapshot["confirmations"].as_array().unwrap().len(), 1);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_pull_json_via_c_api() {
    let event_id = Uuid::new_v4();
    let (base_url, requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "events": [
                {
                    "envelope": {
                        "event_id": event_id,
                        "entity_type": "order",
                        "entity_id": "ORD-PULL-1",
                        "event_type": "order.shipped",
                        "payload": { "status": "shipped" },
                        "created_at": "2024-03-01T00:00:00Z",
                        "sequence_number": 7
                    },
                    "sequenced_at": "2024-03-01T00:00:01Z"
                }
            ],
            "head_sequence": 7,
            "has_more": false
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
    assert!(!pull_ptr.is_null());
    let pull_text = unsafe { CStr::from_ptr(pull_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(pull_ptr) };

    let pull: Value = serde_json::from_str(&pull_text).unwrap();
    assert_eq!(pull["remote_head"], 7);
    assert_eq!(pull["events"].as_array().unwrap().len(), 1);
    assert_eq!(pull["events"][0]["id"], json!(event_id));

    let captured = requests.recv().unwrap();
    assert!(captured.request_line.contains("GET /api/v1/events?"));
    assert!(captured.request_line.contains("tenant_id=tenant-ffi"));
    assert!(captured.request_line.contains("store_id=store-ffi"));

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
    let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
    assert_eq!(snapshot["buffered_events"].as_array().unwrap().len(), 1);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_full_sync_json_via_c_api() {
    let event_id = Uuid::new_v4();
    let (base_url, requests) = spawn_response_server(vec![
        StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "batchId": "B-FFI-2",
                "eventsAccepted": 1,
                "eventsRejected": 0,
                "sequenceStart": 12,
                "sequenceEnd": 12,
                "headSequence": 12
            }),
        },
        StubResponse {
            status: "200 OK".to_string(),
            body: json!({
                "events": [
                    {
                        "envelope": {
                            "event_id": event_id,
                            "entity_type": "order",
                            "entity_id": "ORD-FULL-1",
                            "event_type": "order.confirmed",
                            "payload": { "status": "confirmed" },
                            "created_at": "2024-03-01T00:00:00Z",
                            "sequence_number": 13
                        },
                        "sequenced_at": "2024-03-01T00:00:02Z"
                    }
                ],
                "head_sequence": 13,
                "has_more": false
            }),
        },
    ]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_json = CString::new(signed_event_json("confirmed")).unwrap();
    let record =
        unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
    assert_eq!(record.code, FfiErrorCode::Ok);

    let full_sync_ptr = unsafe { stateset_sync_runtime_full_sync_json(init.value) };
    assert!(!full_sync_ptr.is_null());
    let full_sync_text = unsafe { CStr::from_ptr(full_sync_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(full_sync_ptr) };

    let full_sync: Value = serde_json::from_str(&full_sync_text).unwrap();
    assert_eq!(full_sync["push"]["accepted"], 1);
    assert_eq!(full_sync["pull"]["events"].as_array().unwrap().len(), 1);
    assert_eq!(full_sync["pull"]["events"][0]["id"], json!(event_id));

    let first = requests.recv().unwrap();
    assert!(first.request_line.starts_with("POST /api/v1/ves/events/ingest "));
    let second = requests.recv().unwrap();
    assert!(second.request_line.contains("GET /api/v1/events?"));

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_confirmations_json_and_drain_via_c_api() {
    let (base_url, _requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "batchId": "B-FFI-3",
            "eventsAccepted": 1,
            "eventsRejected": 0,
            "sequenceStart": 21,
            "sequenceEnd": 21,
            "headSequence": 21
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_json = signed_event_json("confirmations");
    let event: SyncEvent = serde_json::from_str(&event_json).unwrap();
    let event_json = CString::new(event_json).unwrap();
    let record =
        unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
    assert_eq!(record.code, FfiErrorCode::Ok);
    let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
    assert!(!push_ptr.is_null());
    unsafe { crate::strings::stateset_string_free(push_ptr) };

    let confirmations_ptr = unsafe { stateset_sync_runtime_confirmations_json(init.value) };
    assert!(!confirmations_ptr.is_null());
    let confirmations_text =
        unsafe { CStr::from_ptr(confirmations_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(confirmations_ptr) };
    let confirmations: Value = serde_json::from_str(&confirmations_text).unwrap();
    assert_eq!(confirmations.as_array().unwrap().len(), 1);
    assert_eq!(confirmations[0]["event_id"], json!(event.id));

    let confirmation_ptr = unsafe {
        stateset_sync_runtime_confirmation_for_event_json(init.value, FfiUuid::from(event.id))
    };
    assert!(!confirmation_ptr.is_null());
    let confirmation_text =
        unsafe { CStr::from_ptr(confirmation_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(confirmation_ptr) };
    let confirmation: Value = serde_json::from_str(&confirmation_text).unwrap();
    assert_eq!(confirmation["remote_sequence"], 21);

    let drained_ptr = unsafe { stateset_sync_runtime_drain_confirmations_json(init.value) };
    assert!(!drained_ptr.is_null());
    let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(drained_ptr) };
    let drained: Value = serde_json::from_str(&drained_text).unwrap();
    assert_eq!(drained.as_array().unwrap().len(), 1);

    let empty_ptr = unsafe { stateset_sync_runtime_confirmations_json(init.value) };
    assert!(!empty_ptr.is_null());
    let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(empty_ptr) };
    let empty: Value = serde_json::from_str(&empty_text).unwrap();
    assert_eq!(empty.as_array().unwrap().len(), 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[tokio::test]
async fn sync_runtime_dead_letters_json_and_drain_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_id = seed_dead_letter(init.value).await;

    let dead_letters_ptr = unsafe { stateset_sync_runtime_dead_letters_json(init.value) };
    assert!(!dead_letters_ptr.is_null());
    let dead_letters_text =
        unsafe { CStr::from_ptr(dead_letters_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(dead_letters_ptr) };
    let dead_letters: Value = serde_json::from_str(&dead_letters_text).unwrap();
    assert_eq!(dead_letters.as_array().unwrap().len(), 1);
    assert_eq!(dead_letters[0]["event"]["id"], json!(event_id));

    let dead_letter_ptr = unsafe {
        stateset_sync_runtime_dead_letter_for_event_json(init.value, FfiUuid::from(event_id))
    };
    assert!(!dead_letter_ptr.is_null());
    let dead_letter_text = unsafe { CStr::from_ptr(dead_letter_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(dead_letter_ptr) };
    let dead_letter: Value = serde_json::from_str(&dead_letter_text).unwrap();
    assert_eq!(dead_letter["event"]["id"], json!(event_id));

    let drained_ptr = unsafe { stateset_sync_runtime_drain_dead_letters_json(init.value) };
    assert!(!drained_ptr.is_null());
    let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(drained_ptr) };
    let drained: Value = serde_json::from_str(&drained_text).unwrap();
    assert_eq!(drained.as_array().unwrap().len(), 1);

    let empty_ptr = unsafe { stateset_sync_runtime_dead_letters_json(init.value) };
    assert!(!empty_ptr.is_null());
    let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(empty_ptr) };
    let empty: Value = serde_json::from_str(&empty_text).unwrap();
    assert_eq!(empty.as_array().unwrap().len(), 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_buffered_events_json_and_drain_via_c_api() {
    let event_id = Uuid::new_v4();
    let (base_url, _requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "events": [
                {
                    "envelope": {
                        "event_id": event_id,
                        "entity_type": "order",
                        "entity_id": "ORD-BUFFER-1",
                        "event_type": "order.buffered",
                        "payload": { "status": "buffered" },
                        "created_at": "2024-03-01T00:00:00Z",
                        "sequence_number": 31
                    },
                    "sequenced_at": "2024-03-01T00:00:01Z"
                }
            ],
            "head_sequence": 31,
            "has_more": false
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
    assert!(!pull_ptr.is_null());
    unsafe { crate::strings::stateset_string_free(pull_ptr) };

    let buffered_ptr = unsafe { stateset_sync_runtime_buffered_events_json(init.value) };
    assert!(!buffered_ptr.is_null());
    let buffered_text = unsafe { CStr::from_ptr(buffered_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(buffered_ptr) };
    let buffered: Value = serde_json::from_str(&buffered_text).unwrap();
    assert_eq!(buffered.as_array().unwrap().len(), 1);
    assert_eq!(buffered[0]["id"], json!(event_id));

    let drained_ptr = unsafe { stateset_sync_runtime_drain_buffer_json(init.value) };
    assert!(!drained_ptr.is_null());
    let drained_text = unsafe { CStr::from_ptr(drained_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(drained_ptr) };
    let drained: Value = serde_json::from_str(&drained_text).unwrap();
    assert_eq!(drained.as_array().unwrap().len(), 1);
    assert_eq!(drained[0]["id"], json!(event_id));

    let empty_ptr = unsafe { stateset_sync_runtime_buffered_events_json(init.value) };
    assert!(!empty_ptr.is_null());
    let empty_text = unsafe { CStr::from_ptr(empty_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(empty_ptr) };
    let empty: Value = serde_json::from_str(&empty_text).unwrap();
    assert_eq!(empty.as_array().unwrap().len(), 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_scalar_status_defaults_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    assert_eq!(unsafe { stateset_sync_runtime_initialized(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_local_head(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_remote_cursor(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_confirmation_count(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_dead_letter_count(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_buffered_count(init.value) }.value, 0);

    let event_type = CString::new("order.created").unwrap();
    let entity_type = CString::new("order").unwrap();
    let entity_id = CString::new("ORD-SCALAR-1").unwrap();
    let payload = CString::new(json!({"total": 1}).to_string()).unwrap();
    let record = unsafe {
        stateset_sync_runtime_record_json(
            init.value,
            event_type.as_ptr(),
            entity_type.as_ptr(),
            entity_id.as_ptr(),
            payload.as_ptr(),
        )
    };
    assert_eq!(record.code, FfiErrorCode::Ok);

    assert_eq!(unsafe { stateset_sync_runtime_local_head(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_scalar_status_after_push_via_c_api() {
    let (base_url, _requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "batchId": "B-FFI-5",
            "eventsAccepted": 1,
            "eventsRejected": 0,
            "sequenceStart": 51,
            "sequenceEnd": 51,
            "headSequence": 51
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_json = CString::new(signed_event_json("scalar-push")).unwrap();
    let record =
        unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
    assert_eq!(record.code, FfiErrorCode::Ok);
    let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
    assert!(!push_ptr.is_null());
    unsafe { crate::strings::stateset_string_free(push_ptr) };

    assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 51);
    assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 51);
    assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_confirmation_count(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_scalar_status_after_pull_via_c_api() {
    let event_id = Uuid::new_v4();
    let (base_url, _requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "events": [
                {
                    "envelope": {
                        "event_id": event_id,
                        "entity_type": "order",
                        "entity_id": "ORD-SCALAR-PULL-1",
                        "event_type": "order.pulled",
                        "payload": { "status": "pulled" },
                        "created_at": "2024-03-01T00:00:00Z",
                        "sequence_number": 31
                    },
                    "sequenced_at": "2024-03-01T00:00:01Z"
                }
            ],
            "head_sequence": 31,
            "has_more": false
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let pull_ptr = unsafe { stateset_sync_runtime_pull_json(init.value) };
    assert!(!pull_ptr.is_null());
    unsafe { crate::strings::stateset_string_free(pull_ptr) };

    assert_eq!(unsafe { stateset_sync_runtime_remote_head(init.value) }.value, 31);
    assert_eq!(unsafe { stateset_sync_runtime_remote_cursor(init.value) }.value, 31);
    assert_eq!(unsafe { stateset_sync_runtime_lag(init.value) }.value, 0);
    assert_eq!(unsafe { stateset_sync_runtime_buffered_count(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_caught_up(init.value) }.value, 1);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[tokio::test]
async fn sync_runtime_scalar_dead_letter_count_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let _event_id = seed_dead_letter(init.value).await;
    assert_eq!(unsafe { stateset_sync_runtime_dead_letter_count(init.value) }.value, 1);
    assert_eq!(unsafe { stateset_sync_runtime_pending_count(init.value) }.value, 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_confirmation_scoped_queries_via_c_api() {
    let (base_url, _requests) = spawn_response_server(vec![StubResponse {
        status: "200 OK".to_string(),
        body: json!({
            "batchId": "B-FFI-4",
            "eventsAccepted": 2,
            "eventsRejected": 0,
            "sequenceStart": 41,
            "sequenceEnd": 42,
            "headSequence": 42,
            "receipt": {
                "batchId": "B-FFI-4",
                "receiptHash": "receipt-scope"
            }
        }),
    }]);
    let config_json = CString::new(runtime_config_json_for(&base_url)).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let first = SyncEvent::new("order.created", "order", "ORD-SCOPE-1", json!({ "step": 1 }))
        .with_signature("sig-scope-1")
        .with_command_id("cmd-scope")
        .with_source_agent_id("agent-ffi")
        .with_agent_key_id(7);
    let second = SyncEvent::new("order.confirmed", "order", "ORD-SCOPE-1", json!({ "step": 2 }))
        .with_signature("sig-scope-2")
        .with_command_id("cmd-scope")
        .with_source_agent_id("agent-ffi")
        .with_agent_key_id(7);

    for event in [&first, &second] {
        let event_json = CString::new(serde_json::to_string(event).unwrap()).unwrap();
        let record =
            unsafe { stateset_sync_runtime_record_event_json(init.value, event_json.as_ptr()) };
        assert_eq!(record.code, FfiErrorCode::Ok);
    }

    let push_ptr = unsafe { stateset_sync_runtime_push_json(init.value) };
    assert!(!push_ptr.is_null());
    unsafe { crate::strings::stateset_string_free(push_ptr) };

    let by_remote_ptr =
        unsafe { stateset_sync_runtime_confirmation_for_remote_sequence_json(init.value, 42) };
    assert!(!by_remote_ptr.is_null());
    let by_remote_text = unsafe { CStr::from_ptr(by_remote_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_remote_ptr) };
    let by_remote: Value = serde_json::from_str(&by_remote_text).unwrap();
    assert_eq!(by_remote["event_id"], json!(second.id));

    let receipt = CString::new("receipt-scope").unwrap();
    let by_receipt_ptr = unsafe {
        stateset_sync_runtime_confirmations_for_receipt_json(init.value, receipt.as_ptr())
    };
    assert!(!by_receipt_ptr.is_null());
    let by_receipt_text = unsafe { CStr::from_ptr(by_receipt_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_receipt_ptr) };
    let by_receipt: Value = serde_json::from_str(&by_receipt_text).unwrap();
    assert_eq!(by_receipt.as_array().unwrap().len(), 2);

    let command_id = CString::new("cmd-scope").unwrap();
    let by_command_ptr = unsafe {
        stateset_sync_runtime_confirmations_for_command_json(init.value, command_id.as_ptr())
    };
    assert!(!by_command_ptr.is_null());
    let by_command_text = unsafe { CStr::from_ptr(by_command_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_command_ptr) };
    let by_command: Value = serde_json::from_str(&by_command_text).unwrap();
    assert_eq!(by_command.as_array().unwrap().len(), 2);

    let entity_type = CString::new("order").unwrap();
    let entity_id = CString::new("ORD-SCOPE-1").unwrap();
    let by_entity_ptr = unsafe {
        stateset_sync_runtime_confirmations_for_entity_json(
            init.value,
            entity_type.as_ptr(),
            entity_id.as_ptr(),
        )
    };
    assert!(!by_entity_ptr.is_null());
    let by_entity_text = unsafe { CStr::from_ptr(by_entity_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_entity_ptr) };
    let by_entity: Value = serde_json::from_str(&by_entity_text).unwrap();
    assert_eq!(by_entity.as_array().unwrap().len(), 2);

    let latest_command_ptr = unsafe {
        stateset_sync_runtime_latest_confirmation_for_command_json(init.value, command_id.as_ptr())
    };
    assert!(!latest_command_ptr.is_null());
    let latest_command_text =
        unsafe { CStr::from_ptr(latest_command_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(latest_command_ptr) };
    let latest_command: Value = serde_json::from_str(&latest_command_text).unwrap();
    assert_eq!(latest_command["event_id"], json!(second.id));
    assert_eq!(latest_command["remote_sequence"], 42);

    let latest_entity_ptr = unsafe {
        stateset_sync_runtime_latest_confirmation_for_entity_json(
            init.value,
            entity_type.as_ptr(),
            entity_id.as_ptr(),
        )
    };
    assert!(!latest_entity_ptr.is_null());
    let latest_entity_text =
        unsafe { CStr::from_ptr(latest_entity_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(latest_entity_ptr) };
    let latest_entity: Value = serde_json::from_str(&latest_entity_text).unwrap();
    assert_eq!(latest_entity["event_id"], json!(second.id));
    assert_eq!(latest_entity["remote_sequence"], 42);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn sync_runtime_dead_letter_scoped_queries_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let lease = begin_sync_runtime_use(init.value).unwrap();
    let runtime = lease.runtime();
    let mut runtime = lock_sync_runtime(runtime);
    let first = SyncEvent::new("payment.failed", "payment", "PAY-SCOPE-1", json!({ "step": 1 }))
        .with_command_id("cmd-dead-scope");
    let first_id = first.id;
    let second =
        SyncEvent::new("payment.retry_failed", "payment", "PAY-SCOPE-1", json!({ "step": 2 }))
            .with_command_id("cmd-dead-scope");
    let second_id = second.id;
    runtime.record(first).unwrap();
    runtime.record(second).unwrap();
    runtime.engine_mut().push(&RejectingTransport).await.unwrap();
    drop(runtime);
    drop(lease);

    let command_id = CString::new("cmd-dead-scope").unwrap();
    let by_command_ptr = unsafe {
        stateset_sync_runtime_dead_letters_for_command_json(init.value, command_id.as_ptr())
    };
    assert!(!by_command_ptr.is_null());
    let by_command_text = unsafe { CStr::from_ptr(by_command_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_command_ptr) };
    let by_command: Value = serde_json::from_str(&by_command_text).unwrap();
    assert_eq!(by_command.as_array().unwrap().len(), 2);

    let entity_type = CString::new("payment").unwrap();
    let entity_id = CString::new("PAY-SCOPE-1").unwrap();
    let by_entity_ptr = unsafe {
        stateset_sync_runtime_dead_letters_for_entity_json(
            init.value,
            entity_type.as_ptr(),
            entity_id.as_ptr(),
        )
    };
    assert!(!by_entity_ptr.is_null());
    let by_entity_text = unsafe { CStr::from_ptr(by_entity_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_entity_ptr) };
    let by_entity: Value = serde_json::from_str(&by_entity_text).unwrap();
    assert_eq!(by_entity.as_array().unwrap().len(), 2);

    let latest_command_ptr = unsafe {
        stateset_sync_runtime_latest_dead_letter_for_command_json(init.value, command_id.as_ptr())
    };
    assert!(!latest_command_ptr.is_null());
    let latest_command_text =
        unsafe { CStr::from_ptr(latest_command_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(latest_command_ptr) };
    let latest_command: Value = serde_json::from_str(&latest_command_text).unwrap();
    assert_eq!(latest_command["event"]["id"], json!(second_id));

    let latest_entity_ptr = unsafe {
        stateset_sync_runtime_latest_dead_letter_for_entity_json(
            init.value,
            entity_type.as_ptr(),
            entity_id.as_ptr(),
        )
    };
    assert!(!latest_entity_ptr.is_null());
    let latest_entity_text =
        unsafe { CStr::from_ptr(latest_entity_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(latest_entity_ptr) };
    let latest_entity: Value = serde_json::from_str(&latest_entity_text).unwrap();
    assert_eq!(latest_entity["event"]["id"], json!(second_id));

    let by_event_ptr = unsafe {
        stateset_sync_runtime_dead_letter_for_event_json(init.value, FfiUuid::from(first_id))
    };
    assert!(!by_event_ptr.is_null());
    let by_event_text = unsafe { CStr::from_ptr(by_event_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(by_event_ptr) };
    let by_event: Value = serde_json::from_str(&by_event_text).unwrap();
    assert_eq!(by_event["event"]["id"], json!(first_id));

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[tokio::test]
async fn sync_runtime_requeue_dead_letter_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_id = seed_dead_letter(init.value).await;

    let requeue =
        unsafe { stateset_sync_runtime_requeue_dead_letter(init.value, FfiUuid::from(event_id)) };
    assert_eq!(requeue.code, FfiErrorCode::Ok);
    assert_eq!(requeue.value, 2);

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
    let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
    assert_eq!(snapshot["status"]["pending"], 1);
    assert_eq!(snapshot["status"]["dead_letters"], 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[tokio::test]
async fn sync_runtime_discard_dead_letter_json_via_c_api() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);

    let event_id = seed_dead_letter(init.value).await;

    let discarded_ptr = unsafe {
        stateset_sync_runtime_discard_dead_letter_json(init.value, FfiUuid::from(event_id))
    };
    assert!(!discarded_ptr.is_null());
    let discarded_text = unsafe { CStr::from_ptr(discarded_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(discarded_ptr) };

    let discarded: Value = serde_json::from_str(&discarded_text).unwrap();
    assert_eq!(discarded["event"]["entity_id"], "PAY-FFI-1");
    assert_eq!(discarded["rejection"]["code"], "invalid_event");

    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(init.value) };
    assert!(!snapshot_ptr.is_null());
    let snapshot_text = unsafe { CStr::from_ptr(snapshot_ptr) }.to_str().unwrap().to_owned();
    unsafe { crate::strings::stateset_string_free(snapshot_ptr) };
    let snapshot: Value = serde_json::from_str(&snapshot_text).unwrap();
    assert_eq!(snapshot["status"]["pending"], 0);
    assert_eq!(snapshot["status"]["dead_letters"], 0);

    unsafe { stateset_sync_runtime_destroy(init.value) };
}

#[test]
fn sync_runtime_record_null_handle_is_rejected() {
    let event_type = CString::new("order.created").unwrap();
    let entity_type = CString::new("order").unwrap();
    let entity_id = CString::new("ORD-NULL").unwrap();
    let payload = CString::new("{}").unwrap();

    let result = unsafe {
        stateset_sync_runtime_record_json(
            std::ptr::null_mut(),
            event_type.as_ptr(),
            entity_type.as_ptr(),
            entity_id.as_ptr(),
            payload.as_ptr(),
        )
    };
    assert_eq!(result.code, FfiErrorCode::NullPointer);
}

#[test]
fn sync_runtime_snapshot_after_destroy_is_rejected() {
    let config_json = CString::new(runtime_config_json()).unwrap();
    let init = unsafe { stateset_sync_runtime_init_from_json(config_json.as_ptr()) };
    assert_eq!(init.code, FfiErrorCode::Ok);
    let handle = init.value;

    unsafe { stateset_sync_runtime_destroy(handle) };
    let snapshot_ptr = unsafe { stateset_sync_runtime_snapshot_json(handle) };
    assert!(snapshot_ptr.is_null());
    let err = crate::error::last_error_as_str();
    assert!(err.as_deref().is_some_and(|msg| msg.contains("invalid or stale sync runtime handle")));
}
