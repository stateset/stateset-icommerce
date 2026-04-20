#!/usr/bin/env python3
"""Minimal Python agent-toolkit example for embedded commerce."""

from __future__ import annotations

import json

from embedded_runtime import emit_summary, load_stateset_embedded


def main() -> None:
    commerce_cls, toolkit_factory, _, _, _, source = load_stateset_embedded()
    commerce = commerce_cls(":memory:")
    commerce.customers.create(
        email="alice@example.com",
        first_name="Alice",
        last_name="Agent",
    )

    toolkit = toolkit_factory(commerce, allow_apply=False)
    openai_tools = toolkit.get_tools(format="openai")
    descriptors = toolkit.create_tool_descriptors(filter=["list_customers", "get_sales_summary"])

    execution = toolkit.execute_openai_tool_call(
        {
            "call_id": "py_demo_1",
            "function": {
                "name": "list_customers",
                "arguments": json.dumps({"limit": 5}),
            },
        }
    )
    summary = {
        "runtimeSource": source,
        "toolCount": len(openai_tools),
        "firstTool": openai_tools[0]["function"]["name"],
        "descriptorNames": [descriptor.name for descriptor in descriptors],
        "status": execution["result"]["status"],
        "outputMessageType": execution["output_message"]["type"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            f"OpenAI tool count: {len(openai_tools)}",
            f"First tool: {openai_tools[0]['function']['name']}",
            f"Descriptor names: {[descriptor.name for descriptor in descriptors]}",
            f"Tool status: {execution['result']['status']}",
            "Function call output:",
            json.dumps(execution["output_message"], indent=2),
        ],
    )


if __name__ == "__main__":
    main()
