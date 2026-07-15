import json
import os
import threading
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer

from stateset_embedded import (
    SyncConfirmation,
    SyncDeadLetter,
    SyncEvent,
    SyncFullSyncResult,
    SyncPullResult,
    SyncPushResult,
    SyncRemoteHead,
    SyncRuntime,
    SyncSnapshot,
    SyncStatus,
)


def build_sync_config(base_url: str, tmp_path):
    return {
        "sequencer_base_url": base_url,
        "engine": {
            "agent_id": "agent-1",
            "tenant_id": "tenant-1",
            "store_id": "store-1",
            "buffer_capacity": 16,
            "batch_size": 8,
            "outbox_capacity": 16,
            "confirmation_capacity": 16,
            "outbox_path": str(tmp_path / "sync-outbox.json"),
            "state_path": str(tmp_path / "sync-state.json"),
            # The stub sequencer publishes commitment metadata without a
            # signed manifest; opt out of the fail-closed trust default for
            # this trusted in-process test environment.
            "commitment_trust": {"require_manifest": False},
        },
        "agent_key_id": 7,
    }


def make_sync_runtime(base_url: str, tmp_path) -> SyncRuntime:
    return SyncRuntime(json.dumps(build_sync_config(base_url, tmp_path)))


def test_sync_runtime_records_events_and_reports_snapshot(tmp_path):
    runtime = make_sync_runtime("http://127.0.0.1:1", tmp_path)

    sequence = runtime.record(
        "order.created",
        "order",
        "ORD-PY-1",
        json.dumps({"status": "created", "total": 42.5}),
        command_id="cmd-py-1",
        source_agent_id="agent-1",
        agent_key_id=7,
    )

    assert sequence == 1
    assert runtime.initialized is True
    assert runtime.caught_up is False
    assert runtime.local_head == 1
    assert runtime.pending_count == 1
    assert runtime.confirmation_count == 0
    assert runtime.dead_letter_count == 0
    assert runtime.buffered_count == 0
    assert runtime.remote_head == 0
    assert runtime.remote_cursor == 0
    assert runtime.lag == 0

    status = runtime.status()
    assert isinstance(status, SyncStatus)
    assert status.pending == 1
    assert status.local_head == 1
    assert status.caught_up is False

    snapshot = runtime.snapshot()
    assert isinstance(snapshot, SyncSnapshot)
    assert snapshot.status.pending == 1
    assert snapshot.confirmations == []
    assert snapshot.dead_letters == []
    assert snapshot.buffered_events == []

    status_json = json.loads(runtime.status_json())
    assert status_json["pending"] == 1

    snapshot_json = json.loads(runtime.snapshot_json(pretty=True))
    assert snapshot_json["status"]["pending"] == 1


def test_sync_runtime_from_file_and_env(tmp_path):
    config = build_sync_config("http://127.0.0.1:1", tmp_path)
    config_path = tmp_path / "sync-runtime.json"
    config_path.write_text(json.dumps(config), encoding="utf-8")

    runtime = SyncRuntime.from_file(str(config_path))
    runtime.record("inventory.adjusted", "inventory", "SKU-PY-1", json.dumps({"delta": 5}))
    assert runtime.pending_count == 1
    assert isinstance(runtime.status(), SyncStatus)

    prefix = "STATESET_PY_SYNC_"
    env_values = {
        f"{prefix}SEQUENCER_BASE_URL": "http://127.0.0.1:1",
        f"{prefix}AGENT_ID": "agent-env",
        f"{prefix}TENANT_ID": "tenant-env",
        f"{prefix}STORE_ID": "store-env",
        f"{prefix}OUTBOX_PATH": str(tmp_path / "env-outbox.json"),
        f"{prefix}STATE_PATH": str(tmp_path / "env-state.json"),
        f"{prefix}AGENT_KEY_ID": "9",
    }
    previous = {key: os.environ.get(key) for key in env_values}
    try:
        os.environ.update(env_values)
        env_runtime = SyncRuntime.from_env(prefix)
        assert env_runtime.initialized is True
        assert env_runtime.pending_count == 0
        assert isinstance(env_runtime.snapshot(), SyncSnapshot)
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


