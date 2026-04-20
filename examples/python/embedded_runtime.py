#!/usr/bin/env python3
"""Shared runtime helpers for embedded Python agent examples."""

from __future__ import annotations

import json
import os
import pathlib
import sys
from typing import Any, Callable, Iterable, Mapping, Tuple


def is_quiet() -> bool:
    return os.environ.get("STATESET_TOOLKIT_QUIET") == "1"


def output_mode() -> str:
    return os.environ.get("STATESET_TOOLKIT_OUTPUT", "text")


def emit_summary(summary: Mapping[str, Any], lines: Iterable[str]) -> None:
    if output_mode() == "json":
        print(json.dumps(summary))
        return

    if not is_quiet():
        for line in lines:
            print(line)


def load_stateset_embedded() -> Tuple[Any, Any, Any, Any, Any, str]:
    try:
        from stateset_embedded import Commerce, create_embedded_agent_toolkit
        from stateset_embedded.autogen import create_autogen_tools
        from stateset_embedded.crewai import create_crewai_tools
        from stateset_embedded.langchain import create_langchain_tools

        return (
            Commerce,
            create_embedded_agent_toolkit,
            create_langchain_tools,
            create_crewai_tools,
            create_autogen_tools,
            "package",
        )
    except ImportError:
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        sys.path.insert(0, str(repo_root / "bindings/python/python"))
        from stateset_embedded import Commerce, create_embedded_agent_toolkit
        from stateset_embedded.autogen import create_autogen_tools
        from stateset_embedded.crewai import create_crewai_tools
        from stateset_embedded.langchain import create_langchain_tools

        return (
            Commerce,
            create_embedded_agent_toolkit,
            create_langchain_tools,
            create_crewai_tools,
            create_autogen_tools,
            "workspace",
        )


def load_openai_helpers() -> Tuple[Any, Any, Any]:
    try:
        from stateset_embedded.openai import (
            create_openai_tools,
            execute_openai_tool_call,
            execute_openai_tool_calls,
        )

        return create_openai_tools, execute_openai_tool_call, execute_openai_tool_calls
    except ImportError:
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        sys.path.insert(0, str(repo_root / "bindings/python/python"))
        from stateset_embedded.openai import (
            create_openai_tools,
            execute_openai_tool_call,
            execute_openai_tool_calls,
        )

        return create_openai_tools, execute_openai_tool_call, execute_openai_tool_calls


def load_generic_helpers() -> Tuple[Any, Any, Any, Any]:
    try:
        from stateset_embedded.generic import (
            create_callable_registry,
            create_tool_descriptors,
            execute_tool,
            execute_tool_calls,
        )

        return create_tool_descriptors, create_callable_registry, execute_tool, execute_tool_calls
    except ImportError:
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        sys.path.insert(0, str(repo_root / "bindings/python/python"))
        from stateset_embedded.generic import (
            create_callable_registry,
            create_tool_descriptors,
            execute_tool,
            execute_tool_calls,
        )

        return create_tool_descriptors, create_callable_registry, execute_tool, execute_tool_calls


def create_demo_commerce() -> Tuple[Any, str]:
    commerce_cls, _, _, _, _, source = load_stateset_embedded()
    commerce = commerce_cls(":memory:")
    commerce.customers.create(
        email="alice@example.com",
        first_name="Alice",
        last_name="Adapter",
    )
    return commerce, source


class DemoStructuredTool:
    def __init__(self, name: str, description: str, runner: Callable[..., Any], args_schema: Any):
        self.name = name
        self.description = description
        self.runner = runner
        self.args_schema = args_schema

    @classmethod
    def from_function(
        cls,
        func: Callable[..., Any],
        name: str,
        description: str,
        args_schema: Any,
    ) -> "DemoStructuredTool":
        return cls(name=name, description=description, runner=func, args_schema=args_schema)


class DemoCrewAITool:
    def __init__(self, descriptor: Any):
        self.name = descriptor.name
        self.description = descriptor.description
        self.run = descriptor.execute


class DemoAutoGenTool:
    def __init__(self, descriptor: Any):
        self.name = descriptor.name
        self.description = descriptor.description
        self.call = descriptor.execute
