"""Tests for the Python embedded agent toolkit."""

from stateset_embedded import (
    Commerce,
    create_callable_registry,
    create_autogen_tools,
    create_crewai_tools,
    create_embedded_agent_toolkit,
    create_langchain_tools,
    create_openai_tools,
    create_tool_descriptors,
    execute_tool,
    execute_tool_calls,
    execute_openai_tool_call,
    execute_openai_tool_calls,
)
from stateset_embedded.autogen import create_autogen_tools as create_autogen_module_tools
from stateset_embedded.crewai import create_crewai_tools as create_crewai_module_tools
from stateset_embedded.generic import (
    create_callable_registry as create_generic_callable_registry,
    create_tool_descriptors as create_generic_tool_descriptors,
    execute_tool as execute_generic_tool,
)
from stateset_embedded.langchain import create_langchain_tools as create_langchain_module_tools
from stateset_embedded.openai import (
    create_openai_tools as create_openai_module_tools,
    execute_openai_tool_call as execute_openai_module_tool_call,
)


def test_get_openai_tools_exposes_core_tooling():
    toolkit = create_embedded_agent_toolkit(Commerce(":memory:"))

    tools = toolkit.get_tools(format="openai")

    assert len(tools) >= 10
    assert tools[0]["type"] == "function"
    names = [tool["function"]["name"] for tool in tools]
    assert "list_customers" in names
    assert "create_order" in names


def test_read_tool_executes_successfully():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="alice@example.com",
        first_name="Alice",
        last_name="Agent",
    )
    toolkit = create_embedded_agent_toolkit(commerce)

    result = toolkit.execute_tool("list_customers", {"limit": 1})

    assert result["success"] is True
    assert result["status"] == "success"
    assert result["result"]["count"] == 1
    assert result["result"]["customers"][0]["email"] == "alice@example.com"


def test_write_tool_previews_when_apply_disabled():
    toolkit = create_embedded_agent_toolkit(Commerce(":memory:"), allow_apply=False)

    result = toolkit.execute_tool(
        "create_customer",
        {
            "email": "preview@example.com",
            "first_name": "Preview",
            "last_name": "User",
        },
    )

    assert result["success"] is True
    assert result["status"] == "preview"
    assert result["preview"] is True
    assert result["would_execute"]["tool"] == "create_customer"


def test_write_tool_executes_when_apply_enabled():
    commerce = Commerce(":memory:")
    toolkit = create_embedded_agent_toolkit(
        commerce,
        allow_apply=True,
        capabilities=["read:*", "create_customer"],
    )

    result = toolkit.execute_tool(
        "create_customer",
        {
            "email": "write@example.com",
            "first_name": "Write",
            "last_name": "Enabled",
        },
    )

    assert result["success"] is True
    assert result["status"] == "success"
    assert result["result"]["email"] == "write@example.com"
    assert commerce.customers.count() == 1


def test_apply_toolkit_requires_and_enforces_capability_scope():
    commerce = Commerce(":memory:")
    try:
        create_embedded_agent_toolkit(commerce, allow_apply=True)
        assert False, "unscoped apply toolkit must be rejected"
    except ValueError as error:
        assert "requires explicit capabilities" in str(error)

    toolkit = create_embedded_agent_toolkit(
        commerce,
        allow_apply=True,
        capabilities=["read:*", "create_customer"],
    )
    names = {tool["name"] for tool in toolkit.get_tools()}
    assert "list_customers" in names
    assert "create_customer" in names
    assert "create_order" not in names
    result = toolkit.execute_tool("create_order", {})
    assert result["status"] == "forbidden"


def test_kernel_tool_uses_host_policy_and_returns_durable_receipt():
    commerce = Commerce(":memory:")
    toolkit = create_embedded_agent_toolkit(
        commerce,
        allow_apply=True,
        capabilities=["execute_kernel_command", "payments.create"],
        kernel={
            "store_id": "store-1",
            "principal": {
                "id": "agent-1",
                "kind": "agent",
                "tenant_id": "tenant-1",
                "delegated_by": "user-1",
                "capabilities": ["payments.create"],
            },
            "policy": {
                "version": "agent-policy-1",
                "commands": {
                    "payments.create": {
                        "required_capabilities": ["payments.create"],
                        "requires_approval": False,
                        "requires_tenant": True,
                        "requires_store": True,
                        "requires_agent_delegation": True,
                        "requires_signed_authority": False,
                    }
                },
                "trusted_authority_keys": {},
            },
        },
    )

    result = toolkit.execute_tool(
        "execute_kernel_command",
        {
            "command_type": "payments.create",
            "idempotency_key": "python-payment-1",
            "payload": {
                "amount": "12.34",
                "currency": "USD",
                "payment_method": "credit_card",
            },
        },
    )

    assert result["success"] is True, result
    assert result["result"]["kernel"] is True
    assert result["result"]["receipt"]["status"] == "succeeded"
    assert result["result"]["receipt"]["idempotency_key"] == "python-payment-1"
    assert commerce.payments.count() == 1