class _SequencerStubHandler(BaseHTTPRequestHandler):
    state = {
        "push_mode": "accept",
        "pushed_event_id": None,
        "push_body": None,
        "pulled_event_id": str(uuid.uuid4()),
    }

    def _send_json(self, payload, status=200):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            self._send_json({"ok": True})
            return

        if self.path.startswith("/api/v1/head"):
            self._send_json(
                {
                    "headSequence": 1,
                    "stateRoot": "root-1",
                    "latestCommitment": {"batchId": "batch-1"},
                }
            )
            return

        if self.path.startswith("/api/v1/events?"):
            self._send_json(
                {
                    "events": [
                        {
                            "envelope": {
                                "event_id": self.state["pulled_event_id"],
                                "entity_type": "order",
                                "entity_id": "ORD-PULL-1",
                                "event_type": "order.confirmed",
                                "command_id": "cmd-pull-1",
                                "payload": {"status": "confirmed"},
                                "payload_plain_hash": "hash-pull-1",
                                "agent_signature": "sig-pull-1",
                                "source_agent_id": "sequencer",
                                "agent_key_id": 3,
                                "base_version": 2,
                                "created_at": datetime.now(timezone.utc).isoformat(),
                                "sequence_number": 3,
                            }
                        }
                    ],
                    "headSequence": 3,
                    "hasMore": False,
                }
            )
            return

        self._send_json({"error": "not found"}, status=404)

    def do_POST(self):
        if self.path == "/api/v1/ves/events/ingest":
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length) or b"{}")
            self.state["push_body"] = body
            self.state["pushed_event_id"] = body["events"][0]["event_id"]
            if self.state["push_mode"] == "reject":
                self._send_json(
                    {
                        "eventsAccepted": 0,
                        "eventsRejected": 1,
                        "headSequence": 1,
                        "rejections": [
                            {
                                "eventId": self.state["pushed_event_id"],
                                "code": "invalid_signature",
                                "reason": "signature rejected",
                                "retryable": False,
                            }
                        ],
                    }
                )
                return

            self._send_json(
                {
                    "eventsAccepted": 1,
                    "eventsRejected": 0,
                    "headSequence": 2,
                    "receipts": [
                        {
                            "eventId": self.state["pushed_event_id"],
                            "sequenceNumber": 2,
                            "receiptHash": "rcpt-1",
                        }
                    ],
                }
            )
            return

        self._send_json({"error": "not found"}, status=404)

    def log_message(self, format, *args):  # noqa: A003
        return


