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
    def revenue_recognition(self) -> RevenueRecognition:
        """Get the revenue recognition (ASC 606) API."""
        ...

    @property
    def cycle_counts(self) -> CycleCounts:
        """Get the cycle counts API."""
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

    def __init__(self, commerce: Commerce, allow_apply: bool = False) -> None:
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
) -> EmbeddedAgentToolkit:
    ...

def create_tool_descriptors(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
) -> List[AgentToolDescriptor]:
    ...

def create_callable_registry(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
) -> Dict[str, Callable[[Optional[Mapping[str, Any]]], Dict[str, Any]]]:
    ...

def execute_tool(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_name: str,
    params: Optional[Mapping[str, Any]] = None,
    allow_apply: bool = False,
) -> Dict[str, Any]:
    ...

def execute_tool_calls(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
) -> List[Mapping[str, Any]]:
    ...

def create_openai_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
) -> List[Mapping[str, Any]]:
    ...

def execute_openai_tool_call(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_call: Mapping[str, Any],
    allow_apply: bool = False,
) -> Mapping[str, Any]:
    ...

def execute_openai_tool_calls(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    tool_calls: Sequence[Mapping[str, Any]],
    allow_apply: bool = False,
) -> List[Mapping[str, Any]]:
    ...

def create_langchain_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
) -> List[FrameworkToolT]:
    ...

def create_crewai_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
    tool_factory: Optional[Callable[[AgentToolDescriptor], FrameworkToolT]] = None,
) -> List[FrameworkToolT]:
    ...

def create_autogen_tools(
    commerce_or_toolkit: Union[Commerce, EmbeddedAgentToolkit],
    filter: Optional[Sequence[str]] = None,
    allow_apply: bool = False,
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
    total: float

class Order:
    """Order data returned from operations."""

    id: str
    order_number: str
    customer_id: str
    status: str
    total_amount: float
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
    compare_at_price: Optional[float]
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

    def list(self) -> List[PurchaseOrder]: ...

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

    def list(self) -> List[WorkOrder]: ...

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
    total: float

class Cart:
    """Cart data."""

    id: str
    cart_number: str
    status: str
    currency: str
    grand_total: float
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
