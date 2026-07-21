"""Fraud API tests for the stateset_embedded Python bindings.

Enums cross as snake_case strings, timestamps as RFC3339 strings.
"""

import uuid

import pytest
from stateset_embedded import Commerce, FraudSignalInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_fraud_api_exists(commerce):
    assert commerce.fraud is not None


def test_fraud_full_lifecycle(commerce):
    fraud = commerce.fraud
    if not fraud.is_supported():
        pytest.skip("fraud backend not supported on this engine build")

    order_id = str(uuid.uuid4())

    # create_assessment scores the supplied signals
    assessment = fraud.create_assessment(
        order_id=order_id,
        signals=[
            FraudSignalInput("velocity_spike", 0.9, "12 orders in an hour"),
            FraudSignalInput("address_mismatch", 0.5, "billing != shipping"),
        ],
    )
    assert assessment.order_id == order_id
    assert assessment.risk_score > 0.0
    assert len(assessment.signals) == 2
    assert assessment.signals[0].signal_type == "velocity_spike"
    assert assessment.signals[0].score == 0.9
    assert assessment.decision in ("accept", "review", "reject")
    assert assessment.reviewed_by is None
    assert assessment.created_at

    # get_assessment / list_assessments find it
    found = fraud.get_assessment(order_id)
    assert found is not None
    assert found.order_id == order_id
    assert any(a.order_id == order_id for a in fraud.list_assessments())
    assert fraud.get_assessment(str(uuid.uuid4())) is None

    # review_assessment records the manual decision
    reviewed = fraud.review_assessment(
        order_id, "reject", "analyst@example.com", "confirmed chargeback"
    )
    assert reviewed.decision == "reject"
    assert reviewed.reviewed_by == "analyst@example.com"
    assert reviewed.review_notes == "confirmed chargeback"
    assert reviewed.needs_review is False
    assert any(a.order_id == order_id for a in fraud.list_assessments(decision="reject"))

    # rules: create, get, update, list, active, delete
    rule = fraud.create_rule(
        name="High velocity",
        signal_type="velocity_spike",
        threshold=0.8,
        action="review",
        description="Flag rapid repeat ordering",
    )
    assert rule.id
    assert rule.name == "High velocity"
    assert rule.signal_type == "velocity_spike"
    assert rule.threshold == 0.8
    assert rule.action == "review"
    assert rule.enabled is True

    got = fraud.get_rule(rule.id)
    assert got is not None
    assert got.id == rule.id

    updated = fraud.update_rule(rule.id, threshold=0.6, action="reject")
    assert updated.threshold == 0.6
    assert updated.action == "reject"

    assert any(r.id == rule.id for r in fraud.list_rules(signal_type="velocity_spike"))
    assert any(r.id == rule.id for r in fraud.get_active_rules())

    disabled = fraud.update_rule(rule.id, enabled=False)
    assert disabled.enabled is False
    assert all(r.id != rule.id for r in fraud.get_active_rules())

    fraud.delete_rule(rule.id)
    assert fraud.get_rule(rule.id) is None


def test_fraud_rejects_invalid_enums(commerce):
    fraud = commerce.fraud
    if not fraud.is_supported():
        pytest.skip("fraud backend not supported on this engine build")

    with pytest.raises(ValueError):
        fraud.create_rule(
            name="bad", signal_type="not_a_signal", threshold=0.5, action="review"
        )
    with pytest.raises(ValueError):
        fraud.list_assessments(decision="maybe")
