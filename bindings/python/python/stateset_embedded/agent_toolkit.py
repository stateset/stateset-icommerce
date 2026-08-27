"""Minimal agent toolkit for Python agent runtimes embedding StateSet commerce."""

from __future__ import annotations

import json
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Dict, List, Mapping, Optional, Sequence

from .stateset_embedded import Commerce, CreateOrderItemInput, CreateProductVariantInput


JsonDict = Dict[str, Any]
ToolHandler = Callable[[Mapping[str, Any]], Any]
FrameworkToolFactory = Callable[["AgentToolDescriptor"], Any]


def _object_schema(
    properties: Mapping[str, JsonDict],
    required: Optional[Sequence[str]] = None,
) -> JsonDict:
    schema: JsonDict = {
        "type": "object",
        "properties": dict(properties),
        "additionalProperties": False,
    }
    if required:
        schema["required"] = list(required)
    return schema


def _string_schema(description: str) -> JsonDict:
    return {"type": "string", "description": description}


def _number_schema(description: str) -> JsonDict:
    return {"type": "number", "description": description}


def _integer_schema(description: str) -> JsonDict:
    return {"type": "integer", "description": description}


def _boolean_schema(description: str) -> JsonDict:
    return {"type": "boolean", "description": description}


def _normalize_tool_name(tool_name: str) -> str:
    normalized = str(tool_name or "").strip()
    if normalized.startswith("mcp__") and "__" in normalized:
        return normalized.split("__", 2)[-1]
    return normalized


def _parse_tool_arguments(arguments: Any) -> JsonDict:
    if arguments is None or arguments == "":
        return {}
    if isinstance(arguments, str):
        parsed = json.loads(arguments)
        if not isinstance(parsed, dict):
            raise ValueError("Tool arguments JSON must decode to an object.")
        return parsed
    if isinstance(arguments, Mapping):
        return dict(arguments)
    raise TypeError("Tool arguments must be a mapping or JSON string.")


