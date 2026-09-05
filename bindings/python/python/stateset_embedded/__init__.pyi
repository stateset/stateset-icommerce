"""Type stubs for stateset_embedded"""

from typing import Any, Callable, Dict, List, Mapping, Optional, Sequence, TypeVar, Union

__version__: str
FrameworkToolT = TypeVar("FrameworkToolT")

# ============================================================================
# Commerce
# ============================================================================

class Commerce:
    """Main Commerce instance for local commerce operations."""

    def __init__(self, db_path: str) -> None:
        """Create a new Commerce instance with a database path.

        Args:
            db_path: Path to SQLite database file, or ":memory:" for in-memory.
        """
        ...

    def execute_kernel_command(self, command_json: str, policy_json: str) -> str:
        """Execute a governed command and return its durable receipt as JSON.

        The policy must be trusted host configuration, not model-generated input.
        """
        ...

    def provision_economic_budget(self, budget_json: str) -> str:
        """Provision an immutable budget and return its exact status as JSON.

        This is an operator API and must not be exposed as a model tool.
        """
        ...

    def economic_budget_status(self, budget_id: str) -> str:
        """Return exact committed and available budget balances as JSON."""
        ...

    @property
    def customers(self) -> Customers:
        """Get the customers API."""
        ...

    @property
    def orders(self) -> Orders:
        """Get the orders API."""
        ...

    @property
    def products(self) -> Products:
        """Get the products API."""
        ...

    @property
    def custom_objects(self) -> CustomObjectsApi:
        """Get the custom objects API (custom states / metaobjects)."""
        ...

    @property
    def custom_states(self) -> CustomObjectsApi:
        """Alias for `custom_objects`."""
        ...

    @property
    def inventory(self) -> Inventory:
        """Get the inventory API."""
        ...

    @property
    def returns(self) -> Returns:
        """Get the returns API."""
        ...

    @property
    def gift_cards(self) -> GiftCards:
        """Get the gift cards API."""
        ...

    @property
    def accounts_payable(self) -> AccountsPayableApi:
        """Get the accounts payable API."""
        ...

    @property
    def general_ledger(self) -> GeneralLedgerApi:
        """Get the general ledger API."""
        ...

    @property
    def fixed_assets(self) -> FixedAssets:
        """Get the fixed assets API."""
        ...

    @property
    def activity_logs(self) -> ActivityLogs:
        """Get the activity logs API."""
        ...

    @property
    def channels(self) -> Channels:
        """Get the channels API."""
        ...

    @property
    def companies(self) -> Companies:
        """Get the companies API."""
        ...

    @property
    def units_of_measure(self) -> UnitsOfMeasure:
        """Get the units of measure API."""
        ...

    @property
    def shipping_zones(self) -> ShippingZones:
        """Get the shipping zones API."""
        ...

    @property
    def stock_snapshots(self) -> StockSnapshots:
        """Get the stock snapshots API."""
        ...

    @property
    def print_stations(self) -> PrintStations:
        """Get the print stations API."""
        ...

    @property
    def integration_mappings(self) -> IntegrationMappings:
        """Get the integration mappings API."""
        ...

    @property
    def integration_field_mappings(self) -> IntegrationFieldMappings:
        """Get the integration field mappings API."""
        ...

    @property
    def payment_obligations(self) -> PaymentObligations:
        """Get the payment obligations API."""
        ...

    @property
    def purgatory(self) -> Purgatory:
        """Get the purgatory API."""
        ...

    @property
    def topology_snapshots(self) -> TopologySnapshots:
        """Get the topology snapshots API."""
        ...

    @property
    def vendor_returns(self) -> VendorReturns:
        """Get the vendor returns API."""
        ...

    @property
    def fraud(self) -> Fraud:
        """Get the fraud API."""
        ...

    @property
    def search_config(self) -> SearchConfigs:
        """Get the search config API."""
        ...

    @property
    def erc8004(self) -> Erc8004:
        """Get the erc8004 API."""
        ...

    @property
    def revenue_recognition(self) -> RevenueRecognition:
        """Get the revenue recognition (ASC 606) API."""
        ...

    @property
    def cycle_counts(self) -> CycleCounts:
        """Get the cycle counts API."""
        ...

    @property
    def prepayments(self) -> Prepayments:
        """Get the prepayments API (advance payments to suppliers)."""
        ...

    @property
    def vendor_credits(self) -> VendorCredits:
        """Get the vendor credits API (supplier-owed credits)."""
        ...

    @property
    def price_schedules(self) -> PriceSchedules:
        """Get the price schedules API (time-bounded pricing)."""
        ...

    @property
    def price_levels(self) -> PriceLevels:
        """Get the price levels API (B2B pricing tiers)."""
        ...

    @property
    def transfer_orders(self) -> TransferOrders:
        """Get the transfer orders API (inter-warehouse stock movement)."""
        ...

    @property
    def production_batches(self) -> ProductionBatches:
        """Get the production batches API (grouping manufacturing work orders)."""
        ...

    @property
    def supplier_skus(self) -> SupplierSkus:
        """Get the supplier SKUs API (per-supplier SKU / unit-cost overrides)."""
        ...

    @property
    def inbound_shipments(self) -> InboundShipments:
        """Get the inbound shipments API (advance ship notices)."""
        ...

    @property
    def loyalty(self) -> Loyalty:
        """Get the loyalty API."""
        ...

    @property
    def store_credits(self) -> StoreCredits:
        """Get the store credits API."""
        ...

    @property
    def reviews(self) -> Reviews:
        """Get the product reviews API."""
        ...

    @property
    def wishlists(self) -> Wishlists:
        """Get the wishlists API."""
        ...

    @property
    def segments(self) -> Segments:
        """Get the customer segments API."""
        ...

    @property
    def payments(self) -> Payments:
        """Get the payments API."""
        ...

    @property
    def shipments(self) -> Shipments:
        """Get the shipments API."""
        ...

    @property
    def warranties(self) -> Warranties:
        """Get the warranties API."""
        ...

    @property
    def purchase_orders(self) -> PurchaseOrders:
        """Get the purchase orders API."""
        ...

    @property
    def invoices(self) -> Invoices:
        """Get the invoices API."""
        ...

    @property
    def bom(self) -> BomApi:
        """Get the bill of materials API."""
        ...

    @property
    def work_orders(self) -> WorkOrders:
        """Get the work orders API."""
        ...

    @property
    def carts(self) -> Carts:
        """Get the carts API."""
        ...

    @property
    def analytics(self) -> Analytics:
        """Get the analytics API."""
        ...

    @property
    def currency(self) -> CurrencyOperations:
        """Get the currency API."""
        ...

    def vector(self, openai_api_key: str) -> VectorSearch:
        """Get the vector search API for semantic search operations."""
        ...

# ============================================================================
# Agent Toolkit
# ============================================================================

class AgentToolDescriptor:
    name: str
    description: str
    schema: Dict[str, Any]
    input_schema: Dict[str, Any]
    side_effect: str

    def execute(self, params: Optional[Mapping[str, Any]] = None) -> Dict[str, Any]:
        ...

FrameworkToolFactory = Callable[[AgentToolDescriptor], FrameworkToolT]

class EmbeddedAgentToolkit:
    commerce: Commerce
    allow_apply: bool
    capabilities: Optional[set[str]]
    kernel: Optional[Dict[str, Any]]

    def __init__(
        self,
        commerce: Commerce,
        allow_apply: bool = False,
        capabilities: Optional[Sequence[str]] = None,
        kernel: Optional[Mapping[str, Any]] = None,
    ) -> None:
        ...

    def get_tools(
        self,
        format: str = "generic",
        filter: Optional[Sequence[str]] = None,
    ) -> List[Dict[str, Any]]:
        ...

    def list_tools(
        self,
        format: str = "generic",
        filter: Optional[Sequence[str]] = None,
    ) -> List[Dict[str, Any]]:
        ...

    def get_tool(self, tool_name: str, format: str = "generic") -> Optional[Dict[str, Any]]:
        ...

    def create_tool_descriptors(
        self,
        filter: Optional[Sequence[str]] = None,
    ) -> List[AgentToolDescriptor]:
        ...

    def create_callable_registry(
        self,
        filter: Optional[Sequence[str]] = None,
    ) -> Dict[str, Callable[[Optional[Mapping[str, Any]]], Dict[str, Any]]]:
        ...

    def create_langchain_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
    ) -> List[FrameworkToolT]:
        ...

    def create_crewai_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
    ) -> List[FrameworkToolT]:
        ...

    def create_autogen_tools(
        self,
        filter: Optional[Sequence[str]] = None,
        tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
    ) -> List[FrameworkToolT]:
        ...

    def execute_tool(
        self,
        tool_name: str,
        params: Optional[Mapping[str, Any]] = None,
    ) -> Dict[str, Any]:
        ...

    def execute_tool_calls(
        self,
        tool_calls: Sequence[Mapping[str, Any]],
    ) -> List[Dict[str, Any]]:
        ...

    def execute_openai_tool_call(
        self,
        tool_call: Mapping[str, Any],
    ) -> Dict[str, Any]:
        ...

def create_embedded_agent_toolkit(
    commerce: Commerce,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    kernel: Optional[Mapping[str, Any]] = None,
) -> EmbeddedAgentToolkit:
    ...

