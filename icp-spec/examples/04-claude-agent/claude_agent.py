#!/usr/bin/env python3
"""Claude / Anthropic SDK + ICP — end-to-end agentic commerce demo.

Spawns a live `icp-handler` subprocess, exposes the 7 ICP-1.0 verbs as
Anthropic SDK tools, and lets Claude execute a complete purchase
transaction through tool calls.

Run:
    export ANTHROPIC_API_KEY=sk-ant-...
    python3 claude_agent.py

Output: a full transcript of Claude's reasoning + every ICP tool call +
the final co-signed SettlementReceipt.

Without ANTHROPIC_API_KEY, falls back to a deterministic agent
simulator that runs the same tool sequence without LLM reasoning —
useful for CI and for demonstrating the architecture in environments
without API access.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

# Make the local Python SDK importable without install.
SDK = Path(__file__).resolve().parents[3] / "packages" / "icp-python-client"
sys.path.insert(0, str(SDK))

from icp_client import ICPClient, ICPError  # noqa: E402


# ---------------------------------------------------------------------------
# Handler subprocess
# ---------------------------------------------------------------------------

HANDLER_SCRIPT = (
    Path(__file__).resolve().parents[3] / "icp-handler" / "src" / "server.mjs"
)


class HandlerProc:
    def __init__(self):
        self.proc = subprocess.Popen(
            ["node", str(HANDLER_SCRIPT)],
            env={**os.environ, "PORT": "0"},
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.base_url = self._wait()

    def _wait(self, timeout: float = 5.0) -> str:
        deadline = time.time() + timeout
        buf = ""
        while time.time() < deadline:
            chunk = os.read(self.proc.stderr.fileno(), 4096)
            if not chunk:
                continue
            buf += chunk.decode("utf-8")
            m = re.search(r"listening on (http://127\.0\.0\.1:\d+)", buf)
            if m:
                return m.group(1)
        raise RuntimeError(f"handler did not start in {timeout}s:\n{buf}")

    def close(self) -> None:
        self.proc.terminate()
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.kill()


# ---------------------------------------------------------------------------
# ICP tools exposed to Claude
# ---------------------------------------------------------------------------

def tool_definitions(merchant_aid: str, settler: str) -> list[dict]:
    """Anthropic SDK tool definitions for the 7 ICP verbs."""
    return [
        {
            "name": "icp_capabilities",
            "description": "Discover what the ICP merchant supports — spec version, allowed Settlers, supported verbs. Call this first.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": False},
        },
        {
            "name": "icp_inventory",
            "description": "Read-only query for inventory availability. Returns a signed InventorySnapshot the buyer can use to decide.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "skus": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "sku": {"type": "string"},
                                "quantity": {"type": "integer"},
                            },
                            "required": ["sku"],
                        },
                    },
                    "in_stock_only": {"type": "boolean"},
                },
            },
        },
        {
            "name": "icp_purchase",
            "description": "Submit a signed purchase Intent. Returns a signed Quote which the buyer should then accept.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "sku": {"type": "string"},
                                "quantity": {"type": "integer"},
                                "unit_price_amount": {"type": "string"},
                                "unit_price_currency": {"type": "string"},
                            },
                            "required": ["sku", "quantity", "unit_price_amount", "unit_price_currency"],
                        },
                    },
                    "max_total_amount": {"type": "string"},
                    "max_total_currency": {"type": "string"},
                },
                "required": ["items", "max_total_amount", "max_total_currency"],
            },
        },
        {
            "name": "icp_accept",
            "description": "Accept a Quote and receive on-chain funding instructions for the escrow.",
            "input_schema": {
                "type": "object",
                "properties": {"quote_id": {"type": "string"}},
                "required": ["quote_id"],
            },
        },
        {
            "name": "icp_observe",
            "description": "Inspect the current state of an escrow.",
            "input_schema": {
                "type": "object",
                "properties": {"escrow_id": {"type": "string"}},
                "required": ["escrow_id"],
            },
        },
    ]


class ICPToolDispatcher:
    """Routes Claude's tool_use calls to icp-client SDK methods."""

    def __init__(self, client: ICPClient, merchant_aid: str, settler: str):
        self.client = client
        self.merchant_aid = merchant_aid
        self.settler = settler
        # For demo simplicity, simulate escrow state by fulfilling immediately
        self.fulfill_url = client.handler_url
        self.last_escrow_id: str | None = None

    def call(self, name: str, args: dict) -> dict:
        if name == "icp_capabilities":
            return self.client.capabilities()
        if name == "icp_inventory":
            skus = args.get("skus") or None
            filters = {"in_stock_only": args["in_stock_only"]} if "in_stock_only" in args else None
            return self.client.inventory(self.merchant_aid, self.settler, skus=skus, filters=filters)
        if name == "icp_purchase":
            items = [
                {
                    "sku": it["sku"],
                    "quantity": it["quantity"],
                    "unit_price": {"amount": it["unit_price_amount"], "currency": it["unit_price_currency"]},
                }
                for it in args["items"]
            ]
            return self.client.purchase(
                self.merchant_aid,
                self.settler,
                items=items,
                max_total={"amount": args["max_total_amount"], "currency": args["max_total_currency"]},
            )
        if name == "icp_accept":
            result = self.client.accept(args["quote_id"])
            self.last_escrow_id = result.get("funding", {}).get("escrow_id")
            # Trigger the fulfillment path so we get a SettlementReceipt in the demo
            if self.last_escrow_id:
                import urllib.request
                req = urllib.request.Request(
                    f"{self.fulfill_url}/icp/v1/escrows/{self.last_escrow_id}/fulfill",
                    data=json.dumps({"evidence_id": "icp_ful_claude_demo"}).encode(),
                    headers={"Content-Type": "application/json"},
                    method="POST",
                )
                try:
                    with urllib.request.urlopen(req) as resp:
                        result["__settlement"] = json.loads(resp.read())
                except Exception as e:
                    result["__settlement_error"] = str(e)
            return result
        if name == "icp_observe":
            # Best-effort: read first event from the SSE stream then return
            events = []
            try:
                for ev in self.client.observe(args["escrow_id"]):
                    events.append(ev)
                    if len(events) >= 6:
                        break
            except Exception as e:
                return {"error": str(e)}
            return {"events": events}
        raise ValueError(f"unknown tool: {name}")


