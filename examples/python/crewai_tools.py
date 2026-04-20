#!/usr/bin/env python3
"""CrewAI-oriented embedded commerce example."""

from __future__ import annotations

from embedded_runtime import (
    DemoCrewAITool,
    create_demo_commerce,
    emit_summary,
    load_stateset_embedded,
)


def main() -> None:
    commerce, source = create_demo_commerce()
    _, _, _, create_crewai_tools, _, _ = load_stateset_embedded()
    crewai_tools = create_crewai_tools(
        commerce,
        filter=["count_customers", "create_customer"],
        tool_factory=DemoCrewAITool,
    )

    count_result = crewai_tools[0].run({})
    preview_result = crewai_tools[1].run(
        {
            "email": "preview@example.com",
            "first_name": "Preview",
            "last_name": "Only",
        }
    )

    summary = {
        "runtimeSource": source,
        "framework": "crewai",
        "tools": [tool.name for tool in crewai_tools],
        "status": count_result["status"],
        "writeStatus": preview_result["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Framework: CrewAI",
            f"Tools: {[tool.name for tool in crewai_tools]}",
            f"Count status: {count_result['status']}",
            f"Write mode: {preview_result['status']}",
        ],
    )


if __name__ == "__main__":
    main()
