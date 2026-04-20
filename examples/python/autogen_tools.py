#!/usr/bin/env python3
"""AutoGen-oriented embedded commerce example."""

from __future__ import annotations

from embedded_runtime import (
    DemoAutoGenTool,
    create_demo_commerce,
    emit_summary,
    load_stateset_embedded,
)


def main() -> None:
    commerce, source = create_demo_commerce()
    _, _, _, _, create_autogen_tools, _ = load_stateset_embedded()
    autogen_tools = create_autogen_tools(
        commerce,
        filter=["get_sales_summary", "top_customers"],
        tool_factory=DemoAutoGenTool,
    )

    summary_result = autogen_tools[0].call({"period": "last30days"})
    customers_result = autogen_tools[1].call({"limit": 5})

    summary = {
        "runtimeSource": source,
        "framework": "autogen",
        "tools": [tool.name for tool in autogen_tools],
        "status": summary_result["status"],
        "secondaryStatus": customers_result["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Framework: AutoGen",
            f"Tools: {[tool.name for tool in autogen_tools]}",
            f"Summary status: {summary_result['status']}",
            f"Top customers status: {customers_result['status']}",
        ],
    )


if __name__ == "__main__":
    main()