# ---------------------------------------------------------------------------
# Claude agent loop (real Anthropic SDK)
# ---------------------------------------------------------------------------

def run_with_claude(dispatcher: ICPToolDispatcher, tools: list[dict], prompt: str) -> str:
    """Send the prompt to Claude and execute its tool calls. Returns the full transcript."""
    import anthropic

    client = anthropic.Anthropic()
    messages = [{"role": "user", "content": prompt}]
    transcript = [f"## User\n\n{prompt}\n"]

    for turn in range(12):  # cap at 12 turns to prevent infinite loops
        response = client.messages.create(
            model=os.environ.get("ANTHROPIC_MODEL", "claude-sonnet-4-5"),
            max_tokens=4096,
            tools=tools,
            messages=messages,
        )

        # Print Claude's reasoning + capture
        assistant_blocks = []
        text_parts = []
        for block in response.content:
            if block.type == "text":
                text_parts.append(block.text)
                assistant_blocks.append({"type": "text", "text": block.text})
            elif block.type == "tool_use":
                assistant_blocks.append({
                    "type": "tool_use",
                    "id": block.id,
                    "name": block.name,
                    "input": block.input,
                })

        if text_parts:
            transcript.append(f"\n## Claude (turn {turn + 1})\n\n{''.join(text_parts).strip()}\n")

        # If no tool calls, Claude is done.
        tool_uses = [b for b in assistant_blocks if b.get("type") == "tool_use"]
        if not tool_uses:
            break

        messages.append({"role": "assistant", "content": assistant_blocks})

        # Execute each tool call.
        tool_results = []
        for tu in tool_uses:
            transcript.append(f"\n### Claude → {tu['name']}\n\n```json\n{json.dumps(tu['input'], indent=2)}\n```\n")
            try:
                result = dispatcher.call(tu["name"], tu["input"])
                snippet = json.dumps(result, indent=2)
                if len(snippet) > 1500:
                    snippet = snippet[:1500] + "\n…(truncated)"
                transcript.append(f"### {tu['name']} → result\n\n```json\n{snippet}\n```\n")
                tool_results.append({
                    "type": "tool_result",
                    "tool_use_id": tu["id"],
                    "content": json.dumps(result),
                })
            except ICPError as err:
                err_payload = {"icp_error_code": err.code, "message": str(err)}
                transcript.append(f"### {tu['name']} → ICP error\n\n```json\n{json.dumps(err_payload)}\n```\n")
                tool_results.append({
                    "type": "tool_result",
                    "tool_use_id": tu["id"],
                    "content": json.dumps(err_payload),
                    "is_error": True,
                })

        messages.append({"role": "user", "content": tool_results})

        if response.stop_reason == "end_turn":
            break

    return "\n".join(transcript)