def test_execute_openai_tool_call_returns_output_message():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="toolcall@example.com",
        first_name="Tool",
        last_name="Call",
    )
    toolkit = create_embedded_agent_toolkit(commerce)

    execution = toolkit.execute_openai_tool_call(
        {
            "call_id": "call_123",
            "function": {
                "name": "list_customers",
                "arguments": "{\"limit\": 1}",
            },
        }
    )

    assert execution["name"] == "list_customers"
    assert execution["arguments"]["limit"] == 1
    assert execution["result"]["status"] == "success"
    assert execution["output_message"]["type"] == "function_call_output"
    assert execution["output_message"]["call_id"] == "call_123"


def test_openai_entry_points_accept_commerce_or_toolkit():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="openai@example.com",
        first_name="OpenAI",
        last_name="Adapter",
    )

    toolkit = create_embedded_agent_toolkit(commerce)
    tools = create_openai_tools(commerce, filter=["list_customers"])
    execution = execute_openai_tool_call(
        toolkit,
        {
            "call_id": "call_openai_1",
            "function": {
                "name": "list_customers",
                "arguments": "{\"limit\": 1}",
            },
        },
    )
    batch = execute_openai_tool_calls(
        commerce,
        [
            {
                "call_id": "call_openai_2",
                "function": {
                    "name": "count_customers",
                    "arguments": "{}",
                },
            }
        ],
    )

    assert tools[0]["function"]["name"] == "list_customers"
    assert execution["result"]["status"] == "success"
    assert execution["output_message"]["call_id"] == "call_openai_1"
    assert batch[0]["name"] == "count_customers"
    assert batch[0]["result"]["status"] == "success"


def test_generic_entry_points_accept_commerce_or_toolkit():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="generic@example.com",
        first_name="Generic",
        last_name="Adapter",
    )

    toolkit = create_embedded_agent_toolkit(commerce)
    descriptors = create_tool_descriptors(commerce, filter=["list_customers"])
    registry = create_callable_registry(toolkit, filter=["list_customers"])
    result = execute_tool(commerce, "list_customers", {"limit": 1})
    batch = execute_tool_calls(
        toolkit,
        [{"name": "count_customers", "arguments": {}}],
    )

    assert descriptors[0].name == "list_customers"
    assert registry["list_customers"]({"limit": 1})["status"] == "success"
    assert result["status"] == "success"
    assert batch[0]["name"] == "count_customers"
    assert batch[0]["result"]["status"] == "success"


def test_create_callable_registry_exposes_executable_functions():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="registry@example.com",
        first_name="Registry",
        last_name="User",
    )
    toolkit = create_embedded_agent_toolkit(commerce)

    registry = toolkit.create_callable_registry(filter=["list_customers", "count_customers"])

    assert sorted(registry.keys()) == ["count_customers", "list_customers"]
    result = registry["list_customers"]({"limit": 1})
    assert result["status"] == "success"
    assert result["result"]["count"] == 1


def test_framework_adapters_accept_custom_factories():
    commerce = Commerce(":memory:")
    toolkit = create_embedded_agent_toolkit(commerce)

    def factory(descriptor):
        return {
            "name": descriptor.name,
            "description": descriptor.description,
            "schema": descriptor.schema,
            "side_effect": descriptor.side_effect,
            "status": descriptor.execute({}).get("status"),
        }

    langchain_tools = toolkit.create_langchain_tools(
        filter=["list_customers"],
        tool_factory=factory,
    )
    crewai_tools = toolkit.create_crewai_tools(
        filter=["count_customers"],
        tool_factory=factory,
    )
    autogen_tools = toolkit.create_autogen_tools(
        filter=["get_sales_summary"],
        tool_factory=factory,
    )

    assert langchain_tools[0]["name"] == "list_customers"
    assert langchain_tools[0]["status"] == "success"
    assert crewai_tools[0]["name"] == "count_customers"
    assert autogen_tools[0]["name"] == "get_sales_summary"


