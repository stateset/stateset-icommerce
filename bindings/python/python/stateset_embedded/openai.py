"""OpenAI-compatible helper functions for the embedded commerce toolkit."""

from __future__ import annotations

from typing import Any, List, Mapping, Optional, Sequence, Union

from .agent_toolkit import EmbeddedAgentToolkit, create_embedded_agent_toolkit
from .stateset_embedded import Commerce


ToolkitTarget = Union[Commerce, EmbeddedAgentToolkit]


def _resolve_toolkit(
    commerce_or_toolkit: ToolkitTarget,
    allow_apply: bool,
) -> EmbeddedAgentToolkit:
    if isinstance(commerce_or_toolkit, EmbeddedAgentToolkit):
        return commerce_or_toolkit
    return create_embedded_agent_toolkit(commerce_or_toolkit, allow_apply=allow_apply)


def create_openai_tools(
    commerce_or_toolkit: ToolkitTarget,
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
) -> List[Mapping[str, Any]]:
    """Create OpenAI-compatible function tools from a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply=allow_apply)
    return toolkit.get_tools(format="openai", filter=filter)


def execute_openai_tool_call(
    commerce_or_toolkit: ToolkitTarget,
    tool_call: Mapping[str, Any],
    allow_apply: bool = False,
) -> Mapping[str, Any]:
    """Execute an OpenAI-style tool call using a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply=allow_apply)
    return toolkit.execute_openai_tool_call(tool_call)


def execute_openai_tool_calls(
    commerce_or_toolkit: ToolkitTarget,
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
) -> List[Mapping[str, Any]]:
    """Execute multiple OpenAI-style tool calls using a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply=allow_apply)
    return [toolkit.execute_openai_tool_call(tool_call) for tool_call in tool_calls]


__all__ = [
    "create_openai_tools",
    "execute_openai_tool_call",
    "execute_openai_tool_calls",
]