# ---------------------------------------------------------------------------
# Deterministic agent simulator (no API key required)
# ---------------------------------------------------------------------------

def run_simulated(dispatcher: ICPToolDispatcher) -> str:
    """Deterministic walk through the ICP flow without an LLM. Useful for CI."""
    transcript = ["## Simulated agent (no ANTHROPIC_API_KEY set)\n\n"
                  "Walking through the same tool sequence Claude would call,\n"
                  "deterministically. Set ANTHROPIC_API_KEY to use real Claude reasoning.\n"]

    # Step 1: discover capabilities
    transcript.append("\n### Agent → icp_capabilities\n")
    caps = dispatcher.call("icp_capabilities", {})
    transcript.append(f"\n```json\n{json.dumps({'merchant_aid': caps['merchant_aid'], 'verbs': caps['capabilities']['verbs']}, indent=2)}\n```\n")

    # Step 2: browse inventory
    transcript.append("\n### Agent → icp_inventory\n")
    inv = dispatcher.call("icp_inventory", {"skus": [{"sku": "WIDGET-001", "quantity": 2}]})
    items = inv["snapshot"]["items"]
    transcript.append(f"\n```json\n{json.dumps(items[:3], indent=2)}\n```\n")

    # Step 3: purchase
    transcript.append("\n### Agent → icp_purchase\n")
    purchase_args = {
        "items": [{"sku": "WIDGET-001", "quantity": 2, "unit_price_amount": "29.99", "unit_price_currency": "USDC"}],
        "max_total_amount": "70.00",
        "max_total_currency": "USDC",
    }
    purchase = dispatcher.call("icp_purchase", purchase_args)
    transcript.append(f"\n```json\n{json.dumps({'quote_total': purchase['quote']['total'], 'quote_id': purchase['quote']['quote_id']}, indent=2)}\n```\n")

    # Step 4: accept
    transcript.append("\n### Agent → icp_accept\n")
    accepted = dispatcher.call("icp_accept", {"quote_id": purchase["quote"]["quote_id"]})
    transcript.append(f"\n```json\n{json.dumps({'escrow_id': accepted['funding']['escrow_id'], 'has_settlement': '__settlement' in accepted}, indent=2)}\n```\n")

    if "__settlement" in accepted:
        receipt = accepted["__settlement"].get("receipt", {})
        transcript.append("\n### Final SettlementReceipt (co-signed)\n")
        transcript.append(f"\n```json\n{json.dumps({'settlement_id': receipt.get('settlement_id'), 'amount': receipt.get('amount'), 'final_state': receipt.get('final_state')}, indent=2)}\n```\n")

    transcript.append("\n## Summary\n\nFull ICP flow completed via 4 tool calls. Set ANTHROPIC_API_KEY to see Claude make these decisions autonomously.\n")
    return "\n".join(transcript)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

PROMPT = """You're an autonomous purchasing agent. Use the icp_* tools to buy 2 widgets
(SKU: WIDGET-001) at no more than $35 each. Settle in USDC on Base Sepolia.

Walk through these steps:
1. Discover the merchant via icp_capabilities.
2. Browse inventory via icp_inventory to confirm the price.
3. Submit a signed purchase Intent via icp_purchase with a sensible max_total.
4. Accept the Quote via icp_accept and report the SettlementReceipt that follows.

Be concise. After the purchase completes, summarize what happened."""


def main():
    handler = HandlerProc()
    try:
        client = ICPClient.create(
            handler_url=handler.base_url,
            principal="did:web:claude-agent-demo.example",
        )
        caps = client.capabilities()
        dispatcher = ICPToolDispatcher(client, caps["merchant_aid"], "settler:stateset.usdc.base-sepolia")
        tools = tool_definitions(caps["merchant_aid"], "settler:stateset.usdc.base-sepolia")

        if os.environ.get("ANTHROPIC_API_KEY"):
            transcript = run_with_claude(dispatcher, tools, PROMPT)
        else:
            transcript = run_simulated(dispatcher)

        out_path = Path(__file__).resolve().parent / "transcript.md"
        header = f"# ICP × Claude Agent Demo\n\n*Generated by `{Path(__file__).name}`*\n\n"
        out_path.write_text(header + transcript)
        print(transcript)
        print(f"\n✓ Transcript written to {out_path}")
    finally:
        handler.close()


if __name__ == "__main__":
    main()
