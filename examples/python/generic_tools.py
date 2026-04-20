#!/usr/bin/env python3
"""Framework-neutral embedded commerce example."""

from __future__ import annotations

from embedded_runtime import create_demo_commerce, emit_summary, load_generic_helpers


def main() -> None:
    commerce, source = create_demo_commerce()
    create_tool_descriptors, create_callable_registry, execute_tool, execute_tool_calls = (
        load_generic_helpers()
    )
    descriptors = create_tool_descriptors(
        commerce,
        filter=["list_customers", "get_sales_summary"],
    )
    registry = create_callable_registry(commerce, filter=["list_customers"])
    result = execute_tool(commerce, "list_customers", {"limit": 1})
    batch = execute_tool_calls(
        commerce,
        [{"name": "count_customers", "arguments": {}}],
    )

    summary = {
        "runtimeSource": source,
        "surface": "generic",
        "descriptors": [descriptor.name for descriptor in descriptors],
        "registryKeys": sorted(registry.keys()),
        "status": result["status"],
        "batchStatus": batch[0]["result"]["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Surface: Framework-neutral",
            f"Descriptors: {[descriptor.name for descriptor in descriptors]}",
            f"Registry keys: {sorted(registry.keys())}",
            f"Single call status: {result['status']}",
            f"Batch call status: {batch[0]['result']['status']}",
        ],
    )


if __name__ == "__main__":
    main()
