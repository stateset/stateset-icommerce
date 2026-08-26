"""Framework-neutral helper functions for the embedded commerce toolkit."""

from __future__ import annotations

from typing import Any, Callable, Dict, List, Mapping, Optional, Sequence, Union

from .agent_toolkit import AgentToolDescriptor, EmbeddedAgentToolkit, create_embedded_agent_toolkit
from .stateset_embedded import Commerce


ToolkitTarget = Union[Commerce, EmbeddedAgentToolkit]


def _resolve_toolkit(
    commerce_or_toolkit: ToolkitTarget,
    allow_apply: bool,
    capabilities: Optional[Sequence[str]] = None,
) -> EmbeddedAgentToolkit:
    if isinstance(commerce_or_toolkit, EmbeddedAgentToolkit):
        return commerce_or_toolkit
    return create_embedded_agent_toolkit(
        commerce_or_toolkit, allow_apply=allow_apply, capabilities=capabilities
    )


def create_tool_descriptors(
    commerce_or_toolkit: ToolkitTarget,
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[AgentToolDescriptor]:
    """Create framework-neutral executable descriptors from a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply, capabilities)
    return toolkit.create_tool_descriptors(filter=filter)


def create_callable_registry(
    commerce_or_toolkit: ToolkitTarget,
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> Dict[str, Callable[[Optional[Mapping[str, Any]]], Dict[str, Any]]]:
    """Create a callable tool registry from a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply, capabilities)
    return toolkit.create_callable_registry(filter=filter)


def execute_tool(
    commerce_or_toolkit: ToolkitTarget,
    tool_name: str,
    params: Optional[Mapping[str, Any]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> Dict[str, Any]:
    """Execute a single tool against a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply, capabilities)
    return toolkit.execute_tool(tool_name, params)


def execute_tool_calls(
    commerce_or_toolkit: ToolkitTarget,
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[Mapping[str, Any]]:
    """Execute multiple generic tool calls against a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply, capabilities)
    return toolkit.execute_tool_calls(tool_calls)


__all__ = [
    "create_tool_descriptors",
    "create_callable_registry",
    "execute_tool",
    "execute_tool_calls",
]
