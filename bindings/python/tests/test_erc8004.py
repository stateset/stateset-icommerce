"""ERC-8004 trustless agent tests for the stateset_embedded Python bindings.

Large integers (chain ids, feedback indexes, counts, signed feedback values)
cross as exact decimal strings; timestamps as RFC3339 strings; enums as
snake_case strings.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_erc8004_api_exists(commerce):
    assert commerce.erc8004 is not None


def test_erc8004_full_lifecycle(commerce):
    api = commerce.erc8004
    is_supported = getattr(api, "is_supported", None)
    if is_supported is not None and is_supported() is False:
        pytest.skip("ERC-8004 backend not supported on this engine build")

    registry = "0xregistry"
    agent = "agent-1"

    # ---- identity registry ----
    identity = api.register_identity(
        agent_registry=registry,
        agent_id=agent,
        agent_uri="https://agents.example/agent-1.json",
        owner_address="0xowner",
    )
    assert identity.id
    assert identity.agent_registry == registry
    assert identity.agent_id == agent
    assert identity.active is True

    found = api.get_identity(registry, agent)
    assert found is not None and found.id == identity.id

    updated = api.update_identity(
        registry, agent, agent_uri="https://agents.example/agent-1-v2.json"
    )
    assert updated.agent_uri == "https://agents.example/agent-1-v2.json"

    bound = api.set_agent_wallet(registry, agent, "0xwallet", proof_chain_id="1")
    assert bound.agent_wallet == "0xwallet"
    assert bound.wallet_proof_chain_id == "1"

    by_wallet = api.get_identity_by_wallet("0xwallet")
    assert by_wallet is not None and by_wallet.agent_id == agent

    listed = api.list_identities(agent_registry=registry)
    assert any(i.agent_id == agent for i in listed)
    assert api.count_identities(agent_registry=registry) == "1"

    cleared = api.clear_agent_wallet(registry, agent)
    assert cleared.agent_wallet is None

    # ---- reputation registry ----
    feedback = api.give_feedback(
        agent_registry=registry,
        agent_id=agent,
        client_address="0xclient",
        value="90",
        value_decimals=0,
        tag1="quality",
    )
    assert feedback.value == "90"
    assert feedback.is_revoked is False

    read = api.read_feedback(registry, agent, "0xclient", feedback.feedback_index)
    assert read is not None and read.id == feedback.id
    assert any(f.id == feedback.id for f in api.read_all_feedback(agent_registry=registry))

    summary = api.feedback_summary(registry, agent, client_addresses=["0xclient"])
    assert summary.count == "1"
    assert summary.summary_value == "90"

    revoked = api.revoke_feedback(registry, agent, "0xclient", feedback.feedback_index)
    assert revoked.is_revoked is True

    # ---- validation registry ----
    request = api.request_validation(
        "0xrequesthash", registry, agent, "0xvalidator", "https://validators.example/req"
    )
    assert request.request_hash == "0xrequesthash"

    response = api.respond_validation("0xrequesthash", 88, tag="audit")
    assert response.response == 88
    assert response.tag == "audit"

    status = api.validation_status("0xrequesthash")
    assert status is not None
    assert status.response == 88
    assert status.validator_address == "0xvalidator"

    vsummary = api.validation_summary(registry, agent)
    assert vsummary.count == "1"
    assert vsummary.average_response == 88
