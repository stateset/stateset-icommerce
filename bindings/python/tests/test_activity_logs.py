"""Activity Logs API tests for the stateset_embedded Python bindings.

Metadata crosses as a JSON string, enums as snake_case strings, timestamps
as RFC3339 strings.
"""

import json
import uuid

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_activity_logs_full_lifecycle(commerce):
    logs = commerce.activity_logs
    if not logs.is_supported():
        pytest.skip("activity logs are not supported by this engine build")

    subject_id = str(uuid.uuid4())

    entry = logs.record(
        subject_type="sales_order",
        subject_id=subject_id,
        action="status_changed",
        summary="Status changed from pending to shipped",
        actor_kind="agent",
        actor="agent-1",
        metadata=json.dumps({"from": "pending", "to": "shipped"}),
    )
    assert entry.id
    assert entry.subject_type == "sales_order"
    assert entry.subject_id == subject_id
    assert entry.action == "status_changed"
    assert entry.actor_kind == "agent"
    assert entry.actor == "agent-1"
    assert json.loads(entry.metadata) == {"from": "pending", "to": "shipped"}
    assert entry.created_at

    # get round-trips the entry
    found = logs.get(entry.id)
    assert found is not None
    assert found.id == entry.id

    # a second entry for the same subject, defaulting the actor kind
    second = logs.record(
        subject_type="sales_order",
        subject_id=subject_id,
        action="note_added",
        summary="Customer note added",
    )
    assert second.actor_kind == "user"

    # list filters by subject and action
    listed = logs.list(subject_type="sales_order", subject_id=subject_id)
    assert {e.id for e in listed} == {entry.id, second.id}
    by_action = logs.list(action="note_added")
    assert [e.id for e in by_action] == [second.id]
    by_kind = logs.list(actor_kind="agent")
    assert [e.id for e in by_kind] == [entry.id]
    assert len(logs.list(subject_id=subject_id, limit=1)) == 1

    # history_for_subject returns everything for the subject
    history = logs.history_for_subject("sales_order", subject_id)
    assert {e.id for e in history} == {entry.id, second.id}

    # unknown ids and bad enums behave predictably
    assert logs.get(str(uuid.uuid4())) is None
    with pytest.raises(ValueError):
        logs.list(actor_kind="not_a_kind")