def test_framework_entry_points_accept_commerce_or_toolkit():
    commerce = Commerce(":memory:")
    commerce.customers.create(
        email="framework@example.com",
        first_name="Framework",
        last_name="Adapter",
    )

    def factory(descriptor):
        return {
            "name": descriptor.name,
            "status": descriptor.execute({"limit": 1}).get("status"),
        }

    toolkit = create_embedded_agent_toolkit(commerce)
    langchain_tools = create_langchain_tools(
        commerce,
        filter=["list_customers"],
        tool_factory=factory,
    )
    crewai_tools = create_crewai_module_tools(
        toolkit,
        filter=["list_customers"],
        tool_factory=factory,
    )
    autogen_tools = create_autogen_tools(
        commerce,
        filter=["list_customers"],
        tool_factory=factory,
    )

    assert langchain_tools == [{"name": "list_customers", "status": "success"}]
    assert crewai_tools == [{"name": "list_customers", "status": "success"}]
    assert autogen_tools == [{"name": "list_customers", "status": "success"}]


def test_framework_entry_points_preserve_write_gating():
    def factory(descriptor):
        return descriptor.execute(
            {
                "email": "write@example.com",
                "first_name": "Write",
                "last_name": "Gate",
            }
        )

    preview_tools = create_crewai_tools(
        Commerce(":memory:"),
        filter=["create_customer"],
        tool_factory=factory,
    )
    apply_tools = create_autogen_module_tools(
        Commerce(":memory:"),
        allow_apply=True,
        capabilities=["create_customer"],
        filter=["create_customer"],
        tool_factory=factory,
    )
    langchain_tools = create_langchain_module_tools(
        Commerce(":memory:"),
        filter=["create_customer"],
        tool_factory=factory,
    )

    assert preview_tools[0]["status"] == "preview"
    assert preview_tools[0]["preview"] is True
    assert apply_tools[0]["status"] == "success"
    assert apply_tools[0]["result"]["email"] == "write@example.com"
    assert langchain_tools[0]["status"] == "preview"


def test_openai_entry_points_preserve_write_gating():
    preview = execute_openai_module_tool_call(
        Commerce(":memory:"),
        {
            "call_id": "call_preview",
            "function": {
                "name": "create_customer",
                "arguments": (
                    "{\"email\":\"preview@example.com\",\"first_name\":\"Preview\",\"last_name\":\"Mode\"}"
                ),
            },
        },
    )
    applied = execute_openai_module_tool_call(
        Commerce(":memory:"),
        {
            "call_id": "call_apply",
            "function": {
                "name": "create_customer",
                "arguments": (
                    "{\"email\":\"apply@example.com\",\"first_name\":\"Apply\",\"last_name\":\"Mode\"}"
                ),
            },
        },
        allow_apply=True,
        capabilities=["create_customer"],
    )
    tools = create_openai_module_tools(Commerce(":memory:"), filter=["create_customer"])

    assert preview["result"]["status"] == "preview"
    assert preview["result"]["preview"] is True
    assert applied["result"]["status"] == "success"
    assert applied["result"]["result"]["email"] == "apply@example.com"
    assert tools[0]["function"]["name"] == "create_customer"


def test_generic_entry_points_preserve_write_gating():
    descriptors = create_generic_tool_descriptors(
        Commerce(":memory:"),
        filter=["create_customer"],
    )
    registry = create_generic_callable_registry(
        Commerce(":memory:"),
        filter=["create_customer"],
    )
    preview = execute_generic_tool(
        Commerce(":memory:"),
        "create_customer",
        {
            "email": "preview@example.com",
            "first_name": "Preview",
            "last_name": "Only",
        },
    )
    applied = execute_tool(
        Commerce(":memory:"),
        "create_customer",
        {
            "email": "apply@example.com",
            "first_name": "Apply",
            "last_name": "Now",
        },
        allow_apply=True,
        capabilities=["create_customer"],
    )

    assert descriptors[0].execute(
        {
            "email": "descriptor@example.com",
            "first_name": "Descriptor",
            "last_name": "Preview",
        }
    )["status"] == "preview"
    assert registry["create_customer"](
        {
            "email": "registry@example.com",
            "first_name": "Registry",
            "last_name": "Preview",
        }
    )["status"] == "preview"
    assert preview["status"] == "preview"
    assert applied["status"] == "success"
    assert applied["result"]["email"] == "apply@example.com"
