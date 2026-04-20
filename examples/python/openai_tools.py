#!/usr/bin/env python3
"""OpenAI-compatible embedded commerce example."""

from __future__ import annotations

import json

from embedded_runtime import create_demo_commerce, emit_summary, load_openai_helpers


def main() -> None:
    commerce, source = create_demo_commerce()
    create_openai_tools, execute_openai_tool_call, execute_openai_tool_calls = load_openai_helpers()
    tools = create_openai_tools(commerce, filter=["list_customers", "count_customers"])
    execution = execute_openai_tool_call(
        commerce,
        {
            "call_id": "py_openai_1",
            "function": {
                "name": "list_customers",
                "arguments": json.dumps({"limit": 1}),
            },
        },
    )
    batch = execute_openai_tool_calls(
        commerce,
        [
            {
                "call_id": "py_openai_2",
                "function": {
                    "name": "count_customers",
                    "arguments": "{}",
                },
            }
        ],
    )

    summary = {
        "runtimeSource": source,
        "surface": "openai",
        "tools": [tool["function"]["name"] for tool in tools],
        "status": execution["result"]["status"],
        "batchStatus": batch[0]["result"]["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Surface: OpenAI-compatible",
            f"Tools: {[tool['function']['name'] for tool in tools]}",
            f"First call status: {execution['result']['status']}",
            f"Batch call status: {batch[0]['result']['status']}",
        ],
    )


if __name__ == "__main__":
    main()
