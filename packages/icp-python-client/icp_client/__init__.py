"""Pip-installable Python SDK for the Intelligent Commerce Protocol (ICP-1.0).

Mirrors the JavaScript @stateset/icp-client API. Wraps the wire format,
signature scheme, and HTTP transport into a single ICPClient class.

Designed for the agent-developer ecosystem (Anthropic SDK, OpenAI Agents,
LangChain, LangGraph) where Python is the dominant language.

Usage:

    from icp_client import ICPClient

    client = ICPClient.create(
        handler_url="http://localhost:8787",
        principal="did:web:my-store.example",
    )

    caps = client.capabilities()
    stock = client.inventory(merchant=caps["merchant_aid"], settler="settler:...")
    order = client.purchase(
        merchant=caps["merchant_aid"],
        settler="settler:...",
        items=[{"sku": "WIDGET-001", "quantity": 1, "unit_price": {"amount": "29.99", "currency": "USDC"}}],
        max_total={"amount": "35.00", "currency": "USDC"},
    )
"""

from .client import ICPClient, ICPError, Identity, generate_identity, identity_from_seeds
from .codec import canonical_json, sign_ed25519, verify_ed25519
from .settlement import verify_settlement_receipt
from .webhook import verify_webhook

__version__ = "1.5.0"
__all__ = [
    "ICPClient",
    "ICPError",
    "Identity",
    "generate_identity",
    "identity_from_seeds",
    "canonical_json",
    "sign_ed25519",
    "verify_ed25519",
    "verify_settlement_receipt",
    "verify_webhook",
]