def _serialize_value(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, list):
        return [_serialize_value(item) for item in value]
    if isinstance(value, tuple):
        return [_serialize_value(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _serialize_value(item) for key, item in value.items()}

    attributes: JsonDict = {}
    for name in dir(value):
        if name.startswith("_"):
            continue
        try:
            attribute = getattr(value, name)
        except Exception:
            continue
        if callable(attribute):
            continue
        attributes[name] = _serialize_value(attribute)
    return attributes or str(value)


def _pascal_case(value: str) -> str:
    parts = [part for part in str(value).replace("-", "_").split("_") if part]
    return "".join(part[:1].upper() + part[1:] for part in parts) or "Tool"


def _normalize_json_schema_type(schema: Mapping[str, Any]) -> Optional[str]:
    schema_type = schema.get("type")
    if isinstance(schema_type, str):
        return schema_type
    if isinstance(schema_type, list):
        non_null = [item for item in schema_type if item != "null"]
        return str(non_null[0]) if non_null else None
    return None


def _json_schema_to_python_type(schema: Mapping[str, Any]) -> Any:
    schema_type = _normalize_json_schema_type(schema)
    if schema_type == "string":
        return str
    if schema_type == "integer":
        return int
    if schema_type == "number":
        return float
    if schema_type == "boolean":
        return bool
    if schema_type == "array":
        item_type = _json_schema_to_python_type(schema.get("items", {}))
        return List[item_type]
    if schema_type == "object":
        return Dict[str, Any]
    return Any


def _create_pydantic_model(model_name: str, schema: Mapping[str, Any]) -> Any:
    try:
        from pydantic import Field, create_model
    except ImportError as exc:
        raise RuntimeError(
            "pydantic is required to build framework adapter schemas.",
        ) from exc

    properties = schema.get("properties", {})
    required = set(schema.get("required", []))
    fields: Dict[str, Any] = {}

    for key, property_schema in properties.items():
        field_type = _json_schema_to_python_type(property_schema)
        description = property_schema.get("description")
        if key in required:
            default_value = ...
            annotated_type = field_type
        else:
            default_value = None
            annotated_type = Optional[field_type]

        if description is None:
            fields[key] = (annotated_type, default_value)
        else:
            fields[key] = (annotated_type, Field(default_value, description=description))

    return create_model(model_name, **fields)


def _build_framework_callable(descriptor: "AgentToolDescriptor") -> Callable[..., Any]:
    def _run(**kwargs: Any) -> Any:
        return json.dumps(descriptor.execute(kwargs))

    _run.__name__ = descriptor.name
    _run.__doc__ = descriptor.description
    return _run


def _default_langchain_tool_factory(descriptor: "AgentToolDescriptor") -> Any:
    try:
        from langchain_core.tools import StructuredTool
    except ImportError as exc:
        raise RuntimeError(
            "langchain-core is required for create_langchain_tools() without a custom tool_factory.",
        ) from exc

    args_schema = _create_pydantic_model(
        f"{_pascal_case(descriptor.name)}LangChainArgs",
        descriptor.input_schema,
    )
    return StructuredTool.from_function(
        func=_build_framework_callable(descriptor),
        name=descriptor.name,
        description=descriptor.description,
        args_schema=args_schema,
    )


def _default_crewai_tool_factory(descriptor: "AgentToolDescriptor") -> Any:
    try:
        from crewai.tools import BaseTool
    except ImportError as exc:
        raise RuntimeError(
            "crewai is required for create_crewai_tools() without a custom tool_factory.",
        ) from exc

    args_schema = _create_pydantic_model(
        f"{_pascal_case(descriptor.name)}CrewAIArgs",
        descriptor.input_schema,
    )

    def _run(self: Any, **kwargs: Any) -> Any:
        return json.dumps(descriptor.execute(kwargs))

    tool_cls = type(
        f"{_pascal_case(descriptor.name)}CrewAITool",
        (BaseTool,),
        {
            "name": descriptor.name,
            "description": descriptor.description,
            "args_schema": args_schema,
            "_run": _run,
        },
    )
    return tool_cls()


def _default_autogen_tool_factory(descriptor: "AgentToolDescriptor") -> Any:
    try:
        from autogen_core.tools import FunctionTool
    except ImportError as exc:
        raise RuntimeError(
            "autogen-core is required for create_autogen_tools() without a custom tool_factory.",
        ) from exc

    return FunctionTool(_build_framework_callable(descriptor), description=descriptor.description)


@dataclass(frozen=True)
class _ToolSpec:
    name: str
    description: str
    input_schema: JsonDict
    side_effect: str
    handler: ToolHandler = field(repr=False)


@dataclass
class AgentToolDescriptor:
    """Framework-neutral tool descriptor for Python agent runtimes."""

    name: str
    description: str
    schema: JsonDict
    input_schema: JsonDict
    side_effect: str
    execute: Callable[[Optional[Mapping[str, Any]]], JsonDict] = field(repr=False)


class EmbeddedAgentToolkit:
    """Small, native Python toolkit for embedding core commerce tools in agents."""

    def __init__(
        self,
        commerce: Commerce,
        allow_apply: bool = False,
        capabilities: Optional[Sequence[str]] = None,
        kernel: Optional[Mapping[str, Any]] = None,
    ) -> None:
        self.commerce = commerce
        self.allow_apply = allow_apply
        normalized = {str(capability).strip() for capability in capabilities or [] if str(capability).strip()}
        if allow_apply and not normalized:
            raise ValueError(
                "allow_apply requires explicit capabilities (for example ['read:*', 'create_customer'])."
            )
        self.capabilities = normalized if capabilities is not None else None
        self.kernel = dict(kernel) if kernel is not None else None
        self._tool_specs = self._build_tool_specs()

    def _build_tool_specs(self) -> List[_ToolSpec]:
        order_item_schema = _object_schema(
            {
                "sku": _string_schema("Stock keeping unit for the line item."),
                "name": _string_schema("Display name for the line item."),
                "quantity": _integer_schema("Number of units for the line item."),
                "unit_price": _number_schema("Unit price for the line item."),
                "product_id": _string_schema("Optional product identifier."),
                "variant_id": _string_schema("Optional product variant identifier."),
            },
            required=["sku", "name", "quantity", "unit_price"],
        )
        variant_schema = _object_schema(
            {
                "sku": _string_schema("SKU for the product variant."),
                "name": _string_schema("Optional display name for the variant."),
                "price": _number_schema("Unit price for the variant."),
                "compare_at_price": _number_schema("Optional compare-at price."),
            },
            required=["sku", "price"],
        )

        return [
            _ToolSpec(
                name="execute_kernel_command",
                description=(
                    "Preview or apply a governed high-risk commerce command. "
                    "Policy, principal, and store scope are supplied by the trusted host."
                ),
                input_schema=_object_schema(
                    {
                        "command_type": _string_schema("Namespaced governed command type."),
                        "idempotency_key": _string_schema("Stable semantic retry key."),
                        "payload": {
                            "type": "object",
                            "description": "Typed payload for the selected command.",
                        },
                        "expected_version": _integer_schema(
                            "Optional expected aggregate version for optimistic concurrency."
                        ),
                    },
                    required=["command_type", "idempotency_key", "payload"],
                ),
                side_effect="write",
                handler=self._execute_kernel_command,
            ),
            _ToolSpec(
                name="list_customers",
                description="List customers stored in the local embedded commerce engine.",
                input_schema=_object_schema(
                    {"limit": _integer_schema("Optional maximum number of customers to return.")}
                ),
                side_effect="read",
                handler=self._list_customers,
            ),
            _ToolSpec(
                name="get_customer",
                description="Fetch a single customer by ID or email.",
                input_schema=_object_schema(
                    {
                        "id": _string_schema("Customer identifier."),
                        "email": _string_schema("Customer email address."),
                    }
                ),
                side_effect="read",
                handler=self._get_customer,
            ),
            _ToolSpec(
                name="count_customers",
                description="Count customers in the local embedded store.",
                input_schema=_object_schema({}),
                side_effect="read",
                handler=self._count_customers,
            ),
            _ToolSpec(
                name="create_customer",
                description="Create a new customer in the embedded commerce engine.",
                input_schema=_object_schema(
                    {
                        "email": _string_schema("Customer email address."),
                        "first_name": _string_schema("Customer first name."),
                        "last_name": _string_schema("Customer last name."),
                        "phone": _string_schema("Optional customer phone number."),
                        "accepts_marketing": _boolean_schema("Optional marketing opt-in."),
                    },
                    required=["email", "first_name", "last_name"],
                ),
                side_effect="write",
                handler=self._create_customer,
            ),
            _ToolSpec(
                name="list_orders",
                description="List orders in the embedded commerce engine.",
                input_schema=_object_schema(
                    {
                        "limit": _integer_schema("Optional maximum number of orders to return."),
                        "status": _string_schema("Optional order status filter applied client-side."),
                    }
                ),
                side_effect="read",
                handler=self._list_orders,
            ),
            _ToolSpec(
                name="get_order",
                description="Fetch a single order by ID.",
                input_schema=_object_schema(
                    {"id": _string_schema("Order identifier.")},
                    required=["id"],
                ),
                side_effect="read",
                handler=self._get_order,
            ),
            _ToolSpec(
                name="create_order",
                description="Create an order with line items in the embedded commerce engine.",
                input_schema=_object_schema(
                    {
                        "customer_id": _string_schema("Customer identifier for the order."),
                        "items": {
                            "type": "array",
                            "description": "Line items to attach to the order.",
                            "items": order_item_schema,
                        },
                        "currency": _string_schema("Optional ISO currency code."),
                        "notes": _string_schema("Optional operator notes."),
                    },
                    required=["customer_id", "items"],
                ),
                side_effect="write",
                handler=self._create_order,
            ),
            _ToolSpec(
                name="list_products",
                description="List products stored in the embedded commerce engine.",
                input_schema=_object_schema(
                    {"limit": _integer_schema("Optional maximum number of products to return.")}
                ),
                side_effect="read",
                handler=self._list_products,
            ),
            _ToolSpec(
                name="get_product",
                description="Fetch a single product by ID.",
                input_schema=_object_schema(
                    {"id": _string_schema("Product identifier.")},
                    required=["id"],
                ),
                side_effect="read",
                handler=self._get_product,
            ),
            _ToolSpec(
                name="create_product",
                description="Create a product with optional variants in the embedded commerce engine.",
                input_schema=_object_schema(
                    {
                        "name": _string_schema("Product name."),
                        "description": _string_schema("Optional product description."),
                        "variants": {
                            "type": "array",
                            "description": "Optional list of variants.",
                            "items": variant_schema,
                        },
                    },
                    required=["name"],
                ),
                side_effect="write",
                handler=self._create_product,
            ),
            _ToolSpec(
                name="get_variant_by_sku",
                description="Fetch a product variant by SKU.",
                input_schema=_object_schema(
                    {"sku": _string_schema("Product variant SKU.")},
                    required=["sku"],
                ),
                side_effect="read",
                handler=self._get_variant_by_sku,
            ),
            _ToolSpec(
                name="get_stock",
                description="Get stock levels for a SKU.",
                input_schema=_object_schema(
                    {"sku": _string_schema("Inventory SKU.")},
                    required=["sku"],
                ),
                side_effect="read",
                handler=self._get_stock,
            ),
            _ToolSpec(
                name="create_inventory_item",
                description="Create an inventory item for a SKU.",
                input_schema=_object_schema(
                    {
                        "sku": _string_schema("Inventory SKU."),
                        "name": _string_schema("Inventory item name."),
                        "description": _string_schema("Optional inventory description."),
                        "initial_quantity": _number_schema("Optional initial quantity."),
                        "reorder_point": _number_schema("Optional reorder threshold."),
                    },
                    required=["sku", "name"],
                ),
                side_effect="write",
                handler=self._create_inventory_item,
            ),
            _ToolSpec(
                name="adjust_inventory",
                description="Adjust inventory quantity for a SKU.",
                input_schema=_object_schema(
                    {
                        "sku": _string_schema("Inventory SKU."),
                        "quantity": _number_schema("Signed inventory delta."),
                        "reason": _string_schema("Reason for the adjustment."),
                    },
                    required=["sku", "quantity", "reason"],
                ),
                side_effect="write",
                handler=self._adjust_inventory,
            ),
            _ToolSpec(
                name="get_sales_summary",
                description="Get a sales summary for a time period.",
                input_schema=_object_schema(
                    {
                        "period": _string_schema("Optional time period such as today or last30days."),
                        "limit": _integer_schema("Optional limit parameter forwarded to analytics."),
                    }
                ),
                side_effect="read",
                handler=self._get_sales_summary,
            ),
            _ToolSpec(
                name="top_products",
                description="Get top products by revenue and units sold.",
                input_schema=_object_schema(
                    {
                        "period": _string_schema("Optional reporting period."),
                        "limit": _integer_schema("Optional number of products to return."),
                    }
                ),
                side_effect="read",
                handler=self._top_products,
            ),
            _ToolSpec(
                name="top_customers",
                description="Get top customers by spend and order count.",
                input_schema=_object_schema(
                    {
                        "period": _string_schema("Optional reporting period."),
                        "limit": _integer_schema("Optional number of customers to return."),
                    }
                ),
                side_effect="read",
                handler=self._top_customers,
            ),
        ]

    def _select_specs(self, filter: Optional[Sequence[str]] = None) -> List[_ToolSpec]:
        selected = {_normalize_tool_name(name) for name in filter} if filter else None
        return [
            spec
            for spec in self._tool_specs
            if (selected is None or spec.name in selected)
            and (spec.name != "execute_kernel_command" or self.kernel is not None)
            and self._capability_allows(spec)
        ]

    def _capability_allows(self, spec: _ToolSpec) -> bool:
        if self.capabilities is None:
            return True
        return bool(
            "*" in self.capabilities
            or spec.name in self.capabilities
            or f"permission:{spec.side_effect}" in self.capabilities
            or (spec.side_effect == "read" and "read:*" in self.capabilities)
        )

    def create_tool_descriptors(
        self,
        filter: Optional[Sequence[str]] = None,
    ) -> List[AgentToolDescriptor]:
        return [
            AgentToolDescriptor(
                name=spec.name,
                description=spec.description,
                schema=spec.input_schema,
                input_schema=spec.input_schema,
                side_effect=spec.side_effect,
                execute=lambda params=None, tool_name=spec.name: self.execute_tool(tool_name, params or {}),
            )
            for spec in self._select_specs(filter)
        ]

    def create_callable_registry(
        self,
        filter: Optional[Sequence[str]] = None,
    ) -> Dict[str, Callable[[Optional[Mapping[str, Any]]], JsonDict]]:
        return {
            descriptor.name: descriptor.execute
            for descriptor in self.create_tool_descriptors(filter=filter)
        }

    def _adapt_framework_tools(
        self,
        filter: Optional[Sequence[str]],
        tool_factory: FrameworkToolFactory,
    ) -> List[Any]:
        return [
            tool_factory(descriptor)
            for descriptor in self.create_tool_descriptors(filter=filter)
        ]

    def create_langchain_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[FrameworkToolFactory] = None,
    ) -> List[Any]:
        factory = tool_factory or _default_langchain_tool_factory
        return self._adapt_framework_tools(filter=filter, tool_factory=factory)

    def create_crewai_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[FrameworkToolFactory] = None,
    ) -> List[Any]:
        factory = tool_factory or _default_crewai_tool_factory
        return self._adapt_framework_tools(filter=filter, tool_factory=factory)

    def create_autogen_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[FrameworkToolFactory] = None,
    ) -> List[Any]:
        factory = tool_factory or _default_autogen_tool_factory
        return self._adapt_framework_tools(filter=filter, tool_factory=factory)

    def get_tools(
        self,
        format: str = "generic",
        filter: Optional[Sequence[str]] = None,
    ) -> List[JsonDict]:
        specs = self._select_specs(filter)
        if format == "generic":
            return [
                {
                    "name": spec.name,
                    "description": spec.description,
                    "input_schema": spec.input_schema,
                    "side_effect": spec.side_effect,
                }
                for spec in specs
            ]
        if format == "openai":
            return [
                {
                    "type": "function",
                    "function": {
                        "name": spec.name,
                        "description": spec.description,
                        "parameters": spec.input_schema,
                    },
                }
                for spec in specs
            ]
        raise ValueError(f"Unsupported tool format: {format}")

    def list_tools(
        self,
        format: str = "generic",
        filter: Optional[Sequence[str]] = None,
    ) -> List[JsonDict]:
        return self.get_tools(format=format, filter=filter)

    def get_tool(self, tool_name: str, format: str = "generic") -> Optional[JsonDict]:
        normalized_name = _normalize_tool_name(tool_name)
        for tool in self.get_tools(format=format):
            if format == "openai":
                if tool["function"]["name"] == normalized_name:
                    return tool
                continue
            if tool["name"] == normalized_name:
                return tool
        return None

    def execute_tool(self, tool_name: str, params: Optional[Mapping[str, Any]] = None) -> JsonDict:
        normalized_name = _normalize_tool_name(tool_name)
        arguments = dict(params or {})
        spec = next((tool for tool in self._tool_specs if tool.name == normalized_name), None)
        if spec is None:
            return {
                "success": False,
                "tool": normalized_name,
                "status": "error",
                "preview": False,
                "error": f"Unknown tool: {normalized_name}",
            }
        if not self._capability_allows(spec):
            return {
                "success": False,
                "tool": normalized_name,
                "status": "forbidden",
                "preview": False,
                "error": f"Tool '{normalized_name}' is outside this toolkit's capability scope.",
            }

        if (
            spec.side_effect == "write"
            and spec.name != "execute_kernel_command"
            and not self.allow_apply
        ):
            return {
                "success": True,
                "tool": spec.name,
                "status": "preview",
                "preview": True,
                "message": "Set allow_apply=True to execute write tools.",
                "would_execute": {
                    "tool": spec.name,
                    "params": _serialize_value(arguments),
                },
            }

        try:
            result = spec.handler(arguments)
            return {
                "success": True,
                "tool": spec.name,
                "status": "success",
                "preview": False,
                "result": _serialize_value(result),
            }
        except Exception as exc:
            return {
                "success": False,
                "tool": spec.name,
                "status": "error",
                "preview": False,
                "error": str(exc),
            }

    def execute_tool_calls(self, tool_calls: Sequence[Mapping[str, Any]]) -> List[JsonDict]:
        results: List[JsonDict] = []
        for tool_call in tool_calls:
            if "function" in tool_call:
                results.append(self.execute_openai_tool_call(tool_call))
                continue
            tool_name = tool_call.get("name") or tool_call.get("tool") or ""
            params = tool_call.get("arguments") or tool_call.get("params") or {}
            parsed_params = _parse_tool_arguments(params)
            results.append(
                {
                    "call_id": tool_call.get("call_id") or tool_call.get("id"),
                    "name": _normalize_tool_name(str(tool_name)),
                    "arguments": parsed_params,
                    "result": self.execute_tool(str(tool_name), parsed_params),
                }
            )
        return results

    def execute_openai_tool_call(self, tool_call: Mapping[str, Any]) -> JsonDict:
        function_payload = tool_call.get("function") or tool_call
        tool_name = function_payload.get("name") or tool_call.get("name") or ""
        arguments = _parse_tool_arguments(function_payload.get("arguments"))
        call_id = tool_call.get("call_id") or tool_call.get("id")
        result = self.execute_tool(str(tool_name), arguments)
        output_message = None
        if call_id:
            output_message = {
                "type": "function_call_output",
                "call_id": call_id,
                "output": json.dumps(result),
            }
        return {
            "call_id": call_id,
            "name": _normalize_tool_name(str(tool_name)),
            "arguments": arguments,
            "result": result,
            "output_message": output_message,
        }

    def _list_customers(self, params: Mapping[str, Any]) -> JsonDict:
        customers = self.commerce.customers.list()
        limit = params.get("limit")
        if limit is not None:
            customers = customers[: int(limit)]
        return {"count": len(customers), "customers": customers}

    def _execute_kernel_command(self, params: Mapping[str, Any]) -> JsonDict:
        if self.kernel is None:
            raise ValueError(
                "execute_kernel_command requires trusted kernel configuration on the toolkit."
            )
        policy = self.kernel.get("policy")
        principal = self.kernel.get("principal")
        store_id = self.kernel.get("store_id") or self.kernel.get("storeId")
        if not isinstance(policy, Mapping):
            raise ValueError("kernel.policy must be trusted host policy configuration.")
        if not isinstance(principal, Mapping):
            raise ValueError("kernel.principal must be trusted host identity configuration.")
        if not store_id:
            raise ValueError("kernel.store_id is required.")

        command_type = str(params["command_type"])
        if self.capabilities is not None and not bool(
            "*" in self.capabilities or command_type in self.capabilities
        ):
            raise PermissionError(
                f"Kernel command '{command_type}' is outside this toolkit's capability scope."
            )
        now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        command: JsonDict = {
            "contract_version": "1.0",
            "command_id": str(uuid.uuid4()),
            "idempotency_key": str(params["idempotency_key"]),
            "command_type": command_type,
            "principal": {
                "id": principal.get("id"),
                "kind": principal.get("kind", "agent"),
                "tenant_id": principal.get("tenant_id") or principal.get("tenantId"),
                "delegated_by": principal.get("delegated_by") or principal.get("delegatedBy"),
                "capabilities": [str(value) for value in principal.get("capabilities", [])],
            },
            "store_id": str(store_id),
            "correlation_id": params.get("correlation_id"),
            "causation_id": params.get("causation_id"),
            "expected_version": params.get("expected_version"),
            "policy_version": policy.get("version"),
            "approval": None,
            "authority": None,
            "deadline": params.get("deadline"),
            "trace_id": params.get("trace_id"),
            "mode": "apply" if self.allow_apply else "preview",
            "payload": dict(params["payload"]),
            "issued_at": now,
        }
        approval = self.kernel.get("approval")
        command["approval"] = approval(command) if callable(approval) else approval
        authorize = self.kernel.get("authorize")
        command["authority"] = authorize(command) if callable(authorize) else None

        receipt_json = self.commerce.execute_kernel_command(
            json.dumps(command), json.dumps(dict(policy))
        )
        receipt = json.loads(receipt_json)
        return {
            "kernel": True,
            "command_type": command_type,
            "receipt": receipt,
            "result": receipt.get("result"),
        }

    def _get_customer(self, params: Mapping[str, Any]) -> Any:
        customer_id = params.get("id")
        email = params.get("email")
        if customer_id:
            return self.commerce.customers.get(str(customer_id))
        if email:
            return self.commerce.customers.get_by_email(str(email))
        raise ValueError("get_customer requires either id or email.")

    def _count_customers(self, _params: Mapping[str, Any]) -> JsonDict:
        return {"count": self.commerce.customers.count()}

    def _create_customer(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.customers.create(
            email=str(params["email"]),
            first_name=str(params["first_name"]),
            last_name=str(params["last_name"]),
            phone=params.get("phone"),
            accepts_marketing=params.get("accepts_marketing"),
        )

    def _list_orders(self, params: Mapping[str, Any]) -> JsonDict:
        orders = self.commerce.orders.list()
        status = params.get("status")
        if status:
            orders = [order for order in orders if str(order.status).lower() == str(status).lower()]
        limit = params.get("limit")
        if limit is not None:
            orders = orders[: int(limit)]
        return {"count": len(orders), "orders": orders}

    def _get_order(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.orders.get(str(params["id"]))

    def _create_order(self, params: Mapping[str, Any]) -> Any:
        raw_items = params.get("items")
        if not isinstance(raw_items, list) or len(raw_items) == 0:
            raise ValueError("create_order requires a non-empty items list.")
        items = [
            CreateOrderItemInput(
                sku=str(item["sku"]),
                name=str(item["name"]),
                quantity=int(item["quantity"]),
                unit_price=float(item["unit_price"]),
                product_id=item.get("product_id"),
                variant_id=item.get("variant_id"),
            )
            for item in raw_items
        ]
        return self.commerce.orders.create(
            customer_id=str(params["customer_id"]),
            items=items,
            currency=params.get("currency"),
            notes=params.get("notes"),
        )

    def _list_products(self, params: Mapping[str, Any]) -> JsonDict:
        products = self.commerce.products.list()
        limit = params.get("limit")
        if limit is not None:
            products = products[: int(limit)]
        return {"count": len(products), "products": products}

    def _get_product(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.products.get(str(params["id"]))

    def _create_product(self, params: Mapping[str, Any]) -> Any:
        raw_variants = params.get("variants")
        variants = None
        if raw_variants is not None:
            if not isinstance(raw_variants, list):
                raise ValueError("create_product variants must be a list when provided.")
            variants = [
                CreateProductVariantInput(
                    sku=str(variant["sku"]),
                    price=float(variant["price"]),
                    name=variant.get("name"),
                    compare_at_price=variant.get("compare_at_price"),
                )
                for variant in raw_variants
            ]
        return self.commerce.products.create(
            name=str(params["name"]),
            description=params.get("description"),
            variants=variants,
        )

    def _get_variant_by_sku(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.products.get_variant_by_sku(str(params["sku"]))

    def _get_stock(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.inventory.get_stock(str(params["sku"]))

    def _create_inventory_item(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.inventory.create_item(
            sku=str(params["sku"]),
            name=str(params["name"]),
            description=params.get("description"),
            initial_quantity=params.get("initial_quantity"),
            reorder_point=params.get("reorder_point"),
        )

    def _adjust_inventory(self, params: Mapping[str, Any]) -> JsonDict:
        self.commerce.inventory.adjust(
            str(params["sku"]),
            float(params["quantity"]),
            str(params["reason"]),
        )
        return {
            "sku": str(params["sku"]),
            "quantity": float(params["quantity"]),
            "reason": str(params["reason"]),
        }

    def _get_sales_summary(self, params: Mapping[str, Any]) -> Any:
        return self.commerce.analytics.sales_summary(
            period=params.get("period"),
            limit=params.get("limit"),
        )

    def _top_products(self, params: Mapping[str, Any]) -> JsonDict:
        products = self.commerce.analytics.top_products(
            period=params.get("period"),
            limit=params.get("limit"),
        )
        return {"count": len(products), "products": products}

    def _top_customers(self, params: Mapping[str, Any]) -> JsonDict:
        customers = self.commerce.analytics.top_customers(
            period=params.get("period"),
            limit=params.get("limit"),
        )
        return {"count": len(customers), "customers": customers}


def create_embedded_agent_toolkit(
    commerce: Commerce,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    kernel: Optional[Mapping[str, Any]] = None,
) -> EmbeddedAgentToolkit:
    """Create the native Python agent toolkit over an embedded Commerce instance."""

    return EmbeddedAgentToolkit(
        commerce=commerce,
        allow_apply=allow_apply,
        capabilities=capabilities,
        kernel=kernel,
    )