class _SequencerStubServer:
    def __init__(self, push_mode: str = "accept"):
        self.push_mode = push_mode

    def __enter__(self):
        _SequencerStubHandler.state = {
            "push_mode": self.push_mode,
            "pushed_event_id": None,
            "push_body": None,
            "pulled_event_id": str(uuid.uuid4()),
        }
        self.server = HTTPServer(("127.0.0.1", 0), _SequencerStubHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        host, port = self.server.server_address
        self.base_url = f"http://{host}:{port}"
        return self

    def __exit__(self, exc_type, exc, tb):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


def test_sync_runtime_push_pull_and_confirmations(tmp_path):
    with _SequencerStubServer() as server:
        runtime = make_sync_runtime(server.base_url, tmp_path)

        assert runtime.healthcheck() is True

        remote_head = runtime.refresh_remote_head()
        assert isinstance(remote_head, SyncRemoteHead)
        assert remote_head.remote_head == 1
        assert remote_head.state_root == "root-1"
        assert remote_head.last_commitment_id == "batch-1"
        assert runtime.remote_state_root == "root-1"
        assert runtime.last_commitment_id == "batch-1"

        local_sequence = runtime.record(
            "order.created",
            "order",
            "ORD-PUSH-1",
            json.dumps({"status": "created"}),
            command_id="cmd-push-1",
            source_agent_id="agent-1",
            agent_key_id=7,
            signature="sig-local-1",
        )
        assert local_sequence == 1

        push = runtime.push()
        assert isinstance(push, SyncPushResult)
        assert push.accepted == 1
        assert push.remote_head == 2
        assert push.acknowledged_head == 2
        assert len(push.acknowledgements) == 1
        confirmation_ack = push.acknowledgements[0]
        assert confirmation_ack.receipt == "rcpt-1"
        assert runtime.pending_count == 0
        assert runtime.confirmation_count == 1
        assert runtime.last_acknowledged_remote_sequence == 2

        confirmation = runtime.confirmation_for_event(confirmation_ack.event_id)
        assert isinstance(confirmation, SyncConfirmation)
        assert confirmation.remote_sequence == 2
        assert confirmation.command_id == "cmd-push-1"

        confirmations = runtime.confirmations()
        assert len(confirmations) == 1
        assert confirmations[0].local_sequence == 1

        pull = runtime.pull()
        assert isinstance(pull, SyncPullResult)
        assert pull.remote_head == 3
        assert len(pull.events) == 1
        assert isinstance(pull.events[0], SyncEvent)
        assert pull.events[0].sequence_authority == "canonical_remote"
        assert runtime.buffered_count == 1
        assert runtime.remote_head == 3
        assert runtime.remote_cursor == 3
        assert runtime.lag == 0
        assert runtime.caught_up is True

        snapshot = runtime.snapshot()
        assert isinstance(snapshot, SyncSnapshot)
        assert snapshot.status.caught_up is True
        assert len(snapshot.confirmations) == 1
        assert len(snapshot.buffered_events) == 1

        buffered = runtime.buffered_events()
        assert len(buffered) == 1
        assert buffered[0].entity_id == "ORD-PULL-1"

        drained = runtime.drain_buffer()
        assert len(drained) == 1
        assert runtime.buffered_count == 0

        drained_confirmations = runtime.drain_confirmations()
        assert len(drained_confirmations) == 1
        assert runtime.confirmation_count == 0

        full_sync = runtime.full_sync()
        assert isinstance(full_sync, SyncFullSyncResult)
        assert isinstance(full_sync.push, SyncPushResult)
        assert isinstance(full_sync.pull, SyncPullResult)


def test_sync_runtime_rejected_push_exposes_typed_dead_letters(tmp_path):
    with _SequencerStubServer(push_mode="reject") as server:
        runtime = make_sync_runtime(server.base_url, tmp_path)

        runtime.record(
            "payment.failed",
            "payment",
            "PAY-REJECT-1",
            json.dumps({"status": "failed"}),
            command_id="cmd-reject-1",
            signature="sig-bad-1",
        )

        push = runtime.push()
        assert isinstance(push, SyncPushResult)
        assert push.accepted == 0
        assert len(push.rejections) == 1
        assert push.rejections[0].reason == "signature rejected"
        assert push.rejections[0].retryable is False
        assert runtime.dead_letter_count == 1

        dead_letters = runtime.dead_letters()
        assert len(dead_letters) == 1
        dead_letter = dead_letters[0]
        assert isinstance(dead_letter, SyncDeadLetter)
        assert dead_letter.event.entity_id == "PAY-REJECT-1"
        assert dead_letter.rejection.code == "invalid_signature"

        lookup = runtime.dead_letter_for_event(dead_letter.event.id)
        assert isinstance(lookup, SyncDeadLetter)
        assert lookup.rejection.reason == "signature rejected"

        discarded = runtime.discard_dead_letter(dead_letter.event.id)
        assert isinstance(discarded, SyncDeadLetter)
        assert discarded.event.entity_id == "PAY-REJECT-1"
        assert runtime.dead_letter_count == 0
