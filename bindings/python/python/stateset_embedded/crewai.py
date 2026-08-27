"""CrewAI adapter helpers for the embedded commerce toolkit."""

from __future__ import annotations

from typing import Any, List, Optional, Sequence, Union

from .agent_toolkit import (
    EmbeddedAgentToolkit,
    FrameworkToolFactory,
    create_embedded_agent_toolkit,
)
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


def create_crewai_tools(
    commerce_or_toolkit: ToolkitTarget,
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    tool_factory: Optional[FrameworkToolFactory] = None,
) -> List[Any]:
    """Create CrewAI-compatible tools from a Commerce instance or toolkit."""

    toolkit = _resolve_toolkit(commerce_or_toolkit, allow_apply, capabilities)
    return toolkit.create_crewai_tools(filter=filter, tool_factory=tool_factory)


__all__ = ["create_crewai_tools"]
