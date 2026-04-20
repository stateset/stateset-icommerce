#!/usr/bin/env python3
"""Python framework adapter example for LangChain, CrewAI, and AutoGen-style hosts."""

from __future__ import annotations

from embedded_runtime import (
    DemoAutoGenTool,
    DemoCrewAITool,
    DemoStructuredTool,
    emit_summary,
    load_stateset_embedded,
)


def main() -> None:
    (
        commerce_cls,
        toolkit_factory,
        create_langchain_tools,
        create_crewai_tools,
        create_autogen_tools,
        source,
    ) = load_stateset_embedded()
    commerce = commerce_cls(":memory:")
    commerce.customers.create(
        email="alice@example.com",
        first_name="Alice",
        last_name="Adapter",
    )

    toolkit = toolkit_factory(commerce, allow_apply=False)
    callable_registry = toolkit.create_callable_registry(filter=["list_customers"])
    langchain_tools = create_langchain_tools(
        commerce,
        filter=["list_customers"],
        tool_factory=lambda descriptor: DemoStructuredTool.from_function(
            func=lambda **kwargs: descriptor.execute(kwargs),
            name=descriptor.name,
            description=descriptor.description,
            args_schema=descriptor.input_schema,
        ),
    )
    crewai_tools = create_crewai_tools(
        commerce,
        filter=["count_customers"],
        tool_factory=DemoCrewAITool,
    )
    autogen_tools = create_autogen_tools(
        commerce,
        filter=["get_sales_summary"],
        tool_factory=DemoAutoGenTool,
    )

    result = callable_registry["list_customers"]({"limit": 1})
    summary = {
        "runtimeSource": source,
        "frameworks": ["langchain", "crewai", "autogen"],
        "registryKeys": sorted(callable_registry.keys()),
        "langchainTool": langchain_tools[0].name,
        "crewaiTool": crewai_tools[0].name,
        "autogenTool": autogen_tools[0].name,
        "status": result["status"],
    }
    emit_summary(
        summary,
        [
            f"Runtime source: {source}",
            "Adapter modules: stateset_embedded.langchain / crewai / autogen",
            f"Callable registry: {sorted(callable_registry.keys())}",
            f"LangChain tool: {langchain_tools[0].name}",
            f"CrewAI tool: {crewai_tools[0].name}",
            f"AutoGen tool: {autogen_tools[0].name}",
            f"Registry tool status: {result['status']}",
        ],
    )


if __name__ == "__main__":
    main()