def create_tool_descriptors(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[AgentToolDescriptor]:
    ...

def create_callable_registry(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> Dict[str, Callable[[Optional[Mapping[str, Any]]], Dict[str, Any]]]:
    ...

def execute_tool(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_name: str,
    params: Optional[Mapping[str, Any]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> Dict[str, Any]:
    ...

def execute_tool_calls(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[Mapping[str, Any]]:
    ...

def create_openai_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[Mapping[str, Any]]:
    ...

def execute_openai_tool_call(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_call: Mapping[str, Any],
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> Mapping[str, Any]:
    ...

def execute_openai_tool_calls(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
) -> List[Mapping[str, Any]]:
    ...

def create_langchain_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
) -> List[FrameworkToolT]:
    ...

def create_crewai_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
) -> List[FrameworkToolT]:
    ...

def create_autogen_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    capabilities: Optional[Sequence[str]] = None,
    tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
) -> List[FrameworkToolT]:
    ...

# ============================================================================
# Sync Runtime
# ============================================================================

class SyncEvent:
    id: str
    sequence: int
    sequence_authority: str
    canonical_sequence: Optional[int]
    local_sequence: Optional[int]
    event_type: str
    entity_type: str
    entity_id: str
    payload_json: str
    hash: str
    signature: Optional[str]
    command_id: Optional[str]
    base_version: Optional[int]
    source_agent_id: Optional[str]
    agent_key_id: Optional[int]
    timestamp: str

class SyncStatus:
    initialized: bool
    local_head: int
    remote_head: int
    remote_state_root: Optional[str]
    last_commitment_id: Optional[str]
    remote_cursor: int
    next_pull_cursor: Optional[int]
    last_acknowledged_remote_sequence: Optional[int]
    pending: int
    dead_letters: int
    retained_confirmations: int
    lag: int
    caught_up: bool
    last_push: Optional[str]
    last_pull: Optional[str]
    buffered_events: int

class SyncRemoteHead:
    remote_head: int
    state_root: Optional[str]
    last_commitment_id: Optional[str]

class SyncAcknowledgement:
    event_id: str
    remote_sequence: int
    receipt: Optional[str]

class SyncRejection:
    event_id: str
    code: Optional[str]
    reason: Optional[str]
    retryable: Optional[bool]

class SyncPushResult:
    accepted: int
    remote_head: int
    acknowledged_head: Optional[int]
    acknowledgements: List[SyncAcknowledgement]
    rejections: List[SyncRejection]

class SyncConfirmation:
    event_id: str
    command_id: Optional[str]
    event_type: str
    entity_type: str
    entity_id: str
    local_sequence: Optional[int]
    remote_sequence: int
    hash: str
    receipt: Optional[str]
    confirmed_at: str

class SyncDeadLetter:
    event: SyncEvent
    rejection: SyncRejection
    rejected_at: str

class SyncPullResult:
    events: List[SyncEvent]
    remote_head: int
    has_more: bool

class SyncSnapshot:
    status: SyncStatus
    confirmations: List[SyncConfirmation]
    dead_letters: List[SyncDeadLetter]
    buffered_events: List[SyncEvent]

class SyncFullSyncResult:
    push: SyncPushResult
    pull: SyncPullResult

class SyncRuntime:
    """Sequencer sync runtime for recording, pushing, and pulling events."""

    def __init__(self, config_json: str) -> None:
        """Create a sync runtime from a JSON-serialized SyncRuntimeConfig."""
        ...

    @staticmethod
    def from_file(path: str) -> "SyncRuntime":
        """Create a sync runtime from a JSON config file."""
        ...

    @staticmethod
    def from_env(prefix: Optional[str] = None) -> "SyncRuntime":
        """Create a sync runtime from environment variables."""
        ...

    @property
    def initialized(self) -> bool:
        ...

    @property
    def caught_up(self) -> bool:
        ...

    @property
    def local_head(self) -> int:
        ...

    @property
    def remote_head(self) -> int:
        ...

    @property
    def remote_cursor(self) -> int:
        ...

    @property
    def next_pull_cursor(self) -> Optional[int]:
        ...

    @property
    def remote_state_root(self) -> Optional[str]:
        ...

    @property
    def last_commitment_id(self) -> Optional[str]:
        ...

    @property
    def last_acknowledged_remote_sequence(self) -> Optional[int]:
        ...

    @property
    def lag(self) -> int:
        ...

    @property
    def pending_count(self) -> int:
        ...

    @property
    def confirmation_count(self) -> int:
        ...

    @property
    def dead_letter_count(self) -> int:
        ...

    @property
    def buffered_count(self) -> int:
        ...

    def record(
        self,
        event_type: str,
        entity_type: str,
        entity_id: str,
        payload_json: str,
        command_id: Optional[str] = None,
        base_version: Optional[int] = None,
        source_agent_id: Optional[str] = None,
        agent_key_id: Optional[int] = None,
        signature: Optional[str] = None,
    ) -> int:
        ...

    def record_event_json(self, event_json: str) -> int:
        ...

    def status(self) -> SyncStatus:
        ...

    def snapshot(self) -> SyncSnapshot:
        ...

    def confirmations(self) -> List[SyncConfirmation]:
        ...

    def confirmation_for_event(self, event_id: str) -> Optional[SyncConfirmation]:
        ...

    def drain_confirmations(self) -> List[SyncConfirmation]:
        ...

    def dead_letters(self) -> List[SyncDeadLetter]:
        ...

    def dead_letter_for_event(self, event_id: str) -> Optional[SyncDeadLetter]:
        ...

    def requeue_dead_letter(self, event_id: str) -> int:
        ...

    def discard_dead_letter(self, event_id: str) -> SyncDeadLetter:
        ...

    def drain_dead_letters(self) -> List[SyncDeadLetter]:
        ...

    def buffered_events(self) -> List[SyncEvent]:
        ...

    def drain_buffer(self) -> List[SyncEvent]:
        ...

    def healthcheck(self) -> bool:
        ...

    def refresh_remote_head(self) -> SyncRemoteHead:
        ...

    def push(self) -> SyncPushResult:
        ...

    def pull(self) -> SyncPullResult:
        ...

    def full_sync(self) -> SyncFullSyncResult:
        ...

    def status_json(self) -> str:
        ...

    def snapshot_json(self, pretty: bool = False) -> str:
        ...

    def confirmations_json(self) -> str:
        ...

    def confirmation_for_event_json(self, event_id: str) -> str:
        ...

    def drain_confirmations_json(self) -> str:
        ...

    def dead_letters_json(self) -> str:
        ...

    def dead_letter_for_event_json(self, event_id: str) -> str:
        ...

    def discard_dead_letter_json(self, event_id: str) -> str:
        ...

    def drain_dead_letters_json(self) -> str:
        ...

    def buffered_events_json(self) -> str:
        ...

    def drain_buffer_json(self) -> str:
        ...

    def refresh_remote_head_json(self) -> str:
        ...

    def push_json(self) -> str:
        ...

    def pull_json(self) -> str:
        ...

    def full_sync_json(self) -> str:
        ...

# ============================================================================
# Customers
# ============================================================================

class Customer:
    """Customer data returned from operations."""

    id: str
    email: str
    first_name: str
    last_name: str
    phone: Optional[str]
    status: str
    accepts_marketing: bool
    created_at: str
    updated_at: str

    @property
    def full_name(self) -> str:
        """Get the full name."""
        ...

class Customers:
    """Customer management operations."""

    def create(
        self,
        email: str,
        first_name: str,
        last_name: str,
        phone: Optional[str] = None,
        accepts_marketing: Optional[bool] = None,
    ) -> Customer:
        """Create a new customer.

        Args:
            email: Customer email address
            first_name: First name
            last_name: Last name
            phone: Phone number (optional)
            accepts_marketing: Marketing opt-in (optional)

        Returns:
            The created customer
        """
        ...

    def get(self, id: str) -> Optional[Customer]:
        """Get a customer by ID."""
        ...

    def get_by_email(self, email: str) -> Optional[Customer]:
        """Get a customer by email."""
        ...

    def list(self) -> List[Customer]:
        """List all customers."""
        ...

    def count(self) -> int:
        """Count customers."""
        ...

# ============================================================================
# Orders
# ============================================================================

class OrderItem:
    """Order line item."""

    id: str
    sku: str
    name: str
    quantity: int
    unit_price: float
    unit_price_exact: str
    total: float
    total_exact: str

class Order:
    """Order data returned from operations."""

    id: str
    order_number: str
    customer_id: str
    status: str
    total_amount: float
    total_amount_exact: str
    currency: str
    payment_status: str
    fulfillment_status: str
    tracking_number: Optional[str]
    items: List[OrderItem]
    created_at: str
    updated_at: str

    @property
    def item_count(self) -> int:
        """Get the number of items in the order."""
        ...

class CreateOrderItemInput:
    """Input for creating an order item."""

    sku: str
    name: str
    quantity: int
    unit_price: float
    product_id: Optional[str]
    variant_id: Optional[str]

    def __init__(
        self,
        sku: str,
        name: str,
        quantity: int,
        unit_price: float,
        product_id: Optional[str] = None,
        variant_id: Optional[str] = None,
    ) -> None: ...

class Orders:
    """Order management operations."""

    def create(
        self,
        customer_id: str,
        items: List[CreateOrderItemInput],
        currency: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> Order:
        """Create a new order."""
        ...

    def get(self, id: str) -> Optional[Order]:
        """Get an order by ID."""
        ...

    def list(self) -> List[Order]:
        """List all orders."""
        ...

    def update_status(self, id: str, status: str) -> Order:
        """Update order status."""
        ...

    def ship(self, id: str, tracking_number: Optional[str] = None) -> Order:
        """Ship an order."""
        ...

    def cancel(self, id: str) -> Order:
        """Cancel an order."""
        ...

    def count(self) -> int:
        """Count orders."""
        ...

# ============================================================================
# Products
# ============================================================================

class Product:
    """Product data returned from operations."""

    id: str
    name: str
    slug: str
    description: str
    status: str
    created_at: str
    updated_at: str

class ProductVariant:
    """Product variant data."""

    id: str
    product_id: str
    sku: str
    name: str
    price: float
    price_exact: str
    compare_at_price: Optional[float]
    compare_at_price_exact: Optional[str]
    is_default: bool

class CreateProductVariantInput:
    """Input for creating a product variant."""

    sku: str
    name: Optional[str]
    price: float
    compare_at_price: Optional[float]

    def __init__(
        self,
        sku: str,
        price: float,
        name: Optional[str] = None,
        compare_at_price: Optional[float] = None,
    ) -> None: ...

class Products:
    """Product catalog operations."""

    def create(
        self,
        name: str,
        description: Optional[str] = None,
        variants: Optional[List[CreateProductVariantInput]] = None,
    ) -> Product:
        """Create a new product."""
        ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        slug: Optional[str] = None,
        description: Optional[str] = None,
        status: Optional[str] = None,
    ) -> Product:
        """Update a product, including publishing it as active."""
        ...

    def get(self, id: str) -> Optional[Product]:
        """Get a product by ID."""
        ...

    def get_variant_by_sku(self, sku: str) -> Optional[ProductVariant]:
        """Get a product variant by SKU."""
        ...

    def list(self) -> List[Product]:
        """List all products."""
        ...

    def count(self) -> int:
        """Count products."""
        ...

# ============================================================================
# Custom Objects
# ============================================================================

class CustomFieldDefinition:
    """Custom field definition in a custom object type schema."""

    key: str
    field_type: str
    required: bool
    list: bool
    description: Optional[str]

class CustomFieldDefinitionInput:
    """Input for defining a custom field in a schema."""

    key: str
    field_type: str
    required: bool
    list: bool
    description: Optional[str]

    def __init__(
        self,
        key: str,
        field_type: str,
        required: bool = False,
        list: bool = False,
        description: Optional[str] = None,
    ) -> None: ...

class CustomObjectType:
    """Custom object type (schema) definition."""

    id: str
    handle: str
    display_name: str
    description: str
    fields: List[CustomFieldDefinition]
    created_at: str
    updated_at: str
    version: int

class CustomObject:
    """Custom object record (validated instance of a type)."""

    id: str
    type_id: str
    type_handle: str
    handle: Optional[str]
    owner_type: Optional[str]
    owner_id: Optional[str]
    values_json: str
    created_at: str
    updated_at: str
    version: int

class CustomObjectsApi:
    """Custom objects API for schemas and records."""

    def create_type(
        self,
        handle: str,
        display_name: str,
        description: Optional[str] = None,
        fields: Optional[List[CustomFieldDefinitionInput]] = None,
    ) -> CustomObjectType: ...

    def get_type(self, id: str) -> Optional[CustomObjectType]: ...
    def get_type_by_handle(self, handle: str) -> Optional[CustomObjectType]: ...

    def update_type(
        self,
        id: str,
        display_name: Optional[str] = None,
        description: Optional[str] = None,
        fields: Optional[List[CustomFieldDefinitionInput]] = None,
    ) -> CustomObjectType: ...

    def list_types(
        self,
        search: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[CustomObjectType]: ...

    def delete_type(self, id: str) -> None: ...

    def create_object(
        self,
        type_handle: str,
        values_json: str,
        handle: Optional[str] = None,
        owner_type: Optional[str] = None,
        owner_id: Optional[str] = None,
    ) -> CustomObject: ...

    def get_object(self, id: str) -> Optional[CustomObject]: ...

    def get_object_by_handle(
        self,
        type_handle: str,
        object_handle: str,
    ) -> Optional[CustomObject]: ...

    def update_object(
        self,
        id: str,
        handle: Optional[str] = None,
        owner_type: Optional[str] = None,
        owner_id: Optional[str] = None,
        values_json: Optional[str] = None,
    ) -> CustomObject: ...

    def list_objects(
        self,
        type_handle: Optional[str] = None,
        owner_type: Optional[str] = None,
        owner_id: Optional[str] = None,
        handle: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[CustomObject]: ...

    def delete_object(self, id: str) -> None: ...

# ============================================================================
# Inventory
# ============================================================================

class InventoryItem:
    """Inventory item data."""

    id: int
    sku: str
    name: str
    description: Optional[str]
    unit_of_measure: str
    is_active: bool

class StockLevel:
    """Stock level information."""

    sku: str
    name: str
    total_on_hand: float
    total_allocated: float
    total_available: float

class Reservation:
    """Inventory reservation."""

    id: str
    item_id: int
    quantity: float
    status: str

class Inventory:
    """Inventory management operations."""

    def create_item(
        self,
        sku: str,
        name: str,
        description: Optional[str] = None,
        initial_quantity: Optional[float] = None,
        reorder_point: Optional[float] = None,
    ) -> InventoryItem:
        """Create a new inventory item."""
        ...

    def get_stock(self, sku: str) -> Optional[StockLevel]:
        """Get stock level for a SKU."""
        ...

    def adjust(self, sku: str, quantity: float, reason: str) -> None:
        """Adjust inventory quantity."""
        ...

    def reserve(
        self,
        sku: str,
        quantity: float,
        reference_type: str,
        reference_id: str,
        expires_in_seconds: Optional[int] = None,
    ) -> Reservation:
        """Reserve inventory for an order."""
        ...

    def confirm_reservation(self, reservation_id: str) -> None:
        """Confirm a reservation."""
        ...

    def release_reservation(self, reservation_id: str) -> None:
        """Release a reservation."""
        ...

# ============================================================================
# Returns
# ============================================================================

class Return:
    """Return request data."""

    id: str
    order_id: str
    status: str
    reason: str
    idempotency_key: Optional[str]
    created_at: str

class CreateReturnItemInput:
    """Input for creating a return item."""

    order_item_id: str
    quantity: int

    def __init__(self, order_item_id: str, quantity: int) -> None: ...

class Returns:
    """Return processing operations."""

    def create(
        self,
        order_id: str,
        reason: str,
        items: List[CreateReturnItemInput],
        reason_details: Optional[str] = None,
        idempotency_key: Optional[str] = None,
    ) -> Return:
        """Create a new return request."""
        ...

    def get(self, id: str) -> Optional[Return]:
        """Get a return by ID."""
        ...

    def approve(self, id: str) -> Return:
        """Approve a return request."""
        ...

    def reject(self, id: str, reason: str) -> Return:
        """Reject a return request."""
        ...

    def list(self) -> List[Return]:
        """List all returns."""
        ...

    def count(self) -> int:
        """Count returns."""
        ...

# ============================================================================
# Gift Cards  (money as exact decimal strings)
# ============================================================================

class GiftCard:
    """A gift card. Balances are exact decimal strings (e.g. "50.00")."""

    id: str
    code: str
    initial_balance: str
    current_balance: str
    currency: str
    status: str
    recipient_email: Optional[str]
    sender_name: Optional[str]
    message: Optional[str]
    expires_at: Optional[str]
    created_at: str
    updated_at: str

class GiftCardTransaction:
    """A gift card charge or refund. Amounts are exact decimal strings."""

    id: str
    gift_card_id: str
    amount: str
    balance_after: str
    transaction_type: str
    reference_id: Optional[str]
    created_at: str

class GiftCards:
    """Gift card operations."""

    def is_supported(self) -> bool:
        """Whether the gift-cards backend is available on this engine build."""
        ...

    def create(
        self,
        initial_balance: str,
        currency: str,
        code: Optional[str] = None,
        recipient_email: Optional[str] = None,
        sender_name: Optional[str] = None,
        message: Optional[str] = None,
        expires_at: Optional[str] = None,
    ) -> GiftCard:
        """Create a gift card (money amounts are exact decimal strings)."""
        ...

    def get(self, id: str) -> Optional[GiftCard]:
        """Get a gift card by ID."""
        ...

    def get_by_code(self, code: str) -> Optional[GiftCard]:
        """Get a gift card by its redemption code."""
        ...

    def update(
        self,
        id: str,
        status: Optional[str] = None,
        recipient_email: Optional[str] = None,
    ) -> GiftCard:
        """Update a gift card's status and/or recipient email."""
        ...

    def list(
        self,
        status: Optional[str] = None,
        code: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[GiftCard]:
        """List gift cards, optionally filtered."""
        ...

    def charge(
        self, id: str, amount: str, reference_id: Optional[str] = None
    ) -> GiftCardTransaction:
        """Charge (debit) an amount from a gift card."""
        ...

    def refund(
        self, id: str, amount: str, reference_id: Optional[str] = None
    ) -> GiftCardTransaction:
        """Refund (credit) an amount to a gift card."""
        ...

    def disable(self, id: str) -> GiftCard:
        """Disable a gift card so it can no longer be used."""
        ...

    def get_transactions(self, gift_card_id: str) -> List[GiftCardTransaction]:
        """Get the transaction history for a gift card."""
        ...

# ============================================================================
# Store Credits  (money as exact decimal strings)
# ============================================================================

class StoreCredit:
    """A customer store credit. Balances are exact decimal strings."""

    id: str
    customer_id: str
    original_balance: str
    current_balance: str
    currency: str
    status: str
    reason: str
    reference_id: Optional[str]
    note: Optional[str]
    expires_at: Optional[str]
    created_at: str
    updated_at: str

class StoreCreditTransaction:
    """A store credit ledger entry. Amounts are exact decimal strings
    (positive = credit, negative = debit)."""

    id: str
    store_credit_id: str
    amount: str
    balance_after: str
    transaction_type: str
    reference_id: Optional[str]
    created_at: str

class StoreCredits:
    """Store credit operations."""

    def is_supported(self) -> bool:
        """Whether the store-credits backend is available on this engine build."""
        ...

    def create(
        self,
        customer_id: str,
        amount: str,
        currency: str,
        reason: Optional[str] = None,
        reference_id: Optional[str] = None,
        note: Optional[str] = None,
        expires_at: Optional[str] = None,
    ) -> StoreCredit:
        """Issue a store credit to a customer (money amounts are exact decimal strings)."""
        ...

    def get(self, id: str) -> Optional[StoreCredit]:
        """Get a store credit by ID."""
        ...

    def list(
        self,
        customer_id: Optional[str] = None,
        status: Optional[str] = None,
        reason: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[StoreCredit]:
        """List store credits, optionally filtered."""
        ...

    def adjust(
        self,
        id: str,
        amount: str,
        note: Optional[str] = None,
        reference_id: Optional[str] = None,
    ) -> StoreCredit:
        """Adjust a store credit balance (signed decimal string; may not go below zero)."""
        ...

    def apply(
        self, id: str, amount: str, reference_id: Optional[str] = None
    ) -> StoreCreditTransaction:
        """Apply (redeem) an amount from a store credit."""
        ...

    def get_transactions(self, store_credit_id: str) -> List[StoreCreditTransaction]:
        """Get the transaction history for a store credit."""
        ...

# ============================================================================
# Product reviews
# ============================================================================

class Review:
    """A product review."""

    id: str
    product_id: str
    customer_id: str
    rating: int
    title: Optional[str]
    body: Optional[str]
    status: str
    verified_purchase: bool
    helpful_count: int
    reported_count: int
    created_at: str
    updated_at: str

class ReviewSummary:
    """Aggregate rating summary for a product."""

    product_id: str
    average_rating: float
    total_reviews: int
    rating_distribution: List[int]

class Reviews:
    """Product review operations."""

    def is_supported(self) -> bool:
        """Whether the reviews backend is available on this engine build."""
        ...

    def create(
        self,
        product_id: str,
        customer_id: str,
        rating: int,
        title: Optional[str] = None,
        body: Optional[str] = None,
        verified_purchase: bool = False,
    ) -> Review:
        """Create a product review (rating is 1-5)."""
        ...

    def get(self, id: str) -> Optional[Review]:
        """Get a review by ID."""
        ...

    def update(
        self,
        id: str,
        rating: Optional[int] = None,
        title: Optional[str] = None,
        body: Optional[str] = None,
        status: Optional[str] = None,
    ) -> Review:
        """Update a review's rating, title, body, and/or moderation status."""
        ...

    def list(
        self,
        product_id: Optional[str] = None,
        customer_id: Optional[str] = None,
        status: Optional[str] = None,
        min_rating: Optional[int] = None,
        verified_only: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Review]:
        """List reviews, optionally filtered."""
        ...

    def delete(self, id: str) -> None:
        """Delete a review."""
        ...

    def get_summary(self, product_id: str) -> ReviewSummary:
        """Aggregate rating summary for a product (average, total, distribution)."""
        ...

    def mark_helpful(self, id: str) -> None:
        """Increment the helpful counter on a review."""
        ...

    def mark_reported(self, id: str) -> None:
        """Increment the reported counter on a review."""
        ...

# ============================================================================
# Wishlists
# ============================================================================

class WishlistItem:
    """An item on a wishlist."""

    product_id: str
    variant_id: Optional[str]
    added_at: str
    note: Optional[str]
    quantity: int
    priority: Optional[int]

class Wishlist:
    """A customer wishlist with its items."""

    id: str
    customer_id: str
    name: str
    is_public: bool
    items: List[WishlistItem]
    created_at: str
    updated_at: str

class Wishlists:
    """Wishlist operations."""

    def is_supported(self) -> bool:
        """Whether the wishlists backend is available on this engine build."""
        ...

    def create(self, customer_id: str, name: str, is_public: bool = False) -> Wishlist:
        """Create a wishlist for a customer."""
        ...

    def get(self, id: str) -> Optional[Wishlist]:
        """Get a wishlist by ID."""
        ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        is_public: Optional[bool] = None,
    ) -> Wishlist:
        """Rename a wishlist and/or change its visibility."""
        ...

    def list(
        self,
        customer_id: Optional[str] = None,
        is_public: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Wishlist]:
        """List wishlists, optionally filtered."""
        ...

    def delete(self, id: str) -> None:
        """Delete a wishlist."""
        ...

    def add_item(
        self,
        wishlist_id: str,
        product_id: str,
        variant_id: Optional[str] = None,
        note: Optional[str] = None,
        quantity: Optional[int] = None,
        priority: Optional[int] = None,
    ) -> WishlistItem:
        """Add a product to a wishlist, returning the added item."""
        ...

    def remove_item(self, wishlist_id: str, product_id: str) -> None:
        """Remove a product from a wishlist."""
        ...

# ============================================================================
# Customer segments
# ============================================================================

class SegmentRuleInput:
    """A segment rule (field/operator/value) passed to create/update."""

    field: str
    operator: str
    value: str

    def __init__(self, field: str, operator: str, value: str) -> None: ...

class SegmentRule:
    """A segment rule returned on a segment."""

    field: str
    operator: str
    value: str

class Segment:
    """A customer segment with its rules."""

    id: str
    name: str
    description: Optional[str]
    segment_type: str
    rules: List[SegmentRule]
    member_count: int
    created_at: str
    updated_at: str

class SegmentMembership:
    """A customer's membership in a segment."""

    segment_id: str
    customer_id: str
    joined_at: str

class Segments:
    """Customer segment operations."""

    def is_supported(self) -> bool:
        """Whether the segments backend is available on this engine build."""
        ...

    def create(
        self,
        name: str,
        description: Optional[str] = None,
        segment_type: Optional[str] = None,
        rules: List[SegmentRuleInput] = ...,
    ) -> Segment:
        """Create a customer segment (segment_type is 'static' or 'dynamic')."""
        ...

    def get(self, id: str) -> Optional[Segment]:
        """Get a segment by ID."""
        ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
        rules: Optional[List[SegmentRuleInput]] = None,
    ) -> Segment:
        """Update a segment's name, description, and/or rules."""
        ...

    def list(
        self,
        segment_type: Optional[str] = None,
        name: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Segment]:
        """List segments, optionally filtered."""
        ...

    def delete(self, id: str) -> None:
        """Delete a segment."""
        ...

    def add_member(self, segment_id: str, customer_id: str) -> SegmentMembership:
        """Add a customer to a (static) segment."""
        ...

    def remove_member(self, segment_id: str, customer_id: str) -> None:
        """Remove a customer from a segment."""
        ...

    def list_members(
        self,
        segment_id: str,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[SegmentMembership]:
        """List a segment's members."""
        ...

    def is_member(self, segment_id: str, customer_id: str) -> bool:
        """Whether a customer is a member of a segment."""
        ...

# ============================================================================
# Loyalty  (points are integers; reward value is an exact decimal string)
# ============================================================================

class LoyaltyTierInput:
    """A loyalty tier passed to create_program."""

    name: str
    min_points: int
    multiplier: float
    perks: List[str]

    def __init__(
        self,
        name: str,
        min_points: int = 0,
        multiplier: float = 1.0,
        perks: List[str] = ...,
    ) -> None: ...

class LoyaltyTier:
    """A loyalty tier returned on a program."""

    name: str
    min_points: int
    multiplier: float
    perks: List[str]

class LoyaltyProgram:
    id: str
    name: str
    description: Optional[str]
    points_per_dollar: int
    tiers: List[LoyaltyTier]
    status: str
    created_at: str
    updated_at: str

class LoyaltyAccount:
    id: str
    customer_id: str
    program_id: str
    points_balance: int
    lifetime_points: int
    tier: str
    created_at: str
    updated_at: str

class LoyaltyTransaction:
    id: str
    account_id: str
    points: int
    transaction_type: str
    reference_id: Optional[str]
    description: Optional[str]
    created_at: str

class Reward:
    id: str
    program_id: str
    name: str
    description: Optional[str]
    points_cost: int
    reward_type: str
    value: Optional[str]
    is_active: bool
    created_at: str
    updated_at: str

class Loyalty:
    """Loyalty program operations."""

    def is_supported(self) -> bool: ...
    def create_program(
        self,
        name: str,
        points_per_dollar: int,
        description: Optional[str] = None,
        tiers: List[LoyaltyTierInput] = ...,
    ) -> LoyaltyProgram: ...
    def get_program(self, id: str) -> Optional[LoyaltyProgram]: ...
    def list_programs(self) -> List[LoyaltyProgram]: ...
    def enroll(self, customer_id: str, program_id: str) -> LoyaltyAccount: ...
    def get_account(self, id: str) -> Optional[LoyaltyAccount]: ...
    def get_account_by_customer(
        self, customer_id: str, program_id: str
    ) -> Optional[LoyaltyAccount]: ...
    def list_accounts(
        self,
        customer_id: Optional[str] = None,
        program_id: Optional[str] = None,
        tier: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[LoyaltyAccount]: ...
    def adjust_points(
        self,
        account_id: str,
        points: int,
        transaction_type: str,
        reference_id: Optional[str] = None,
        description: Optional[str] = None,
    ) -> LoyaltyTransaction: ...
    def get_transactions(
        self, account_id: str, limit: Optional[int] = None
    ) -> List[LoyaltyTransaction]: ...
    def create_reward(
        self,
        program_id: str,
        name: str,
        points_cost: int,
        reward_type: str,
        description: Optional[str] = None,
        value: Optional[str] = None,
    ) -> Reward: ...
    def get_reward(self, id: str) -> Optional[Reward]: ...
    def list_rewards(
        self,
        program_id: Optional[str] = None,
        reward_type: Optional[str] = None,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Reward]: ...
    def delete_reward(self, id: str) -> None: ...

# ============================================================================
# Payments
# ============================================================================

class Payment:
    """Payment data returned from operations."""

    id: str
    payment_number: str
    order_id: Optional[str]
    invoice_id: Optional[str]
    customer_id: Optional[str]
    idempotency_key: Optional[str]
    amount: float
    amount_exact: str
    currency: str
    status: str
    payment_method: str
    created_at: str
    updated_at: str

class Refund:
    """Refund data returned from operations."""

    id: str
    payment_id: str
    idempotency_key: Optional[str]
    amount: float
    amount_exact: str
    status: str
    reason: Optional[str]
    created_at: str

class Payments:
    """Payment processing operations."""

    def create(
        self,
        amount: float,
        currency: Optional[str] = None,
        order_id: Optional[str] = None,
        customer_id: Optional[str] = None,
        payment_method: Optional[str] = None,
        idempotency_key: Optional[str] = None,
    ) -> Payment: ...

    def create_exact(
        self,
        amount: str,
        currency: Optional[str] = None,
        order_id: Optional[str] = None,
        customer_id: Optional[str] = None,
        payment_method: Optional[str] = None,
        idempotency_key: Optional[str] = None,
    ) -> Payment: ...

    def get(self, id: str) -> Optional[Payment]: ...

    def list(self) -> List[Payment]: ...

    def complete(self, id: str) -> Payment: ...

    def mark_failed(self, id: str, reason: str, code: Optional[str] = None) -> Payment: ...

    def create_refund(
        self,
        payment_id: str,
        amount: float,
        reason: Optional[str] = None,
        idempotency_key: Optional[str] = None,
    ) -> Refund: ...

    def create_refund_exact(
        self,
        payment_id: str,
        amount: str,
        reason: Optional[str] = None,
        idempotency_key: Optional[str] = None,
    ) -> Refund: ...

    def count(self) -> int: ...

# ============================================================================
# Shipments
# ============================================================================

class Shipment:
    """Shipment data returned from operations."""

    id: str
    shipment_number: str
    order_id: str
    status: str
    carrier: str
    shipping_method: str
    tracking_number: Optional[str]
    tracking_url: Optional[str]
    recipient_name: str
    shipping_address: str
    created_at: str
    updated_at: str

class Shipments:
    """Shipment management operations."""

    def create(
        self,
        order_id: str,
        recipient_name: str,
        shipping_address: str,
        carrier: Optional[str] = None,
        shipping_method: Optional[str] = None,
        tracking_number: Optional[str] = None,
    ) -> Shipment: ...

    def get(self, id: str) -> Optional[Shipment]: ...

    def list(self) -> List[Shipment]: ...

    def ship(self, id: str, tracking_number: Optional[str] = None) -> Shipment: ...

    def mark_delivered(self, id: str) -> Shipment: ...

    def cancel(self, id: str) -> Shipment: ...

    def count(self) -> int: ...

# ============================================================================
# Warranties
# ============================================================================

class Warranty:
    """Warranty data returned from operations."""

    id: str
    warranty_number: str
    customer_id: str
    product_id: Optional[str]
    order_id: Optional[str]
    status: str
    warranty_type: str
    start_date: str
    end_date: str

class WarrantyClaim:
    """Warranty claim data."""

    id: str
    claim_number: str
    warranty_id: str
    status: str
    issue_description: str
    resolution: Optional[str]
    created_at: str

class Warranties:
    """Warranty management operations."""

    def create(
        self,
        customer_id: str,
        product_id: Optional[str] = None,
        order_id: Optional[str] = None,
        warranty_type: Optional[str] = None,
        duration_months: Optional[int] = None,
        serial_number: Optional[str] = None,
    ) -> Warranty: ...

    def get(self, id: str) -> Optional[Warranty]: ...

    def list(self) -> List[Warranty]: ...

    def create_claim(self, warranty_id: str, issue_description: str) -> WarrantyClaim: ...

    def approve_claim(self, id: str) -> WarrantyClaim: ...

    def deny_claim(self, id: str, reason: str) -> WarrantyClaim: ...

    def complete_claim(self, id: str, resolution: str) -> WarrantyClaim: ...

    def count(self) -> int: ...

# ============================================================================
# Purchase Orders
# ============================================================================

class Supplier:
    """Supplier data."""

    id: str
    supplier_code: str
    name: str
    email: Optional[str]
    phone: Optional[str]
    status: str

class PurchaseOrder:
    """Purchase order data."""

    id: str
    po_number: str
    supplier_id: str
    status: str
    total_amount: float
    currency: str
    created_at: str
    updated_at: str

class PurchaseOrders:
    """Purchase order management operations."""

    def create_supplier(self, name: str, email: Optional[str] = None, phone: Optional[str] = None) -> Supplier: ...

    def get_supplier(self, id: str) -> Optional[Supplier]: ...

    def list_suppliers(self) -> List[Supplier]: ...

    def create(self, supplier_id: str) -> PurchaseOrder: ...

    def get(self, id: str) -> Optional[PurchaseOrder]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        status: Optional[str] = None,
        from_date: Optional[str] = None,
        to_date: Optional[str] = None,
        min_total: Optional[str] = None,
        max_total: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[PurchaseOrder]: ...

    def submit(self, id: str) -> PurchaseOrder: ...

    def approve(self, id: str, approved_by: str) -> PurchaseOrder: ...

    def send(self, id: str) -> PurchaseOrder: ...

    def cancel(self, id: str) -> PurchaseOrder: ...

    def count(self) -> int: ...

# ============================================================================
# Invoices
# ============================================================================

class Invoice:
    """Invoice data returned from operations."""

    id: str
    invoice_number: str
    status: str
    total_amount: float
    balance_due: float
    currency: str
    created_at: str
    updated_at: str

class Invoices:
    """Invoice management operations."""

    def create(self, customer_id: str, invoice_type: Optional[str] = None) -> Invoice: ...

    def get(self, id: str) -> Optional[Invoice]: ...

    def list(self) -> List[Invoice]: ...

    def send(self, id: str) -> Invoice: ...

    def void(self, id: str) -> Invoice: ...

    def record_payment(self, id: str, amount: float, payment_method: str) -> Invoice: ...

    def get_overdue(self) -> List[Invoice]: ...

    def count(self) -> int: ...

# ============================================================================
# Bill of Materials
# ============================================================================

class Bom:
    """Bill of materials data."""

    id: str
    bom_number: str
    product_id: str
    status: str

class BomComponent:
    """BOM component data."""

    id: str
    bom_id: str
    component_sku: str
    quantity: float

class BomApi:
    """Bill of materials operations."""

    def create(self, product_id: str) -> Bom: ...

    def get(self, id: str) -> Optional[Bom]: ...

    def list(self) -> List[Bom]: ...

    def add_component(self, bom_id: str, component_sku: str, quantity: float) -> BomComponent: ...

    def get_components(self, bom_id: str) -> List[BomComponent]: ...

    def activate(self, id: str) -> Bom: ...

    def count(self) -> int: ...

# ============================================================================
# Work Orders
# ============================================================================

class WorkOrder:
    """Work order data."""

    id: str
    work_order_number: str
    status: str
    quantity_planned: float
    quantity_completed: float

class WorkOrders:
    """Work order operations."""

    def create(self, bom_id: str, quantity_planned: float) -> WorkOrder: ...

    def get(self, id: str) -> Optional[WorkOrder]: ...

    def list(
        self,
        product_id: Optional[str] = None,
        bom_id: Optional[str] = None,
        status: Optional[str] = None,
        priority: Optional[str] = None,
        assigned_to: Optional[str] = None,
        work_center_id: Optional[str] = None,
        overdue_only: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[WorkOrder]: ...

    def start(self, id: str) -> WorkOrder: ...

    def complete(self, id: str, quantity_completed: float) -> WorkOrder: ...

    def cancel(self, id: str) -> WorkOrder: ...

    def count(self) -> int: ...

# ============================================================================
# Carts
# ============================================================================

class CartAddress:
    """Cart address."""

    first_name: str
    last_name: str
    company: Optional[str]
    line1: str
    line2: Optional[str]
    city: str
    state: Optional[str]
    postal_code: str
    country: str
    phone: Optional[str]
    email: Optional[str]

class CartItem:
    """Cart item."""

    id: str
    sku: str
    name: str
    quantity: int
    unit_price: float
    unit_price_exact: str
    original_price_exact: Optional[str]
    discount_amount_exact: str
    tax_amount_exact: str
    total: float
    total_exact: str

class Cart:
    """Cart data."""

    id: str
    cart_number: str
    status: str
    currency: str
    subtotal_exact: str
    tax_amount_exact: str
    shipping_amount_exact: str
    discount_amount_exact: str
    grand_total: float
    grand_total_exact: str
    item_count: int

    @property
    def shipping_address(self) -> Optional[CartAddress]: ...

    @property
    def billing_address(self) -> Optional[CartAddress]: ...

    @property
    def items(self) -> List[CartItem]: ...

class AddCartItemInput:
    """Input for adding cart item."""

    def __init__(
        self,
        sku: str,
        name: str,
        quantity: int,
        unit_price: float,
        product_id: Optional[str] = None,
        variant_id: Optional[str] = None,
        description: Optional[str] = None,
        image_url: Optional[str] = None,
        original_price: Optional[float] = None,
        weight: Optional[float] = None,
        requires_shipping: Optional[bool] = None,
    ) -> None: ...

class ShippingRate:
    """Available shipping option."""

    id: str
    carrier: str
    service: str
    description: Optional[str]
    price: float
    currency: str
    estimated_days: Optional[int]

class CheckoutResult:
    """Checkout completion result."""

    cart_id: str
    order_id: str
    order_number: str
    payment_id: Optional[str]
    total_charged: float
    total_charged_exact: str
    currency: str

class Carts:
    """Cart and checkout operations."""

    def create(
        self,
        customer_id: Optional[str] = None,
        customer_email: Optional[str] = None,
        customer_name: Optional[str] = None,
        currency: Optional[str] = None,
        expires_in_minutes: Optional[int] = None,
    ) -> Cart: ...

    def get(self, id: str) -> Optional[Cart]: ...

    def get_by_number(self, cart_number: str) -> Optional[Cart]: ...

    def update(
        self,
        id: str,
        customer_email: Optional[str] = None,
        customer_phone: Optional[str] = None,
        customer_name: Optional[str] = None,
        shipping_method: Optional[str] = None,
        coupon_code: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> Cart: ...

    def list(self) -> List[Cart]: ...

    def for_customer(self, customer_id: str) -> List[Cart]: ...

    def delete(self, id: str) -> None: ...

    def add_item(self, cart_id: str, item: AddCartItemInput) -> CartItem: ...

    def update_item(self, item_id: str, quantity: Optional[int] = None) -> CartItem: ...

    def remove_item(self, item_id: str) -> None: ...

    def get_items(self, cart_id: str) -> List[CartItem]: ...

    def clear_items(self, cart_id: str) -> None: ...

    def set_shipping_address(self, id: str, address: CartAddress) -> Cart: ...

    def set_billing_address(self, id: str, address: CartAddress) -> Cart: ...

    def set_shipping(
        self,
        id: str,
        address: CartAddress,
        shipping_method: Optional[str] = None,
        shipping_carrier: Optional[str] = None,
        shipping_amount: Optional[float] = None,
    ) -> Cart: ...

    def get_shipping_rates(self, id: str) -> List[ShippingRate]: ...

    def set_payment(self, id: str, payment_method: str, payment_token: Optional[str] = None) -> Cart: ...

    def apply_discount(self, id: str, coupon_code: str) -> Cart: ...

    def remove_discount(self, id: str) -> Cart: ...

    def mark_ready_for_payment(self, id: str) -> Cart: ...

    def begin_checkout(self, id: str) -> Cart: ...

    def complete(self, id: str) -> CheckoutResult: ...

    def cancel(self, id: str) -> Cart: ...

    def abandon(self, id: str) -> Cart: ...

    def expire(self, id: str) -> Cart: ...

    def reserve_inventory(self, id: str) -> Cart: ...

    def release_inventory(self, id: str) -> Cart: ...

    def recalculate(self, id: str) -> Cart: ...

    def set_tax(self, id: str, tax_amount: float) -> Cart: ...

    def get_abandoned(self) -> List[Cart]: ...

    def get_expired(self) -> List[Cart]: ...

    def count(self) -> int: ...

# ============================================================================
# Analytics
# ============================================================================

class SalesSummary:
    total_revenue: float
    order_count: int
    average_order_value: float
    items_sold: int
    unique_customers: int

class RevenueByPeriod:
    period: str
    revenue: float
    order_count: int
    period_start: str

class TopProduct:
    product_id: Optional[str]
    sku: str
    name: str
    units_sold: int
    revenue: float
    order_count: int

class ProductPerformance:
    product_id: str
    sku: str
    name: str
    units_sold: int
    revenue: float
    previous_units_sold: int
    previous_revenue: float
    units_growth_percent: float
    revenue_growth_percent: float

class CustomerMetrics:
    total_customers: int
    new_customers: int
    returning_customers: int
    average_lifetime_value: float
    average_orders_per_customer: float

class TopCustomer:
    customer_id: str
    name: str
    email: str
    order_count: int
    total_spent: float
    average_order_value: float

class InventoryHealth:
    total_skus: int
    in_stock_skus: int
    low_stock_skus: int
    out_of_stock_skus: int
    total_value: float

class LowStockItem:
    sku: str
    name: str
    on_hand: float
    allocated: float
    available: float
    reorder_point: Optional[float]
    average_daily_sales: Optional[float]
    days_of_stock: Optional[float]

class InventoryMovement:
    sku: str
    name: str
    units_sold: int
    units_received: int
    units_returned: int
    units_adjusted: int
    net_change: int

class OrderStatusBreakdown:
    pending: int
    confirmed: int
    processing: int
    shipped: int
    delivered: int
    cancelled: int
    refunded: int

class FulfillmentMetrics:
    avg_time_to_ship_hours: Optional[float]
    avg_time_to_deliver_hours: Optional[float]
    on_time_shipping_percent: Optional[float]
    on_time_delivery_percent: Optional[float]
    shipped_today: int
    awaiting_shipment: int

class ReturnMetrics:
    total_returns: int
    return_rate_percent: float
    total_refunded: float

class DemandForecast:
    sku: str
    name: str
    average_daily_demand: float
    forecasted_demand: float
    confidence: float
    current_stock: float
    days_until_stockout: Optional[int]
    recommended_reorder_qty: Optional[float]
    trend: str

class RevenueForecast:
    period: str
    forecasted_revenue: float
    lower_bound: float
    upper_bound: float
    confidence_level: float
    based_on_periods: int

class Analytics:
    def sales_summary(self, period: Optional[str] = None, limit: Optional[int] = None) -> SalesSummary: ...

    def revenue_by_period(self, period: Optional[str] = None, granularity: Optional[str] = None) -> List[RevenueByPeriod]: ...

    def top_products(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[TopProduct]: ...

    def product_performance(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[ProductPerformance]: ...

    def customer_metrics(self, period: Optional[str] = None) -> CustomerMetrics: ...

    def top_customers(self, period: Optional[str] = None, limit: Optional[int] = None) -> List[TopCustomer]: ...

    def inventory_health(self) -> InventoryHealth: ...

    def low_stock_items(self, threshold: Optional[float] = None) -> List[LowStockItem]: ...

    def inventory_movement(self, period: Optional[str] = None) -> List[InventoryMovement]: ...

    def order_status_breakdown(self, period: Optional[str] = None) -> OrderStatusBreakdown: ...

    def fulfillment_metrics(self, period: Optional[str] = None) -> FulfillmentMetrics: ...

    def return_metrics(self, period: Optional[str] = None) -> ReturnMetrics: ...

    def demand_forecast(self, skus: Optional[List[str]] = None, days_ahead: Optional[int] = None) -> List[DemandForecast]: ...

    def revenue_forecast(self, periods_ahead: Optional[int] = None, granularity: Optional[str] = None) -> List[RevenueForecast]: ...

# ============================================================================
# Currency
# ============================================================================

class ExchangeRate:
    id: str
    base_currency: str
    quote_currency: str
    rate: float
    source: str
    rate_at: str
    created_at: str
    updated_at: str

class ConversionResult:
    original_amount: float
    original_currency: str
    converted_amount: float
    target_currency: str
    rate: float
    inverse_rate: float
    rate_at: str

class StoreCurrencySettings:
    base_currency: str
    enabled_currencies: List[str]
    auto_convert: bool
    rounding_mode: str

class SetExchangeRateInput:
    base_currency: str
    quote_currency: str
    rate: float
    source: Optional[str]

    def __init__(self, base_currency: str, quote_currency: str, rate: float, source: Optional[str] = None) -> None: ...

class CurrencyOperations:
    def get_rate(self, from_currency: str, to_currency: str) -> Optional[ExchangeRate]: ...

    def get_rates_for(self, base_currency: str) -> List[ExchangeRate]: ...

    def list_rates(self, base_currency: Optional[str] = None, quote_currency: Optional[str] = None) -> List[ExchangeRate]: ...

    def set_rate(self, base_currency: str, quote_currency: str, rate: float, source: Optional[str] = None) -> ExchangeRate: ...

    def set_rates(self, rates: List[SetExchangeRateInput]) -> List[ExchangeRate]: ...

    def delete_rate(self, id: str) -> None: ...

    def convert(self, from_currency: str, to_currency: str, amount: float) -> ConversionResult: ...

    def get_settings(self) -> StoreCurrencySettings: ...

    def update_settings(
        self,
        base_currency: str,
        enabled_currencies: List[str],
        auto_convert: Optional[bool] = None,
        rounding_mode: Optional[str] = None,
    ) -> StoreCurrencySettings: ...

    def set_base_currency(self, currency_code: str) -> StoreCurrencySettings: ...

    def enable_currencies(self, currency_codes: List[str]) -> StoreCurrencySettings: ...

    def is_enabled(self, currency_code: str) -> bool: ...

    def base_currency(self) -> str: ...

    def enabled_currencies(self) -> List[str]: ...

    def format(self, amount: float, currency_code: str) -> str: ...

# ============================================================================
# Vector Search
# ============================================================================

class ProductSearchResult:
    id: str
    name: str
    description: str
    distance: float
    score: float

class CustomerSearchResult:
    id: str
    name: str
    email: str
    distance: float
    score: float

class EmbeddingStats:
    product_count: int
    customer_count: int
    order_count: int
    inventory_count: int
    total_count: int
    model: str
    dimensions: int

class VectorSearch:
    def search_products(self, query: str, limit: Optional[int] = None) -> List[ProductSearchResult]: ...

    def search_customers(self, query: str, limit: Optional[int] = None) -> List[CustomerSearchResult]: ...

    def index_product(self, product_id: str) -> None: ...

    def index_customer(self, customer_id: str) -> None: ...

    def index_all_products(self) -> int: ...

    def index_all_customers(self) -> int: ...

    def stats(self) -> EmbeddingStats: ...

    def clear(self, entity_type: str) -> int: ...

    def clear_all(self) -> int: ...

# ============================================================================
# Accounts Payable
# ============================================================================

class Bill:
    id: str
    bill_number: str
    supplier_id: str
    total_amount: float
    amount_paid: float
    amount_due: float
    status: str
    due_date: str

class ApAgingSummary:
    current: float
    days_1_30: float
    days_31_60: float
    days_61_90: float
    days_over_90: float
    total: float

class ThreeWayMatchLine:
    """One line of a three-way match. Quantities/costs are decimal strings."""

    po_line_id: Optional[str]
    bill_item_id: str
    description: str
    ordered_quantity: Optional[str]
    ordered_unit_cost: Optional[str]
    received_quantity: str
    billed_quantity: str
    billed_unit_cost: str
    quantity_variance: str
    price_variance: str
    matched: bool
    issues: List[str]

class ThreeWayMatchResult:
    """Result of a three-way match run."""

    match_status: str
    variance_line_count: Optional[int]
    tolerance_percent: str
    lines: List[ThreeWayMatchLine]

class AccountsPayableApi:
    """Accounts payable operations."""

    def create_bill(self, supplier_id: str, due_date: str) -> Bill: ...

    def get_bill(self, id: str) -> Optional[Bill]: ...

    def list_bills(self) -> List[Bill]: ...

    def approve_bill(self, id: str) -> Bill: ...

    def pay_bill(self, id: str, amount: float, payment_method: Optional[str] = None) -> Bill: ...

    def get_aging_summary(self) -> ApAgingSummary: ...

    def get_overdue_bills(self) -> List[Bill]: ...

    def three_way_match(
        self, bill_id: str, tolerance_percent: Optional[str] = None
    ) -> ThreeWayMatchResult:
        """Three-way match a bill against its purchase order and receipts.

        `tolerance_percent` is an exact decimal string (e.g. "5" for 5%);
        omit it for exact matching.
        """
        ...

# ============================================================================
# General Ledger
# ============================================================================

class GlAccount:
    id: str
    account_number: str
    name: str
    account_type: str
    current_balance: float
    status: str

class JournalEntry:
    id: str
    entry_number: str
    description: str
    status: str
    entry_date: str

class TrialBalance:
    total_debits: float
    total_credits: float
    is_balanced: bool

class GlPeriod:
    """An accounting period. Dates are ISO strings (YYYY-MM-DD)."""

    id: str
    period_name: str
    fiscal_year: int
    period_number: int
    start_date: str
    end_date: str
    status: str
    closed_by: Optional[str]

class RevaluationLine:
    """One revalued account line. Money values are exact decimal strings."""

    account_id: str
    account_number: str
    account_name: str
    currency: str
    normal_balance: str
    foreign_balance: str
    carrying_value: str
    rate: str
    revalued_value: str
    adjustment: str
    unrealized_gain_loss: str

class RevaluationResult:
    """Result of an FX revaluation run."""

    as_of_date: str
    base_currency: str
    total_unrealized_gain_loss: str
    lines: List[RevaluationLine]
    journal_entry: Optional[JournalEntry]

class CloseMonthStep:
    """One step of a month-end close run."""

    status: str
    entry_count: int
    total_amount: str
    warnings: List[str]

class CloseMonthReport:
    """Report from a month-end close run."""

    period_id: str
    period_name: str
    dry_run: bool
    depreciation: CloseMonthStep
    revenue_recognition: CloseMonthStep
    fx_revaluation: CloseMonthStep
    period_close: CloseMonthStep
    closing_entry: Optional[JournalEntry]
    period_status: str

class GeneralLedgerApi:
    """General ledger operations."""

    def create_account(
        self,
        account_number: str,
        name: str,
        account_type: str,
        description: Optional[str] = None,
    ) -> GlAccount: ...

    def get_account(self, id: str) -> Optional[GlAccount]: ...

    def get_account_by_number(self, account_number: str) -> Optional[GlAccount]: ...

    def list_accounts(self) -> List[GlAccount]: ...

    def get_journal_entry(self, id: str) -> Optional[JournalEntry]: ...

    def post_journal_entry(self, id: str, posted_by: str) -> JournalEntry: ...

    def get_trial_balance(self, as_of_date: Optional[str] = None) -> TrialBalance: ...

    def initialize_chart_of_accounts(self) -> List[GlAccount]:
        """Initialize the standard chart of accounts."""
        ...

    def create_period(
        self,
        period_name: str,
        fiscal_year: int,
        period_number: int,
        start_date: str,
        end_date: str,
    ) -> GlPeriod:
        """Create an accounting period. Dates are ISO strings (YYYY-MM-DD)."""
        ...

    def open_period(self, id: str) -> GlPeriod:
        """Open a period (transition from future to open)."""
        ...

    def revalue(
        self, as_of_date: str, base_currency: Optional[str] = None
    ) -> RevaluationResult:
        """Revalue foreign-currency account balances at the as-of rate.

        `as_of_date` is an ISO date (YYYY-MM-DD); `base_currency` defaults
        to the store's configured base currency.
        """
        ...

    def close_month(
        self,
        period_id: str,
        dry_run: Optional[bool] = None,
        skip_depreciation: Optional[bool] = None,
        skip_revenue_recognition: Optional[bool] = None,
        skip_fx_revaluation: Optional[bool] = None,
        skip_period_close: Optional[bool] = None,
        closed_by: Optional[str] = None,
    ) -> CloseMonthReport:
        """Close the month: depreciation, revenue recognition, FX
        revaluation, then the period close. `dry_run=True` computes
        per-step counts and amounts without writing anything.
        """
        ...

# ============================================================================
# Fixed Assets
# ============================================================================

class AssetDisposal:
    """A recorded asset disposal. Money values are exact decimal strings."""

    disposal_date: str
    proceeds: str
    book_value_at_disposal: str
    gain_loss: str
    notes: Optional[str]

class FixedAsset:
    """A fixed asset. Money values are exact decimal strings."""

    id: str
    asset_number: str
    name: str
    description: Optional[str]
    category: str
    acquisition_date: str
    acquisition_cost: str
    salvage_value: str
    useful_life_months: int
    depreciation_method: str
    declining_balance_rate: Optional[str]
    status: str
    in_service_date: Optional[str]
    location_id: Optional[str]
    asset_account_id: Optional[str]
    accumulated_depreciation_account_id: Optional[str]
    depreciation_expense_account_id: Optional[str]
    accumulated_depreciation: str
    book_value: str
    currency: str
    disposal: Optional[AssetDisposal]
    created_at: str
    updated_at: str

class DepreciationEntry:
    """One period in a depreciation schedule."""

    period: int
    amount: str
    accumulated: str
    book_value: str
    status: str

class DepreciationSchedule:
    """A depreciation schedule for a fixed asset."""

    asset_id: str
    method: str
    declining_balance_rate: Optional[str]
    entries: List[DepreciationEntry]
    total_depreciation: str

class FixedAssets:
    """Fixed asset operations. Money is exchanged as exact decimal strings,
    dates as ISO strings (YYYY-MM-DD), enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        category: str,
        acquisition_date: str,
        acquisition_cost: str,
        salvage_value: str,
        useful_life_months: int,
        depreciation_method: str,
        asset_number: Optional[str] = None,
        description: Optional[str] = None,
        declining_balance_rate: Optional[str] = None,
        in_service_date: Optional[str] = None,
        location_id: Optional[str] = None,
        asset_account_id: Optional[str] = None,
        accumulated_depreciation_account_id: Optional[str] = None,
        depreciation_expense_account_id: Optional[str] = None,
        currency: Optional[str] = None,
    ) -> FixedAsset:
        """Create a fixed asset (draft)."""
        ...

    def get(self, id: str) -> Optional[FixedAsset]: ...

    def list(
        self,
        category: Optional[str] = None,
        status: Optional[str] = None,
        location_id: Optional[str] = None,
        acquired_from: Optional[str] = None,
        acquired_to: Optional[str] = None,
        search: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[FixedAsset]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
        category: Optional[str] = None,
        salvage_value: Optional[str] = None,
        useful_life_months: Optional[int] = None,
        in_service_date: Optional[str] = None,
        location_id: Optional[str] = None,
        asset_account_id: Optional[str] = None,
        accumulated_depreciation_account_id: Optional[str] = None,
        depreciation_expense_account_id: Optional[str] = None,
    ) -> FixedAsset: ...

    def place_in_service(self, id: str, date: str) -> FixedAsset:
        """Place a draft asset in service on the given ISO date."""
        ...

    def dispose(
        self,
        id: str,
        proceeds: str,
        date: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> FixedAsset:
        """Dispose of an asset for the given proceeds (decimal string)."""
        ...

    def write_off(
        self, id: str, date: Optional[str] = None, notes: Optional[str] = None
    ) -> FixedAsset:
        """Write off an asset (disposal with zero proceeds)."""
        ...

    def generate_schedule(self, id: str) -> DepreciationSchedule:
        """Generate and persist the depreciation schedule for an asset."""
        ...

    def get_schedule(self, id: str) -> Optional[DepreciationSchedule]:
        """Get the persisted depreciation schedule, if generated."""
        ...

    def post_depreciation(self, id: str, periods: int) -> FixedAsset:
        """Post the next `periods` scheduled depreciation entries."""
        ...

# ============================================================================
# Revenue Recognition (ASC 606)
# ============================================================================

class PerformanceObligationInput:
    """Input for a performance obligation under a revenue contract."""

    description: str
    allocated_amount: str
    recognition_method: str
    standalone_selling_price: Optional[str]
    recognition_start: Optional[str]
    recognition_end: Optional[str]

    def __init__(
        self,
        description: str,
        allocated_amount: str,
        recognition_method: str,
        standalone_selling_price: Optional[str] = None,
        recognition_start: Optional[str] = None,
        recognition_end: Optional[str] = None,
    ) -> None: ...

class PerformanceObligation:
    """A performance obligation. Money values are exact decimal strings."""

    id: str
    contract_id: str
    description: str
    standalone_selling_price: Optional[str]
    allocated_amount: str
    recognition_method: str
    recognition_start: Optional[str]
    recognition_end: Optional[str]
    recognized_amount: str
    deferred_amount: str
    created_at: str
    updated_at: str

class RevenueContract:
    """A revenue contract (ASC 606). Money values are decimal strings."""

    id: str
    contract_number: str
    customer_id: str
    order_id: Optional[str]
    invoice_id: Optional[str]
    transaction_price: str
    currency: str
    status: str
    effective_date: str
    obligations: List[PerformanceObligation]
    total_recognized: str
    deferred_balance: str
    created_at: str
    updated_at: str

class RevenueScheduleEntry:
    """One entry in a revenue recognition schedule."""

    period: int
    period_start: str
    amount: str
    status: str

class RevenueSchedule:
    """A revenue recognition schedule for an obligation."""

    obligation_id: str
    method: str
    recognition_start: Optional[str]
    recognition_end: Optional[str]
    entries: List[RevenueScheduleEntry]
    total_amount: str
    recognized_total: str
    deferred_total: str

class RevenueRecognition:
    """Revenue recognition (ASC 606) operations."""

    def is_supported(self) -> bool: ...

    def create_contract(
        self,
        customer_id: str,
        transaction_price: str,
        effective_date: str,
        obligations: List[PerformanceObligationInput],
        contract_number: Optional[str] = None,
        order_id: Optional[str] = None,
        invoice_id: Optional[str] = None,
        currency: Optional[str] = None,
    ) -> RevenueContract:
        """Create a revenue contract with its performance obligations."""
        ...

    def get_contract(self, id: str) -> Optional[RevenueContract]: ...

    def list_contracts(
        self,
        customer_id: Optional[str] = None,
        order_id: Optional[str] = None,
        invoice_id: Optional[str] = None,
        status: Optional[str] = None,
        effective_from: Optional[str] = None,
        effective_to: Optional[str] = None,
        search: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[RevenueContract]: ...

    def update_contract(
        self,
        id: str,
        order_id: Optional[str] = None,
        invoice_id: Optional[str] = None,
        status: Optional[str] = None,
        effective_date: Optional[str] = None,
    ) -> RevenueContract: ...

    def list_obligations(self, contract_id: str) -> List[PerformanceObligation]: ...

    def generate_schedule(self, obligation_id: str) -> RevenueSchedule:
        """Generate and persist the recognition schedule for an obligation."""
        ...

    def get_schedule(self, obligation_id: str) -> Optional[RevenueSchedule]:
        """Get the persisted recognition schedule, if generated."""
        ...

    def recognize(self, obligation_id: str, through: str) -> RevenueSchedule:
        """Recognize deferred entries with a period start on or before
        `through` (ISO date, YYYY-MM-DD)."""
        ...

# ============================================================================
# Cycle Counts
# ============================================================================

class CycleCountLineInput:
    """Input for an expected cycle count line."""

    sku: str
    expected_quantity: str
    lot_id: Optional[str]

    def __init__(
        self, sku: str, expected_quantity: str, lot_id: Optional[str] = None
    ) -> None: ...

class RecordCycleCountLineInput:
    """Input for recording a physical count against a line."""

    sku: str
    counted_quantity: str
    lot_id: Optional[str]

    def __init__(
        self, sku: str, counted_quantity: str, lot_id: Optional[str] = None
    ) -> None: ...

class CycleCountLine:
    """One line of a cycle count. Quantities are exact decimal strings."""

    id: str
    cycle_count_id: str
    sku: str
    lot_id: Optional[str]
    expected_quantity: str
    counted_quantity: Optional[str]
    variance: Optional[str]

class CycleCount:
    """A cycle count with its lines."""

    id: str
    warehouse_id: int
    location_id: Optional[int]
    status: str
    scheduled_date: Optional[str]
    counted_by: Optional[str]
    lines: List[CycleCountLine]
    created_at: str
    updated_at: str
    completed_at: Optional[str]

class CycleCounts:
    """Cycle count operations. Quantities are exact decimal strings."""

    def create(
        self,
        warehouse_id: int,
        lines: List[CycleCountLineInput],
        location_id: Optional[int] = None,
        scheduled_date: Optional[str] = None,
        counted_by: Optional[str] = None,
    ) -> CycleCount:
        """Create a cycle count (draft) with its expected lines."""
        ...

    def get(self, id: str) -> Optional[CycleCount]: ...

    def list(
        self,
        warehouse_id: Optional[int] = None,
        location_id: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[CycleCount]: ...

    def start(self, id: str) -> CycleCount:
        """Start a draft cycle count (draft -> in_progress)."""
        ...

    def record_counts(
        self, id: str, counts: List[RecordCycleCountLineInput]
    ) -> CycleCount:
        """Record physical counts against an in-progress cycle count."""
        ...

    def complete(self, id: str) -> CycleCount:
        """Complete an in-progress cycle count, applying variances."""
        ...

    def cancel(self, id: str) -> CycleCount:
        """Cancel a draft or in-progress cycle count."""
        ...

class Prepayment:
    """Cash paid to a supplier in advance. Money values are exact decimal
    strings."""

    id: str
    number: str
    supplier_id: str
    amount: str
    remaining: str
    currency: str
    status: str
    method: Optional[str]
    reference: Optional[str]
    memo: Optional[str]
    created_at: str
    updated_at: str

class PrepaymentApplication:
    """An application of a prepayment against a bill or payment obligation."""

    id: str
    prepayment_id: str
    target_type: str
    target_id: str
    amount: str
    reversed: bool
    created_at: str

class Prepayments:
    """Prepayment operations. Money is exchanged as exact decimal strings,
    enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        supplier_id: str,
        amount: str,
        currency: Optional[str] = None,
        method: Optional[str] = None,
        reference: Optional[str] = None,
        memo: Optional[str] = None,
    ) -> Prepayment: ...

    def get(self, id: str) -> Optional[Prepayment]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Prepayment]: ...

    def apply(
        self,
        id: str,
        target_type: str,
        target_id: str,
        amount: str,
    ) -> Prepayment:
        """Apply a prepayment against a bill or payment obligation."""
        ...

    def list_applications(self, id: str) -> List[PrepaymentApplication]: ...

    def reverse_application(self, id: str, application_id: str) -> Prepayment: ...

    def refund(self, id: str) -> Prepayment:
        """Refund the remaining balance, closing the prepayment."""
        ...

class VendorCredit:
    """A vendor credit balance owed by a supplier. Money values are exact
    decimal strings."""

    id: str
    number: str
    supplier_id: str
    vendor_return_id: Optional[str]
    amount: str
    remaining: str
    currency: str
    status: str
    memo: Optional[str]
    created_at: str
    updated_at: str

class VendorCreditApplication:
    """An application of a vendor credit against a bill or payment
    obligation."""

    id: str
    vendor_credit_id: str
    target_type: str
    target_id: str
    amount: str
    reversed: bool
    created_at: str

class VendorCredits:
    """Vendor credit operations. Money is exchanged as exact decimal strings,
    enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        supplier_id: str,
        amount: str,
        vendor_return_id: Optional[str] = None,
        currency: Optional[str] = None,
        memo: Optional[str] = None,
    ) -> VendorCredit: ...

    def get(self, id: str) -> Optional[VendorCredit]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[VendorCredit]: ...

    def apply(
        self,
        id: str,
        target_type: str,
        target_id: str,
        amount: str,
    ) -> VendorCredit:
        """Apply a vendor credit against a bill or payment obligation."""
        ...

    def list_applications(self, id: str) -> List[VendorCreditApplication]: ...

    def reverse_application(self, id: str, application_id: str) -> VendorCredit: ...

    def cancel(self, id: str) -> VendorCredit: ...

class PriceSchedule:
    """A time-bounded set of product price overrides."""

    id: str
    name: str
    code: Optional[str]
    currency: str
    starts_at: Optional[str]
    ends_at: Optional[str]
    is_active: bool
    priority: int
    created_at: str
    updated_at: str

class PriceScheduleEntry:
    """A per-product fixed price within a schedule."""

    price_schedule_id: str
    product_id: str
    price: str
    created_at: str
    updated_at: str

class PriceSchedules:
    """Price schedule operations. Money is exchanged as exact decimal
    strings, timestamps as RFC 3339 strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        code: Optional[str] = None,
        currency: Optional[str] = None,
        starts_at: Optional[str] = None,
        ends_at: Optional[str] = None,
        priority: int = 0,
    ) -> PriceSchedule: ...

    def get(self, id: str) -> Optional[PriceSchedule]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        code: Optional[str] = None,
        starts_at: Optional[str] = None,
        ends_at: Optional[str] = None,
        is_active: Optional[bool] = None,
        priority: Optional[int] = None,
    ) -> PriceSchedule: ...

    def list(
        self,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[PriceSchedule]: ...

    def delete(self, id: str) -> None: ...

    def set_entry(self, id: str, product_id: str, price: str) -> PriceScheduleEntry: ...

    def delete_entry(self, id: str, product_id: str) -> None: ...

    def list_entries(self, id: str) -> List[PriceScheduleEntry]: ...

    def resolve_price(self, product_id: str, at: Optional[str] = None) -> Optional[str]:
        """Resolve the effective scheduled price for a product at an instant
        (RFC 3339; defaults to now)."""
        ...

class PriceLevel:
    """A named B2B pricing tier."""

    id: str
    name: str
    code: str
    description: Optional[str]
    adjustment_type: str
    adjustment_value: str
    currency: str
    is_active: bool
    created_at: str
    updated_at: str

class PriceLevelEntry:
    """An explicit fixed price for a product within a price level."""

    price_level_id: str
    product_id: str
    price: str
    created_at: str
    updated_at: str

class PriceLevels:
    """Price level operations. Money is exchanged as exact decimal strings,
    enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        code: str,
        description: Optional[str] = None,
        adjustment_type: Optional[str] = None,
        adjustment_value: Optional[str] = None,
        currency: Optional[str] = None,
    ) -> PriceLevel: ...

    def get(self, id: str) -> Optional[PriceLevel]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
        adjustment_type: Optional[str] = None,
        adjustment_value: Optional[str] = None,
        is_active: Optional[bool] = None,
    ) -> PriceLevel: ...

    def list(
        self,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[PriceLevel]: ...

    def delete(self, id: str) -> None: ...

    def set_entry(self, id: str, product_id: str, price: str) -> PriceLevelEntry: ...

    def delete_entry(self, id: str, product_id: str) -> None: ...

    def list_entries(self, id: str) -> List[PriceLevelEntry]: ...

class TransferOrderItemInput:
    """A line on a create-transfer-order request."""

    product_id: str
    quantity: str

    def __init__(self, product_id: str, quantity: str) -> None: ...

class TransferOrderItem:
    """A single line on a transfer order. Quantities are exact decimal
    strings."""

    id: str
    transfer_order_id: str
    product_id: str
    sku: str
    quantity: str
    quantity_shipped: str
    quantity_received: str

class TransferOrder:
    """A transfer order moving stock between two warehouses."""

    id: str
    number: str
    source_warehouse_id: str
    destination_warehouse_id: str
    status: str
    items: List[TransferOrderItem]
    expected_at: Optional[str]
    shipped_at: Optional[str]
    received_at: Optional[str]
    notes: Optional[str]
    created_at: str
    updated_at: str

class TransferOrders:
    """Transfer order operations. Quantities are exact decimal strings,
    timestamps RFC 3339 strings, enums snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        source_warehouse_id: str,
        destination_warehouse_id: str,
        items: List[TransferOrderItemInput],
        expected_at: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> TransferOrder: ...

    def get(self, id: str) -> Optional[TransferOrder]: ...

    def list(
        self,
        status: Optional[str] = None,
        source_warehouse_id: Optional[str] = None,
        destination_warehouse_id: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[TransferOrder]: ...

    def ship(self, id: str) -> TransferOrder:
        """Mark a transfer order as shipped from the source."""
        ...

    def receive_line(self, id: str, item_id: str, quantity: str) -> TransferOrder:
        """Receive a quantity against a single line at the destination."""
        ...

    def cancel(self, id: str) -> TransferOrder: ...

class ProductionBatch:
    """A batch grouping multiple work orders for coordinated production."""

    id: str
    name: str
    status: str
    vendor_id: Optional[str]
    work_order_ids: List[str]
    notes: Optional[str]
    scheduled_start: Optional[str]
    scheduled_end: Optional[str]
    created_at: str
    updated_at: str

class ProductionBatches:
    """Production batch operations. Timestamps are RFC 3339 strings, enums
    snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        vendor_id: Optional[str] = None,
        work_order_ids: Optional[List[str]] = None,
        notes: Optional[str] = None,
        scheduled_start: Optional[str] = None,
        scheduled_end: Optional[str] = None,
    ) -> ProductionBatch: ...

    def get(self, id: str) -> Optional[ProductionBatch]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        vendor_id: Optional[str] = None,
        status: Optional[str] = None,
        notes: Optional[str] = None,
        scheduled_start: Optional[str] = None,
        scheduled_end: Optional[str] = None,
    ) -> ProductionBatch: ...

    def list(
        self,
        status: Optional[str] = None,
        vendor_id: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[ProductionBatch]: ...

    def delete(self, id: str) -> None: ...

    def add_work_orders(self, id: str, work_order_ids: List[str]) -> ProductionBatch:
        """Link work orders to a batch."""
        ...

    def remove_work_order(self, id: str, work_order_id: str) -> ProductionBatch:
        """Remove a work order from a batch."""
        ...

class SupplierSkuBulkItemInput:
    """A single item in a bulk supplier-SKU upsert."""

    product_id: str
    sku: str
    unit_cost: Optional[str]

    def __init__(
        self,
        product_id: str,
        sku: str,
        unit_cost: Optional[str] = None,
    ) -> None: ...

class SupplierSku:
    """A supplier-specific SKU and optional unit-cost override for a
    product. Money values are exact decimal strings."""

    id: str
    product_id: str
    supplier_id: str
    sku: str
    unit_cost: Optional[str]
    currency: str
    min_order_qty: Optional[str]
    lead_time_days: Optional[int]
    is_preferred: bool
    created_at: str
    updated_at: str

class SupplierSkus:
    """Supplier SKU operations. Money is exchanged as exact decimal
    strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        product_id: str,
        supplier_id: str,
        sku: str,
        unit_cost: Optional[str] = None,
        currency: Optional[str] = None,
        min_order_qty: Optional[str] = None,
        lead_time_days: Optional[int] = None,
    ) -> SupplierSku: ...

    def get(self, id: str) -> Optional[SupplierSku]: ...

    def update(
        self,
        id: str,
        sku: Optional[str] = None,
        unit_cost: Optional[str] = None,
        currency: Optional[str] = None,
        min_order_qty: Optional[str] = None,
        lead_time_days: Optional[int] = None,
        is_preferred: Optional[bool] = None,
    ) -> SupplierSku: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        product_id: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[SupplierSku]: ...

    def delete(self, id: str) -> None: ...

    def bulk_upsert(
        self,
        supplier_id: str,
        items: List[SupplierSkuBulkItemInput],
    ) -> int:
        """Bulk upsert supplier SKUs for a supplier; returns the count."""
        ...

class InboundShipmentItemInput:
    """A line on a create-inbound-shipment request."""

    product_id: str
    sku: str
    quantity_expected: str

    def __init__(self, product_id: str, sku: str, quantity_expected: str) -> None: ...

class InboundShipmentItem:
    """A single expected line on an inbound shipment. Quantities are exact
    decimal strings."""

    id: str
    inbound_shipment_id: str
    product_id: str
    sku: str
    quantity_expected: str
    quantity_received: str

class InboundShipment:
    """Goods in transit from a supplier to a warehouse."""

    id: str
    number: str
    supplier_id: str
    purchase_order_id: Optional[str]
    warehouse_id: Optional[str]
    carrier: Optional[str]
    tracking_number: Optional[str]
    status: str
    items: List[InboundShipmentItem]
    expected_at: Optional[str]
    received_at: Optional[str]
    notes: Optional[str]
    created_at: str
    updated_at: str

class InboundShipments:
    """Inbound shipment (ASN) operations. Quantities are exact decimal
    strings, timestamps RFC 3339 strings, enums snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        supplier_id: str,
        items: List[InboundShipmentItemInput],
        purchase_order_id: Optional[str] = None,
        warehouse_id: Optional[str] = None,
        carrier: Optional[str] = None,
        tracking_number: Optional[str] = None,
        expected_at: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> InboundShipment: ...

    def get(self, id: str) -> Optional[InboundShipment]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        warehouse_id: Optional[str] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[InboundShipment]: ...

    def mark_in_transit(self, id: str) -> InboundShipment: ...

    def mark_arrived(self, id: str) -> InboundShipment: ...

    def receive_line(self, id: str, item_id: str, quantity: str) -> InboundShipment:
        """Receive a quantity against a single line."""
        ...

    def cancel(self, id: str) -> InboundShipment: ...


# ============================================================================
# activity_logs
# ============================================================================

class ActivityLogEntry:
    """A single append-only activity log entry for a subject record."""

    @property
    def id(self) -> str: ...
    @property
    def subject_type(self) -> str: ...
    @property
    def subject_id(self) -> str: ...
    @property
    def action(self) -> str: ...
    @property
    def summary(self) -> str: ...
    @property
    def actor_kind(self) -> str:
        """user, system, integration, agent"""
    @property
    def actor(self) -> Optional[str]: ...
    @property
    def metadata(self) -> str:
        """Metadata as a JSON string"""
    @property
    def created_at(self) -> str:
        """RFC3339 timestamp"""

class ActivityLogs:
    """Activity log operations. Metadata crosses as a JSON string, enums as
    snake_case strings, timestamps as RFC3339 strings."""

    def is_supported(self) -> bool: ...

    def record(
        self,
        subject_type: str,
        subject_id: str,
        action: str,
        summary: str,
        actor_kind: Optional[str] = None,
        actor: Optional[str] = None,
        metadata: Optional[str] = None,
    ) -> ActivityLogEntry:
        """Record an activity log entry."""

    def get(self, id: str) -> Optional[ActivityLogEntry]:
        """Get an activity log entry by ID."""

    def list(
        self,
        subject_type: Optional[str] = None,
        subject_id: Optional[str] = None,
        action: Optional[str] = None,
        actor_kind: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[ActivityLogEntry]:
        """List activity log entries, most recent first."""

    def history_for_subject(
        self, subject_type: str, subject_id: str
    ) -> List[ActivityLogEntry]:
        """Full history for a single subject, most recent first."""


# ============================================================================
# channels
# ============================================================================

class Channel:
    """A sales / fulfillment channel."""

    @property
    def id(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def channel_type(self) -> str: ...
    @property
    def integration(self) -> Optional[str]: ...
    @property
    def status(self) -> str: ...
    @property
    def api_locked(self) -> bool: ...
    @property
    def default_warehouse_id(self) -> Optional[str]: ...
    @property
    def tags(self) -> List[str]: ...
    @property
    def metadata(self) -> str: ...
    @property
    def created_at(self) -> str: ...
    @property
    def updated_at(self) -> str: ...

class ChannelProductMapping:
    """A mapping between a channel-specific SKU and an internal product SKU."""

    @property
    def channel_id(self) -> str: ...
    @property
    def channel_sku(self) -> str: ...
    @property
    def product_id(self) -> str: ...
    @property
    def internal_sku(self) -> str: ...
    @property
    def created_at(self) -> str: ...
    @property
    def updated_at(self) -> str: ...

class ChannelProductSyncItem:
    """One item in a bulk channel product sync request."""

    def __init__(
        self,
        channel_sku: str,
        product_id: Optional[str] = None,
        internal_sku: Optional[str] = None,
        delete: bool = False,
    ) -> None: ...
    channel_sku: str
    product_id: Optional[str]
    internal_sku: Optional[str]
    delete: bool

class Channels:
    """Sales / fulfillment channel operations. Enums cross as snake_case
    strings, metadata as JSON strings, timestamps as RFC3339 strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        channel_type: str,
        integration: Optional[str] = None,
        default_warehouse_id: Optional[str] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[str] = None,
    ) -> Channel: ...

    def get(self, id: str) -> Optional[Channel]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        integration: Optional[str] = None,
        status: Optional[str] = None,
        default_warehouse_id: Optional[str] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[str] = None,
    ) -> Channel: ...

    def list(
        self,
        channel_type: Optional[str] = None,
        status: Optional[str] = None,
        integration: Optional[str] = None,
        api_locked: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Channel]: ...

    def delete(self, id: str) -> None: ...

    def set_lock(self, id: str, locked: bool) -> Channel: ...

    def sync_products(self, id: str, items: List[ChannelProductSyncItem]) -> int: ...

    def list_product_mappings(self, id: str) -> List[ChannelProductMapping]: ...


# ============================================================================
# companies
# ============================================================================

class Company:
    """A B2B company / account. Metadata crosses as a JSON string."""

    id: str
    name: str
    reference: Optional[str]
    email: Optional[str]
    phone: Optional[str]
    currency: str
    payment_terms_days: Optional[int]
    status: str
    tags: List[str]
    metadata: str
    created_at: str
    updated_at: str

class CompanyShippingAddress:
    """A shipping address belonging to a company."""

    id: str
    company_id: str
    label: Optional[str]
    name: Optional[str]
    line1: str
    line2: Optional[str]
    city: str
    region: Optional[str]
    postal_code: Optional[str]
    country: str
    is_default: bool
    created_at: str
    updated_at: str

class CompanyPriceOverride:
    """A company-specific product price override."""

    company_id: str
    product_id: str
    price: str
    currency: str
    created_at: str
    updated_at: str

class Contact:
    """A contact associated with one or more companies."""

    id: str
    first_name: str
    last_name: Optional[str]
    email: Optional[str]
    phone: Optional[str]
    title: Optional[str]
    company_ids: List[str]
    portal_enabled: bool
    is_active: bool
    created_at: str
    updated_at: str

class Companies:
    """B2B company (account) operations."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        reference: Optional[str] = None,
        email: Optional[str] = None,
        phone: Optional[str] = None,
        currency: Optional[str] = None,
        payment_terms_days: Optional[int] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[str] = None,
    ) -> Company:
        """Create a company."""
        ...

    def get(self, id: str) -> Optional[Company]:
        """Get a company by ID."""
        ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        reference: Optional[str] = None,
        email: Optional[str] = None,
        phone: Optional[str] = None,
        currency: Optional[str] = None,
        payment_terms_days: Optional[int] = None,
        status: Optional[str] = None,
        tags: Optional[List[str]] = None,
        metadata: Optional[str] = None,
    ) -> Company:
        """Update a company (partial update)."""
        ...

    def list(
        self,
        status: Optional[str] = None,
        search: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[Company]:
        """List companies matching the filter."""
        ...

    def delete(self, id: str) -> None:
        """Delete a company."""
        ...

    def list_addresses(self, id: str) -> List[CompanyShippingAddress]:
        """List a company's shipping addresses."""
        ...

    def list_price_overrides(self, id: str) -> List[CompanyPriceOverride]:
        """List a company's product price overrides."""
        ...

    def create_contact(
        self,
        first_name: str,
        last_name: Optional[str] = None,
        email: Optional[str] = None,
        phone: Optional[str] = None,
        title: Optional[str] = None,
        company_ids: Optional[List[str]] = None,
    ) -> Contact:
        """Create a contact linked to one or more companies."""
        ...

    def get_contact(self, id: str) -> Optional[Contact]:
        """Get a contact by ID."""
        ...

    def list_contacts(self, company_id: str) -> List[Contact]:
        """List contacts for a company."""
        ...


# ============================================================================
# units_of_measure
# ============================================================================

class UnitClass:
    """A class of mutually-convertible units (e.g. Weight, Length)."""

    id: str
    name: str
    description: str | None
    base_uom_id: str | None
    created_at: str
    updated_at: str

class UnitOfMeasure:
    """A unit of measure within a unit class. `factor` is an exact decimal string."""

    id: str
    unit_class_id: str
    name: str
    abbreviation: str
    factor: str
    is_base: bool
    created_at: str
    updated_at: str

class UnitConversionRule:
    """An explicit conversion rule between two units of measure."""

    id: str
    rule_type: str
    product_id: str | None
    from_uom_id: str
    to_uom_id: str
    factor: str
    created_at: str
    updated_at: str

class UnitsOfMeasure:
    """Units of measure operations: unit classes, units, and conversion rules."""

    def is_supported(self) -> bool: ...

    def create_class(
        self,
        name: str,
        description: str | None = None,
    ) -> UnitClass: ...

    def list_classes(self) -> list[UnitClass]: ...

    def delete_class(self, id: str) -> None: ...

    def create_uom(
        self,
        unit_class_id: str,
        name: str,
        abbreviation: str,
        factor: str,
    ) -> UnitOfMeasure: ...

    def list_uoms(
        self,
        class_id: str | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[UnitOfMeasure]: ...

    def set_base_uom(self, id: str) -> UnitOfMeasure: ...

    def delete_uom(self, id: str) -> None: ...

    def create_rule(
        self,
        rule_type: str,
        from_uom_id: str,
        to_uom_id: str,
        factor: str,
        product_id: str | None = None,
    ) -> UnitConversionRule: ...

    def list_rules(self) -> list[UnitConversionRule]: ...

    def delete_rule(self, id: str) -> None: ...


# ============================================================================
# shipping_zones
# ============================================================================

class ShippingZone:
    id: str
    name: str
    countries: list[str]
    regions: list[str]
    postal_codes: list[str]
    priority: int
    is_active: bool
    created_at: str
    updated_at: str

class ShippingCondition:
    min_weight: str | None
    max_weight: str | None
    min_price: str | None
    max_price: str | None
    rate: str
    def __init__(
        self,
        rate: str,
        min_weight: str | None = None,
        max_weight: str | None = None,
        min_price: str | None = None,
        max_price: str | None = None,
    ) -> None: ...

class ZoneShippingMethod:
    id: str
    zone_id: str
    name: str
    carrier: str | None
    method_type: str
    base_rate: str
    currency: str
    min_delivery_days: int | None
    max_delivery_days: int | None
    conditions: list[ShippingCondition]
    is_active: bool
    created_at: str
    updated_at: str

class ZoneShippingRate:
    method_id: str
    method_name: str
    carrier: str | None
    rate: str
    currency: str
    min_delivery_days: int | None
    max_delivery_days: int | None

class ShippingZones:
    def is_supported(self) -> bool: ...
    def create(
        self,
        name: str,
        countries: list[str] | None = None,
        regions: list[str] | None = None,
        postal_codes: list[str] | None = None,
        priority: int | None = None,
    ) -> ShippingZone: ...
    def get(self, id: str) -> ShippingZone | None: ...
    def update(
        self,
        id: str,
        name: str | None = None,
        countries: list[str] | None = None,
        regions: list[str] | None = None,
        postal_codes: list[str] | None = None,
        priority: int | None = None,
        is_active: bool | None = None,
    ) -> ShippingZone: ...
    def list(
        self,
        country: str | None = None,
        is_active: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[ShippingZone]: ...
    def delete(self, id: str) -> None: ...
    def find_matching_zones(
        self,
        country: str,
        region: str | None = None,
        postal_code: str | None = None,
    ) -> list[ShippingZone]: ...
    def create_method(
        self,
        zone_id: str,
        name: str,
        method_type: str,
        base_rate: str,
        currency: str,
        carrier: str | None = None,
        min_delivery_days: int | None = None,
        max_delivery_days: int | None = None,
        conditions: list[ShippingCondition] | None = None,
    ) -> ZoneShippingMethod: ...
    def get_method(self, id: str) -> ZoneShippingMethod | None: ...
    def list_methods(
        self,
        zone_id: str | None = None,
        carrier: str | None = None,
        method_type: str | None = None,
        is_active: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[ZoneShippingMethod]: ...
    def delete_method(self, id: str) -> None: ...
    def calculate_rates(
        self,
        country: str,
        currency: str,
        region: str | None = None,
        postal_code: str | None = None,
        weight: str | None = None,
        order_total: str | None = None,
    ) -> list[ZoneShippingRate]: ...


# ============================================================================
# stock_snapshots
# ============================================================================

class CaptureStockLineInput:
    """A line on a capture-stock-snapshot request. Quantities are exact
    decimal strings."""

    product_id: str
    sku: str
    quantity_on_hand: str
    quantity_available: str
    location: Optional[str]

    def __init__(
        self,
        product_id: str,
        sku: str,
        quantity_on_hand: str,
        quantity_available: str,
        location: Optional[str] = None,
    ) -> None: ...

class StockSnapshotLine:
    """A single per-SKU line within a stock snapshot."""

    id: str
    stock_snapshot_id: str
    product_id: str
    sku: str
    quantity_on_hand: str
    quantity_available: str
    location: Optional[str]

class StockSnapshot:
    """A point-in-time inventory snapshot."""

    id: str
    label: Optional[str]
    total_skus: int
    total_units: str
    lines: List[StockSnapshotLine]
    captured_at: str

class StockSnapshots:
    """Stock snapshot operations. Quantities are exchanged as exact decimal
    strings, timestamps as RFC3339 strings."""

    def is_supported(self) -> bool: ...
    def capture(
        self,
        lines: List[CaptureStockLineInput],
        label: Optional[str] = None,
    ) -> StockSnapshot:
        """Capture a new snapshot; totals are computed from the lines."""
        ...

    def get(self, id: str) -> Optional[StockSnapshot]: ...
    def latest(self) -> Optional[StockSnapshot]:
        """Most recent snapshot, if any."""
        ...

    def list(
        self,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[StockSnapshot]: ...
    def delete(self, id: str) -> None: ...


# ============================================================================
# print_stations
# ============================================================================

class PrintStation:
    """A paired print station. Timestamps are RFC3339 strings."""

    id: str
    name: str
    printers: List[str]
    revoked: bool
    last_seen_at: Optional[str]
    created_at: str
    updated_at: str

class PairStationResult:
    """Result of pairing a station: the station plus its one-time bearer token."""

    station: PrintStation
    token: str

class PrintJob:
    """A print job queued to a station."""

    id: str
    station_id: str
    printer_name: Optional[str]
    payload_kind: str
    payload: str
    status: str
    created_at: str
    picked_up_at: Optional[str]

class PrintStations:
    """Print station operations (paired agents + print job queue)."""

    def is_supported(self) -> bool: ...

    def pair(
        self,
        name: str,
        printers: Optional[List[str]] = None,
    ) -> PairStationResult:
        """Pair a new station, returning the station and its one-time token."""
        ...

    def list_stations(self) -> List[PrintStation]: ...

    def get_station(self, id: str) -> Optional[PrintStation]: ...

    def revoke_station(self, id: str) -> PrintStation: ...

    def enqueue_job(
        self,
        station_id: str,
        payload: str,
        printer_name: Optional[str] = None,
        payload_kind: Optional[str] = None,
    ) -> PrintJob:
        """Enqueue a print job. payload_kind is zpl (default) or pdf."""
        ...

    def next_job(self, station_id: str) -> Optional[PrintJob]: ...

    def complete_job(self, job_id: str, success: bool) -> PrintJob: ...

    def list_jobs(
        self,
        station_id: str,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[PrintJob]: ...


# ============================================================================
# integration_mappings
# ============================================================================

class CreateIntegrationMappingInput:
    """Input for creating an integration mapping (used with `bulk_upsert`)."""

    integration: str
    mapping_group: str
    field_name: str
    external_value: str
    internal_value: str

    def __init__(
        self,
        integration: str,
        mapping_group: str,
        field_name: str,
        external_value: str,
        internal_value: str,
    ) -> None: ...

class IntegrationMapping:
    """A single external -> internal value mapping for an integration."""

    @property
    def id(self) -> str: ...
    @property
    def integration(self) -> str: ...
    @property
    def mapping_group(self) -> str: ...
    @property
    def field_name(self) -> str: ...
    @property
    def external_value(self) -> str: ...
    @property
    def internal_value(self) -> str: ...
    @property
    def is_active(self) -> bool: ...
    @property
    def created_at(self) -> str: ...
    @property
    def updated_at(self) -> str: ...

class IntegrationMappings:
    """Integration mapping operations: translate external system values into
    canonical internal values for a given integration and mapping group."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        integration: str,
        mapping_group: str,
        field_name: str,
        external_value: str,
        internal_value: str,
    ) -> IntegrationMapping:
        """Create an integration mapping."""

    def get(self, id: str) -> Optional[IntegrationMapping]:
        """Get an integration mapping by ID."""

    def update(
        self,
        id: str,
        internal_value: Optional[str] = None,
        is_active: Optional[bool] = None,
    ) -> IntegrationMapping:
        """Update an integration mapping (partial)."""

    def list(
        self,
        integration: Optional[str] = None,
        mapping_group: Optional[str] = None,
        field_name: Optional[str] = None,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[IntegrationMapping]:
        """List integration mappings matching the filter."""

    def delete(self, id: str) -> None:
        """Delete an integration mapping."""

    def bulk_upsert(self, items: List[CreateIntegrationMappingInput]) -> str:
        """Bulk upsert mappings; returns the number of rows affected as a string."""

    def resolve(
        self,
        integration: str,
        mapping_group: str,
        field_name: str,
        external_value: str,
    ) -> Optional[str]:
        """Resolve the internal value for an external value."""


# ============================================================================
# integration_field_mappings
# ============================================================================

class IntegrationFieldMapping:
    """A field-path mapping for an integration account."""

    id: str
    integration_account: str
    mapping_group: str
    source_field: str
    destination_field: str
    template: Optional[str]
    transform: str
    fallback: Optional[str]
    is_active: bool
    created_at: str
    updated_at: str

class NewIntegrationFieldMapping:
    """A field mapping to create, used for bulk creation."""

    integration_account: str
    mapping_group: str
    source_field: str
    destination_field: str
    template: Optional[str]
    transform: Optional[str]
    fallback: Optional[str]

    def __init__(
        self,
        integration_account: str,
        mapping_group: str,
        source_field: str,
        destination_field: str,
        template: Optional[str] = None,
        transform: Optional[str] = None,
        fallback: Optional[str] = None,
    ) -> None: ...

class IntegrationFieldMappings:
    """Integration field-mapping operations. Enums cross as snake_case strings,
    timestamps as RFC3339 strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        integration_account: str,
        mapping_group: str,
        source_field: str,
        destination_field: str,
        template: Optional[str] = None,
        transform: Optional[str] = None,
        fallback: Optional[str] = None,
    ) -> IntegrationFieldMapping:
        """Create a field mapping."""
        ...

    def get(self, id: str) -> Optional[IntegrationFieldMapping]: ...

    def update(
        self,
        id: str,
        destination_field: Optional[str] = None,
        template: Optional[str] = None,
        transform: Optional[str] = None,
        fallback: Optional[str] = None,
        is_active: Optional[bool] = None,
    ) -> IntegrationFieldMapping: ...

    def list(
        self,
        integration_account: Optional[str] = None,
        mapping_group: Optional[str] = None,
        source_field: Optional[str] = None,
        is_active: Optional[bool] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[IntegrationFieldMapping]: ...

    def delete(self, id: str) -> None: ...

    def bulk_create(self, items: List[NewIntegrationFieldMapping]) -> int:
        """Bulk create field mappings; returns the number of rows affected."""
        ...

    def bulk_delete(self, ids: List[str]) -> int:
        """Bulk delete field mappings by ID; returns the number of rows affected."""
        ...

    def distinct_groups(self, integration_account: str) -> List[str]:
        """Distinct mapping groups for an integration account."""
        ...


# ============================================================================
# payment_obligations
# ============================================================================

class PaymentObligation:
    """A scheduled amount owed to a supplier. Money values are exact decimal
    strings; dates are ISO strings (YYYY-MM-DD)."""

    id: str
    number: str
    supplier_id: str
    purchase_order_id: Optional[str]
    amount: str
    amount_paid: str
    outstanding: str
    currency: str
    due_date: str
    status: str
    linked_bill_ids: List[str]
    notes: Optional[str]
    created_at: str
    updated_at: str

class PaymentObligationDashboard:
    """Aggregate summary across payment obligations."""

    open_count: int
    total_outstanding: str
    overdue_count: int
    overdue_amount: str

class PaymentObligations:
    """Payment obligation operations. Money is exchanged as exact decimal
    strings, dates as ISO strings (YYYY-MM-DD), enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        supplier_id: str,
        amount: str,
        due_date: str,
        purchase_order_id: Optional[str] = None,
        currency: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> PaymentObligation: ...

    def get(self, id: str) -> Optional[PaymentObligation]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        status: Optional[str] = None,
        due_before: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[PaymentObligation]: ...

    def record_payment(self, id: str, amount: str) -> PaymentObligation: ...

    def set_status(self, id: str, status: str) -> PaymentObligation: ...

    def link_bill(self, id: str, bill_id: str) -> PaymentObligation: ...

    def dashboard(self, today: str) -> PaymentObligationDashboard: ...


# ============================================================================
# purgatory
# ============================================================================

class IngestLineItemInput:
    external_sku: str
    quantity: str
    product_id: str | None
    def __init__(
        self,
        external_sku: str,
        quantity: str,
        product_id: str | None = None,
    ) -> None: ...

class PurgatoryLineItem:
    @property
    def id(self) -> str: ...
    @property
    def purgatory_order_id(self) -> str: ...
    @property
    def external_sku(self) -> str: ...
    @property
    def product_id(self) -> str | None: ...
    @property
    def quantity(self) -> str: ...
    @property
    def ignore_item(self) -> bool: ...
    @property
    def non_physical(self) -> bool: ...
    @property
    def is_resolved(self) -> bool: ...

class PurgatoryOrder:
    @property
    def id(self) -> str: ...
    @property
    def channel_id(self) -> str | None: ...
    @property
    def external_order_id(self) -> str: ...
    @property
    def external_status(self) -> str | None: ...
    @property
    def is_posted(self) -> bool: ...
    @property
    def hold_reason(self) -> str | None: ...
    @property
    def metadata(self) -> str: ...
    @property
    def items(self) -> list[PurgatoryLineItem]: ...
    @property
    def is_ready_to_post(self) -> bool: ...
    @property
    def unresolved_count(self) -> str: ...
    @property
    def created_at(self) -> str: ...
    @property
    def updated_at(self) -> str: ...

class Purgatory:
    def is_supported(self) -> bool: ...
    def ingest(
        self,
        external_order_id: str,
        items: list[IngestLineItemInput],
        channel_id: str | None = None,
        external_status: str | None = None,
        metadata: str | None = None,
    ) -> PurgatoryOrder: ...
    def get(self, id: str) -> PurgatoryOrder | None: ...
    def list(
        self,
        channel_id: str | None = None,
        is_posted: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[PurgatoryOrder]: ...
    def map_line(
        self,
        id: str,
        line_id: str,
        product_id: str | None = None,
        ignore_item: bool | None = None,
        non_physical: bool | None = None,
    ) -> PurgatoryOrder: ...
    def post(self, id: str) -> PurgatoryOrder: ...
    def delete(self, id: str) -> None: ...


# ============================================================================
# topology_snapshots
# ============================================================================

class TopologySnapshot:
    """A captured operational topology snapshot."""

    id: str
    channels_total: int
    channels_active: int
    warehouses_total: int
    products_total: int
    open_orders: int
    health: str
    signals: str
    captured_at: str

class TopologySnapshots:
    """Operational topology snapshot operations. Counts cross as integers,
    signals as a JSON string, enums as snake_case strings."""

    def is_supported(self) -> bool: ...

    def capture(
        self,
        channels_total: int,
        channels_active: int,
        warehouses_total: int,
        products_total: int,
        open_orders: int,
        signals: Optional[str] = None,
    ) -> TopologySnapshot:
        """Capture a new snapshot; health is derived from the metrics."""
        ...

    def get(self, id: str) -> Optional[TopologySnapshot]: ...

    def latest(self) -> Optional[TopologySnapshot]: ...

    def list(
        self,
        health: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[TopologySnapshot]:
        """List snapshots, newest first, with optional health filtering."""
        ...

    def delete(self, id: str) -> None: ...


# ============================================================================
# vendor_returns
# ============================================================================

class VendorReturnItemInput:
    """A line on a create-vendor-return request."""

    product_id: str
    quantity: str
    unit_cost: str
    reason: Optional[str]

    def __init__(
        self,
        product_id: str,
        quantity: str,
        unit_cost: str,
        reason: Optional[str] = None,
    ) -> None: ...

class VendorReturnItem:
    """A single line on a vendor return. Money values are exact decimal
    strings."""

    id: str
    vendor_return_id: str
    product_id: str
    sku: str
    quantity: str
    unit_cost: str
    line_total: str
    reason: str

class VendorReturn:
    """A return of goods to a supplier. Money values are exact decimal
    strings, timestamps RFC 3339 strings, enums snake_case strings."""

    id: str
    number: str
    supplier_id: str
    purchase_order_id: Optional[str]
    status: str
    currency: str
    items: List[VendorReturnItem]
    total_credit: str
    credit_generated: bool
    notes: Optional[str]
    processed_at: Optional[str]
    created_at: str
    updated_at: str

class VendorReturns:
    """Vendor return operations. Money and quantities are exchanged as exact
    decimal strings, timestamps as RFC 3339 strings, enums as snake_case."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        supplier_id: str,
        items: List[VendorReturnItemInput],
        purchase_order_id: Optional[str] = None,
        currency: Optional[str] = None,
        notes: Optional[str] = None,
    ) -> VendorReturn:
        """Create a vendor return (draft). At least one item is required."""
        ...

    def get(self, id: str) -> Optional[VendorReturn]: ...

    def list(
        self,
        supplier_id: Optional[str] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[VendorReturn]: ...

    def submit(self, id: str) -> VendorReturn:
        """Submit a draft vendor return to the supplier."""
        ...

    def process(self, id: str, generate_credit: bool) -> VendorReturn:
        """Process a vendor return, optionally generating a vendor credit."""
        ...

    def cancel(self, id: str) -> VendorReturn:
        """Cancel a vendor return."""
        ...


# ============================================================================
# fraud
# ============================================================================

class FraudSignalInput:
    signal_type: str
    score: float
    details: str
    def __init__(self, signal_type: str, score: float, details: str) -> None: ...

class FraudSignal:
    order_id: str
    signal_type: str
    score: float
    details: str
    detected_at: str

class FraudAssessment:
    order_id: str
    risk_score: float
    signals: list[FraudSignal]
    decision: str
    reviewed_by: str | None
    review_notes: str | None
    needs_review: bool
    created_at: str
    updated_at: str

class FraudRule:
    id: str
    name: str
    description: str | None
    signal_type: str
    threshold: float
    action: str
    enabled: bool
    created_at: str
    updated_at: str

class Fraud:
    def is_supported(self) -> bool: ...
    def create_assessment(
        self, order_id: str, signals: list[FraudSignalInput]
    ) -> FraudAssessment: ...
    def get_assessment(self, order_id: str) -> FraudAssessment | None: ...
    def list_assessments(
        self,
        decision: str | None = None,
        min_risk_score: float | None = None,
        unreviewed_only: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[FraudAssessment]: ...
    def review_assessment(
        self,
        order_id: str,
        decision: str,
        reviewer: str,
        notes: str | None = None,
    ) -> FraudAssessment: ...
    def create_rule(
        self,
        name: str,
        signal_type: str,
        threshold: float,
        action: str,
        description: str | None = None,
    ) -> FraudRule: ...
    def get_rule(self, id: str) -> FraudRule | None: ...
    def update_rule(
        self,
        id: str,
        name: str | None = None,
        description: str | None = None,
        threshold: float | None = None,
        action: str | None = None,
        enabled: bool | None = None,
    ) -> FraudRule: ...
    def list_rules(
        self,
        signal_type: str | None = None,
        action: str | None = None,
        enabled: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[FraudRule]: ...
    def delete_rule(self, id: str) -> None: ...
    def get_active_rules(self) -> list[FraudRule]: ...


# ============================================================================
# search_config
# ============================================================================

class SearchFieldInput:
    """A searchable field passed to create/update."""

    field_name: str
    weight: float
    tokenizer: Optional[str]
    enabled: Optional[bool]

    def __init__(
        self,
        field_name: str,
        weight: float,
        tokenizer: Optional[str] = None,
        enabled: Optional[bool] = None,
    ) -> None: ...

class FacetConfigInput:
    """A facet configuration passed to create/update."""

    field_name: str
    display_name: str
    facet_type: Optional[str]
    sort_order: Optional[int]
    max_values: Optional[int]

    def __init__(
        self,
        field_name: str,
        display_name: str,
        facet_type: Optional[str] = None,
        sort_order: Optional[int] = None,
        max_values: Optional[int] = None,
    ) -> None: ...

class SynonymGroupInput:
    """A synonym group passed to create/update."""

    canonical: str
    synonyms: List[str]

    def __init__(self, canonical: str, synonyms: List[str]) -> None: ...

class BoostRuleInput:
    """A relevance boost rule passed to create/update."""

    field: str
    value_match: str
    boost_factor: float

    def __init__(self, field: str, value_match: str, boost_factor: float) -> None: ...

class SearchField:
    """A searchable field on a search configuration."""

    @property
    def field_name(self) -> str: ...
    @property
    def weight(self) -> float: ...
    @property
    def tokenizer(self) -> str:
        """standard, ngram, edge, keyword"""
        ...
    @property
    def enabled(self) -> bool: ...

class FacetConfig:
    """A facet on a search configuration."""

    @property
    def field_name(self) -> str: ...
    @property
    def facet_type(self) -> str:
        """value, range, hierarchical"""
        ...
    @property
    def display_name(self) -> str: ...
    @property
    def sort_order(self) -> int: ...
    @property
    def max_values(self) -> Optional[int]: ...

class SynonymGroup:
    """A synonym group on a search configuration."""

    @property
    def canonical(self) -> str: ...
    @property
    def synonyms(self) -> List[str]: ...

class BoostRule:
    """A relevance boost rule on a search configuration."""

    @property
    def field(self) -> str: ...
    @property
    def value_match(self) -> str: ...
    @property
    def boost_factor(self) -> float: ...

class SearchConfig:
    """A search configuration. Timestamps are RFC3339 strings."""

    @property
    def id(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def description(self) -> Optional[str]: ...
    @property
    def searchable_fields(self) -> List[SearchField]: ...
    @property
    def facets(self) -> List[FacetConfig]: ...
    @property
    def synonyms(self) -> List[SynonymGroup]: ...
    @property
    def boost_rules(self) -> List[BoostRule]: ...
    @property
    def is_active(self) -> bool: ...
    @property
    def created_at(self) -> str: ...
    @property
    def updated_at(self) -> str: ...

class SearchConfigs:
    """Search configuration operations. Enums cross as snake_case strings and
    timestamps as RFC3339 strings."""

    def is_supported(self) -> bool: ...

    def create(
        self,
        name: str,
        description: Optional[str] = None,
        searchable_fields: Optional[List[SearchFieldInput]] = None,
        facets: Optional[List[FacetConfigInput]] = None,
        synonyms: Optional[List[SynonymGroupInput]] = None,
        boost_rules: Optional[List[BoostRuleInput]] = None,
    ) -> SearchConfig:
        """Create a search configuration."""
        ...

    def get(self, id: str) -> Optional[SearchConfig]: ...

    def update(
        self,
        id: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
        searchable_fields: Optional[List[SearchFieldInput]] = None,
        facets: Optional[List[FacetConfigInput]] = None,
        synonyms: Optional[List[SynonymGroupInput]] = None,
        boost_rules: Optional[List[BoostRuleInput]] = None,
        is_active: Optional[bool] = None,
    ) -> SearchConfig:
        """Update a search configuration."""
        ...

    def list(
        self,
        is_active: Optional[bool] = None,
        name: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> List[SearchConfig]: ...

    def delete(self, id: str) -> None: ...

    def get_active(self) -> Optional[SearchConfig]: ...

    def set_active(self, id: str) -> SearchConfig: ...


# ============================================================================
# erc8004
# ============================================================================

class AgentIdentity:
    id: str
    agent_registry: str
    agent_id: str
    agent_uri: str
    agent_wallet: str | None
    owner_address: str | None
    agent_card_id: str | None
    registration: str | None
    registration_hash: str | None
    wallet_proof_type: str | None
    wallet_proof: str | None
    wallet_proof_chain_id: str | None
    wallet_proof_deadline: str | None
    active: bool
    created_at: str
    updated_at: str

class AgentFeedback:
    id: str
    agent_registry: str
    agent_id: str
    client_address: str
    feedback_index: str
    value: str
    value_decimals: int
    tag1: str | None
    tag2: str | None
    endpoint: str | None
    feedback_uri: str | None
    feedback_hash: str | None
    is_revoked: bool
    created_at: str
    revoked_at: str | None

class FeedbackSummary:
    count: str
    summary_value: str
    summary_value_decimals: int

class AgentValidationRequest:
    request_hash: str
    agent_registry: str
    agent_id: str
    validator_address: str
    request_uri: str
    created_at: str

class AgentValidationResponse:
    id: str
    request_hash: str
    agent_registry: str
    agent_id: str
    validator_address: str
    response: int
    response_uri: str | None
    response_hash: str | None
    tag: str | None
    created_at: str

class AgentValidationStatus:
    validator_address: str
    agent_registry: str
    agent_id: str
    response: int
    response_hash: str | None
    tag: str | None
    last_update: str

class ValidationSummary:
    count: str
    average_response: int

class Erc8004:
    def register_identity(
        self,
        agent_registry: str,
        agent_id: str,
        agent_uri: str,
        agent_wallet: str | None = None,
        owner_address: str | None = None,
        agent_card_id: str | None = None,
        registration: str | None = None,
        registration_hash: str | None = None,
        wallet_proof_type: str | None = None,
        wallet_proof: str | None = None,
        wallet_proof_chain_id: str | None = None,
        wallet_proof_deadline: str | None = None,
        active: bool | None = None,
    ) -> AgentIdentity: ...
    def get_identity(self, agent_registry: str, agent_id: str) -> AgentIdentity | None: ...
    def get_identity_by_wallet(self, agent_wallet: str) -> AgentIdentity | None: ...
    def update_identity(
        self,
        agent_registry: str,
        agent_id: str,
        agent_uri: str | None = None,
        agent_wallet: str | None = None,
        owner_address: str | None = None,
        agent_card_id: str | None = None,
        registration: str | None = None,
        registration_hash: str | None = None,
        wallet_proof_type: str | None = None,
        wallet_proof: str | None = None,
        wallet_proof_chain_id: str | None = None,
        wallet_proof_deadline: str | None = None,
        active: bool | None = None,
    ) -> AgentIdentity: ...
    def set_agent_wallet(
        self,
        agent_registry: str,
        agent_id: str,
        agent_wallet: str,
        proof_type: str | None = None,
        proof: str | None = None,
        proof_chain_id: str | None = None,
        proof_deadline: str | None = None,
    ) -> AgentIdentity: ...
    def clear_agent_wallet(self, agent_registry: str, agent_id: str) -> AgentIdentity: ...
    def list_identities(
        self,
        agent_registry: str | None = None,
        agent_id: str | None = None,
        agent_wallet: str | None = None,
        owner_address: str | None = None,
        agent_card_id: str | None = None,
        active: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[AgentIdentity]: ...
    def count_identities(
        self,
        agent_registry: str | None = None,
        agent_id: str | None = None,
        agent_wallet: str | None = None,
        owner_address: str | None = None,
        agent_card_id: str | None = None,
        active: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> str: ...
    def give_feedback(
        self,
        agent_registry: str,
        agent_id: str,
        client_address: str,
        value: str,
        value_decimals: int,
        tag1: str | None = None,
        tag2: str | None = None,
        endpoint: str | None = None,
        feedback_uri: str | None = None,
        feedback_hash: str | None = None,
    ) -> AgentFeedback: ...
    def revoke_feedback(
        self,
        agent_registry: str,
        agent_id: str,
        client_address: str,
        feedback_index: str,
    ) -> AgentFeedback: ...
    def read_feedback(
        self,
        agent_registry: str,
        agent_id: str,
        client_address: str,
        feedback_index: str,
    ) -> AgentFeedback | None: ...
    def read_all_feedback(
        self,
        agent_registry: str | None = None,
        agent_id: str | None = None,
        client_addresses: list[str] | None = None,
        tag1: str | None = None,
        tag2: str | None = None,
        include_revoked: bool | None = None,
        limit: int | None = None,
        offset: int | None = None,
    ) -> list[AgentFeedback]: ...
    def feedback_summary(
        self,
        agent_registry: str,
        agent_id: str,
        client_addresses: list[str] | None = None,
        tag1: str | None = None,
        tag2: str | None = None,
    ) -> FeedbackSummary: ...
    def request_validation(
        self,
        request_hash: str,
        agent_registry: str,
        agent_id: str,
        validator_address: str,
        request_uri: str,
    ) -> AgentValidationRequest: ...
    def respond_validation(
        self,
        request_hash: str,
        response: int,
        response_uri: str | None = None,
        response_hash: str | None = None,
        tag: str | None = None,
    ) -> AgentValidationResponse: ...
    def validation_status(self, request_hash: str) -> AgentValidationStatus | None: ...
    def validation_summary(
        self,
        agent_registry: str,
        agent_id: str,
        validator_addresses: list[str] | None = None,
        tag: str | None = None,
    ) -> ValidationSummary: ...
