#!/usr/bin/env python3
"""LangChain-oriented embedded commerce example."""

from __future__ import annotations

from embedded_runtime import (
    DemoStructuredTool,
    create_demo_commerce,
    emit_summary,
    load_stateset_embedded,
)


def main() -> None:
    commerce, source = create_demo_commerce()
    _, _, create_langchain_tools, _, _, _ = load_stateset_embedded()
    langchain_tools = create_langchain_tools(
        commerce,
        filter=["list_customers", "get_sales_summary"],
        tool_factory=lambda descriptor: DemoStructuredTool.from_function(
            func=lambda **kwargs: descriptor.execute(kwargs),
            name=descriptor.name,
            description=descriptor.description,
            args_schema=descriptor.input_schema,
        ),
    )

    first_result = langchain_tools[0].runner(limit=1)

    summary = {
        "runtimeSource": source,
        "framework": "langchain",
        "tools": [tool.name for tool in langchain_tools],
        "status": first_result["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Framework: LangChain",
            f"Tools: {[tool.name for tool in langchain_tools]}",
            f"First tool status: {first_result['status']}",
        ],
    )


if __name__ == "__main__":
    main()
