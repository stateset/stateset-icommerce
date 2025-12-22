package stateset

// Customer represents a customer in the commerce system
type Customer struct {
	ID        string  `json:"id"`
	Email     string  `json:"email"`
	FirstName string  `json:"first_name"`
	LastName  string  `json:"last_name"`
	Phone     *string `json:"phone,omitempty"`
	CreatedAt *string `json:"created_at,omitempty"`
	UpdatedAt *string `json:"updated_at,omitempty"`
}

// Product represents a product in the catalog
type Product struct {
	ID          string  `json:"id"`
	Name        string  `json:"name"`
	Slug        *string `json:"slug,omitempty"`
	Description *string `json:"description,omitempty"`
	IsActive    bool    `json:"is_active"`
	CreatedAt   *string `json:"created_at,omitempty"`
	UpdatedAt   *string `json:"updated_at,omitempty"`
}

// ProductVariant represents a product variant (SKU)
type ProductVariant struct {
	ID             string  `json:"id"`
	ProductID      string  `json:"product_id"`
	SKU            string  `json:"sku"`
	Name           *string `json:"name,omitempty"`
	Price          string  `json:"price"`
	CompareAtPrice *string `json:"compare_at_price,omitempty"`
	IsDefault      bool    `json:"is_default"`
	CreatedAt      *string `json:"created_at,omitempty"`
}

// Order represents an order
type Order struct {
	ID          string  `json:"id"`
	OrderNumber string  `json:"order_number"`
	CustomerID  string  `json:"customer_id"`
	Status      string  `json:"status"`
	TotalAmount string  `json:"total_amount"`
	Currency    string  `json:"currency"`
	CreatedAt   *string `json:"created_at,omitempty"`
	UpdatedAt   *string `json:"updated_at,omitempty"`
}

// OrderItem represents an item in an order (for creation)
type OrderItem struct {
	ProductID  string  `json:"product_id"`
	VariantID  *string `json:"variant_id,omitempty"`
	SKU        string  `json:"sku"`
	Name       string  `json:"name"`
	Quantity   int32   `json:"quantity"`
	UnitPrice  string  `json:"unit_price"`
	Discount   *string `json:"discount,omitempty"`
	TaxAmount  *string `json:"tax_amount,omitempty"`
}

// InventoryItem represents an inventory item
type InventoryItem struct {
	ID            int64   `json:"id"`
	SKU           string  `json:"sku"`
	Name          string  `json:"name"`
	Description   *string `json:"description,omitempty"`
	UnitOfMeasure string  `json:"unit_of_measure"`
	IsActive      bool    `json:"is_active"`
	CreatedAt     *string `json:"created_at,omitempty"`
	UpdatedAt     *string `json:"updated_at,omitempty"`
}

// StockLevel represents stock level summary for an inventory item
type StockLevel struct {
	SKU            string          `json:"sku"`
	Name           string          `json:"name"`
	TotalOnHand    string          `json:"total_on_hand"`
	TotalAllocated string          `json:"total_allocated"`
	TotalAvailable string          `json:"total_available"`
	Locations      []LocationStock `json:"locations"`
}

// LocationStock represents stock at a specific location
type LocationStock struct {
	LocationID   int32   `json:"location_id"`
	LocationName *string `json:"location_name,omitempty"`
	OnHand       string  `json:"on_hand"`
	Allocated    string  `json:"allocated"`
	Available    string  `json:"available"`
}

// Cart represents a shopping cart
type Cart struct {
	ID         string  `json:"id"`
	CustomerID *string `json:"customer_id,omitempty"`
	Status     string  `json:"status"`
	GrandTotal string  `json:"grand_total"`
	Currency   string  `json:"currency"`
	CreatedAt  *string `json:"created_at,omitempty"`
}

// Return represents a return request
type Return struct {
	ID           string  `json:"id"`
	OrderID      string  `json:"order_id"`
	Reason       string  `json:"reason"`
	Status       string  `json:"status"`
	RefundAmount *string `json:"refund_amount,omitempty"`
	Notes        *string `json:"notes,omitempty"`
	CreatedAt    *string `json:"created_at,omitempty"`
}

// Payment represents a payment
type Payment struct {
	ID             string  `json:"id"`
	PaymentNumber  string  `json:"payment_number"`
	OrderID        *string `json:"order_id,omitempty"`
	InvoiceID      *string `json:"invoice_id,omitempty"`
	CustomerID     *string `json:"customer_id,omitempty"`
	Status         string  `json:"status"`
	PaymentMethod  string  `json:"payment_method"`
	Amount         string  `json:"amount"`
	Currency       string  `json:"currency"`
	AmountRefunded string  `json:"amount_refunded"`
	ExternalID     *string `json:"external_id,omitempty"`
	Processor      *string `json:"processor,omitempty"`
	CardBrand      *string `json:"card_brand,omitempty"`
	CardLast4      *string `json:"card_last4,omitempty"`
	BillingEmail   *string `json:"billing_email,omitempty"`
	BillingName    *string `json:"billing_name,omitempty"`
	Description    *string `json:"description,omitempty"`
	FailureReason  *string `json:"failure_reason,omitempty"`
	FailureCode    *string `json:"failure_code,omitempty"`
	PaidAt         *string `json:"paid_at,omitempty"`
	Version        int32   `json:"version"`
	CreatedAt      *string `json:"created_at,omitempty"`
	UpdatedAt      *string `json:"updated_at,omitempty"`
}

// SalesSummary represents a sales summary from analytics
type SalesSummary struct {
	TotalRevenue      string `json:"total_revenue"`
	OrderCount        int    `json:"order_count"`
	AverageOrderValue string `json:"average_order_value"`
}

// TopProduct represents a top-selling product from analytics
type TopProduct struct {
	ProductID     string `json:"product_id"`
	ProductName   string `json:"product_name"`
	TotalQuantity int    `json:"total_quantity"`
	TotalRevenue  string `json:"total_revenue"`
}

// TopCustomer represents a top customer from analytics
type TopCustomer struct {
	CustomerID   string `json:"customer_id"`
	CustomerName string `json:"customer_name"`
	OrderCount   int    `json:"order_count"`
	TotalSpent   string `json:"total_spent"`
}

// OrderStatus represents the status of an order
type OrderStatus string

const (
	OrderStatusPending    OrderStatus = "pending"
	OrderStatusConfirmed  OrderStatus = "confirmed"
	OrderStatusProcessing OrderStatus = "processing"
	OrderStatusShipped    OrderStatus = "shipped"
	OrderStatusDelivered  OrderStatus = "delivered"
	OrderStatusCancelled  OrderStatus = "cancelled"
	OrderStatusRefunded   OrderStatus = "refunded"
)

// ReturnReason represents the reason for a return
type ReturnReason string

const (
	ReturnReasonDefective     ReturnReason = "defective"
	ReturnReasonWrongItem     ReturnReason = "wrong_item"
	ReturnReasonNotAsDescribed ReturnReason = "not_as_described"
	ReturnReasonChangedMind   ReturnReason = "changed_mind"
	ReturnReasonDamaged       ReturnReason = "damaged"
	ReturnReasonOther         ReturnReason = "other"
)

// PaymentMethod represents a payment method
type PaymentMethod string

const (
	PaymentMethodCreditCard   PaymentMethod = "credit_card"
	PaymentMethodDebitCard    PaymentMethod = "debit_card"
	PaymentMethodBankTransfer PaymentMethod = "bank_transfer"
	PaymentMethodPayPal       PaymentMethod = "paypal"
	PaymentMethodApplePay     PaymentMethod = "apple_pay"
	PaymentMethodGooglePay    PaymentMethod = "google_pay"
	PaymentMethodCrypto       PaymentMethod = "crypto"
	PaymentMethodOther        PaymentMethod = "other"
)

// TimePeriod represents a time period for analytics
type TimePeriod string

const (
	TimePeriodToday   TimePeriod = "today"
	TimePeriodWeek    TimePeriod = "week"
	TimePeriodMonth   TimePeriod = "month"
	TimePeriodQuarter TimePeriod = "quarter"
	TimePeriodYear    TimePeriod = "year"
	TimePeriodAllTime TimePeriod = "all"
)

// =============================================================================
// Shipment Models
// =============================================================================

// Shipment represents a shipment for order fulfillment
type Shipment struct {
	ID              string  `json:"id"`
	ShipmentNumber  string  `json:"shipment_number"`
	OrderID         string  `json:"order_id"`
	Status          string  `json:"status"`
	Carrier         *string `json:"carrier,omitempty"`
	TrackingNumber  *string `json:"tracking_number,omitempty"`
	TrackingURL     *string `json:"tracking_url,omitempty"`
	RecipientName   string  `json:"recipient_name"`
	RecipientEmail  *string `json:"recipient_email,omitempty"`
	ShippingAddress string  `json:"shipping_address"`
	ShippedAt       *string `json:"shipped_at,omitempty"`
	DeliveredAt     *string `json:"delivered_at,omitempty"`
	EstimatedDelivery *string `json:"estimated_delivery,omitempty"`
	Weight          *string `json:"weight,omitempty"`
	Notes           *string `json:"notes,omitempty"`
	CreatedAt       *string `json:"created_at,omitempty"`
	UpdatedAt       *string `json:"updated_at,omitempty"`
}

// ShipmentItem represents an item in a shipment
type ShipmentItem struct {
	ID          string  `json:"id"`
	ShipmentID  string  `json:"shipment_id"`
	SKU         string  `json:"sku"`
	Name        string  `json:"name"`
	Quantity    int32   `json:"quantity"`
	OrderItemID *string `json:"order_item_id,omitempty"`
}

// ShipmentEvent represents a tracking event for a shipment
type ShipmentEvent struct {
	ID          string  `json:"id"`
	ShipmentID  string  `json:"shipment_id"`
	EventType   string  `json:"event_type"`
	Location    *string `json:"location,omitempty"`
	Description *string `json:"description,omitempty"`
	EventTime   string  `json:"event_time"`
	CreatedAt   *string `json:"created_at,omitempty"`
}

// ShipmentStatus represents the status of a shipment
type ShipmentStatus string

const (
	ShipmentStatusPending        ShipmentStatus = "pending"
	ShipmentStatusProcessing     ShipmentStatus = "processing"
	ShipmentStatusReady          ShipmentStatus = "ready"
	ShipmentStatusShipped        ShipmentStatus = "shipped"
	ShipmentStatusInTransit      ShipmentStatus = "in_transit"
	ShipmentStatusOutForDelivery ShipmentStatus = "out_for_delivery"
	ShipmentStatusDelivered      ShipmentStatus = "delivered"
	ShipmentStatusFailed         ShipmentStatus = "failed"
	ShipmentStatusCancelled      ShipmentStatus = "cancelled"
)

// ShippingCarrier represents a shipping carrier
type ShippingCarrier string

const (
	CarrierUPS     ShippingCarrier = "ups"
	CarrierFedEx   ShippingCarrier = "fedex"
	CarrierUSPS    ShippingCarrier = "usps"
	CarrierDHL     ShippingCarrier = "dhl"
	CarrierOther   ShippingCarrier = "other"
)

// =============================================================================
// Warranty Models
// =============================================================================

// Warranty represents a product warranty
type Warranty struct {
	ID                  string  `json:"id"`
	WarrantyNumber      string  `json:"warranty_number"`
	CustomerID          string  `json:"customer_id"`
	ProductID           *string `json:"product_id,omitempty"`
	OrderID             *string `json:"order_id,omitempty"`
	OrderItemID         *string `json:"order_item_id,omitempty"`
	SerialNumber        *string `json:"serial_number,omitempty"`
	Status              string  `json:"status"`
	WarrantyType        string  `json:"warranty_type"`
	DurationMonths      int32   `json:"duration_months"`
	CoverageDescription *string `json:"coverage_description,omitempty"`
	StartDate           string  `json:"start_date"`
	EndDate             string  `json:"end_date"`
	PurchaseDate        *string `json:"purchase_date,omitempty"`
	Notes               *string `json:"notes,omitempty"`
	CreatedAt           *string `json:"created_at,omitempty"`
	UpdatedAt           *string `json:"updated_at,omitempty"`
}

// WarrantyClaim represents a warranty claim
type WarrantyClaim struct {
	ID               string  `json:"id"`
	ClaimNumber      string  `json:"claim_number"`
	WarrantyID       string  `json:"warranty_id"`
	Status           string  `json:"status"`
	IssueDescription string  `json:"issue_description"`
	Resolution       *string `json:"resolution,omitempty"`
	ResolutionNotes  *string `json:"resolution_notes,omitempty"`
	ContactEmail     *string `json:"contact_email,omitempty"`
	ContactPhone     *string `json:"contact_phone,omitempty"`
	DenialReason     *string `json:"denial_reason,omitempty"`
	ResolvedAt       *string `json:"resolved_at,omitempty"`
	CreatedAt        *string `json:"created_at,omitempty"`
	UpdatedAt        *string `json:"updated_at,omitempty"`
}

// WarrantyType represents the type of warranty
type WarrantyType string

const (
	WarrantyTypeStandard WarrantyType = "standard"
	WarrantyTypeExtended WarrantyType = "extended"
	WarrantyTypeLimited  WarrantyType = "limited"
	WarrantyTypeLifetime WarrantyType = "lifetime"
)

// WarrantyStatus represents the status of a warranty
type WarrantyStatus string

const (
	WarrantyStatusActive  WarrantyStatus = "active"
	WarrantyStatusExpired WarrantyStatus = "expired"
	WarrantyStatusVoided  WarrantyStatus = "voided"
)

// ClaimStatus represents the status of a warranty claim
type ClaimStatus string

const (
	ClaimStatusPending   ClaimStatus = "pending"
	ClaimStatusApproved  ClaimStatus = "approved"
	ClaimStatusDenied    ClaimStatus = "denied"
	ClaimStatusCompleted ClaimStatus = "completed"
	ClaimStatusCancelled ClaimStatus = "cancelled"
)

// ClaimResolution represents how a warranty claim was resolved
type ClaimResolution string

const (
	ClaimResolutionRepair      ClaimResolution = "repair"
	ClaimResolutionReplacement ClaimResolution = "replacement"
	ClaimResolutionRefund      ClaimResolution = "refund"
	ClaimResolutionCredit      ClaimResolution = "store_credit"
)

// =============================================================================
// Purchase Order Models
// =============================================================================

// Supplier represents a supplier/vendor
type Supplier struct {
	ID           string  `json:"id"`
	SupplierCode *string `json:"supplier_code,omitempty"`
	Name         string  `json:"name"`
	Email        *string `json:"email,omitempty"`
	Phone        *string `json:"phone,omitempty"`
	Address      *string `json:"address,omitempty"`
	ContactName  *string `json:"contact_name,omitempty"`
	PaymentTerms *string `json:"payment_terms,omitempty"`
	LeadTimeDays *int32  `json:"lead_time_days,omitempty"`
	IsActive     bool    `json:"is_active"`
	Notes        *string `json:"notes,omitempty"`
	CreatedAt    *string `json:"created_at,omitempty"`
	UpdatedAt    *string `json:"updated_at,omitempty"`
}

// PurchaseOrder represents a purchase order to a supplier
type PurchaseOrder struct {
	ID                string  `json:"id"`
	PONumber          string  `json:"po_number"`
	SupplierID        string  `json:"supplier_id"`
	Status            string  `json:"status"`
	Subtotal          string  `json:"subtotal"`
	TaxAmount         string  `json:"tax_amount"`
	ShippingCost      string  `json:"shipping_cost"`
	Total             string  `json:"total"`
	Currency          string  `json:"currency"`
	ShipToAddress     *string `json:"ship_to_address,omitempty"`
	ExpectedDate      *string `json:"expected_date,omitempty"`
	ReceivedDate      *string `json:"received_date,omitempty"`
	ApprovedBy        *string `json:"approved_by,omitempty"`
	ApprovedAt        *string `json:"approved_at,omitempty"`
	SupplierReference *string `json:"supplier_reference,omitempty"`
	Notes             *string `json:"notes,omitempty"`
	CreatedAt         *string `json:"created_at,omitempty"`
	UpdatedAt         *string `json:"updated_at,omitempty"`
}

// PurchaseOrderItem represents an item in a purchase order
type PurchaseOrderItem struct {
	ID               string  `json:"id"`
	PurchaseOrderID  string  `json:"purchase_order_id"`
	SKU              string  `json:"sku"`
	Name             string  `json:"name"`
	Description      *string `json:"description,omitempty"`
	Quantity         string  `json:"quantity"`
	UnitCost         string  `json:"unit_cost"`
	Total            string  `json:"total"`
	QuantityReceived string  `json:"quantity_received"`
}

// PurchaseOrderStatus represents the status of a purchase order
type PurchaseOrderStatus string

const (
	POStatusDraft              PurchaseOrderStatus = "draft"
	POStatusPendingApproval    PurchaseOrderStatus = "pending_approval"
	POStatusApproved           PurchaseOrderStatus = "approved"
	POStatusSent               PurchaseOrderStatus = "sent"
	POStatusAcknowledged       PurchaseOrderStatus = "acknowledged"
	POStatusPartiallyReceived  PurchaseOrderStatus = "partially_received"
	POStatusReceived           PurchaseOrderStatus = "received"
	POStatusCompleted          PurchaseOrderStatus = "completed"
	POStatusCancelled          PurchaseOrderStatus = "cancelled"
	POStatusOnHold             PurchaseOrderStatus = "on_hold"
)

// PaymentTerms represents payment terms for suppliers
type PaymentTerms string

const (
	PaymentTermsDueOnReceipt PaymentTerms = "due_on_receipt"
	PaymentTermsNet15        PaymentTerms = "net_15"
	PaymentTermsNet30        PaymentTerms = "net_30"
	PaymentTermsNet45        PaymentTerms = "net_45"
	PaymentTermsNet60        PaymentTerms = "net_60"
	PaymentTermsNet90        PaymentTerms = "net_90"
)

// =============================================================================
// Invoice Models
// =============================================================================

// Invoice represents an invoice for billing
type Invoice struct {
	ID              string  `json:"id"`
	InvoiceNumber   string  `json:"invoice_number"`
	CustomerID      string  `json:"customer_id"`
	OrderID         *string `json:"order_id,omitempty"`
	Status          string  `json:"status"`
	InvoiceType     string  `json:"invoice_type"`
	Subtotal        string  `json:"subtotal"`
	TaxAmount       string  `json:"tax_amount"`
	Total           string  `json:"total"`
	AmountPaid      string  `json:"amount_paid"`
	Currency        string  `json:"currency"`
	BillingEmail    *string `json:"billing_email,omitempty"`
	BillingName     *string `json:"billing_name,omitempty"`
	BillingAddress  *string `json:"billing_address,omitempty"`
	DueDate         *string `json:"due_date,omitempty"`
	SentAt          *string `json:"sent_at,omitempty"`
	ViewedAt        *string `json:"viewed_at,omitempty"`
	PaidAt          *string `json:"paid_at,omitempty"`
	Notes           *string `json:"notes,omitempty"`
	CreatedAt       *string `json:"created_at,omitempty"`
	UpdatedAt       *string `json:"updated_at,omitempty"`
}

// InvoiceItem represents an item in an invoice
type InvoiceItem struct {
	ID          string  `json:"id"`
	InvoiceID   string  `json:"invoice_id"`
	Description string  `json:"description"`
	Quantity    string  `json:"quantity"`
	UnitPrice   string  `json:"unit_price"`
	Total       string  `json:"total"`
	SKU         *string `json:"sku,omitempty"`
	TaxRate     *string `json:"tax_rate,omitempty"`
}

// InvoiceStatus represents the status of an invoice
type InvoiceStatus string

const (
	InvoiceStatusDraft         InvoiceStatus = "draft"
	InvoiceStatusSent          InvoiceStatus = "sent"
	InvoiceStatusViewed        InvoiceStatus = "viewed"
	InvoiceStatusPartiallyPaid InvoiceStatus = "partially_paid"
	InvoiceStatusPaid          InvoiceStatus = "paid"
	InvoiceStatusOverdue       InvoiceStatus = "overdue"
	InvoiceStatusVoided        InvoiceStatus = "voided"
	InvoiceStatusWrittenOff    InvoiceStatus = "written_off"
	InvoiceStatusDisputed      InvoiceStatus = "disputed"
)

// InvoiceType represents the type of invoice
type InvoiceType string

const (
	InvoiceTypeStandard   InvoiceType = "standard"
	InvoiceTypeProforma   InvoiceType = "proforma"
	InvoiceTypeCreditNote InvoiceType = "credit_note"
	InvoiceTypeRecurring  InvoiceType = "recurring"
)

// =============================================================================
// Bill of Materials (BOM) Models
// =============================================================================

// BillOfMaterials represents a bill of materials for manufacturing
type BillOfMaterials struct {
	ID          string  `json:"id"`
	BOMNumber   string  `json:"bom_number"`
	ProductID   string  `json:"product_id"`
	Name        string  `json:"name"`
	Description *string `json:"description,omitempty"`
	Version     string  `json:"version"`
	Status      string  `json:"status"`
	Notes       *string `json:"notes,omitempty"`
	CreatedAt   *string `json:"created_at,omitempty"`
	UpdatedAt   *string `json:"updated_at,omitempty"`
}

// BOMComponent represents a component in a bill of materials
type BOMComponent struct {
	ID           string  `json:"id"`
	BOMID        string  `json:"bom_id"`
	ComponentSKU *string `json:"component_sku,omitempty"`
	Name         string  `json:"name"`
	Description  *string `json:"description,omitempty"`
	Quantity     string  `json:"quantity"`
	UnitOfMeasure *string `json:"unit_of_measure,omitempty"`
	Position     *string `json:"position,omitempty"`
	IsOptional   bool    `json:"is_optional"`
	Notes        *string `json:"notes,omitempty"`
}

// BOMStatus represents the status of a BOM
type BOMStatus string

const (
	BOMStatusDraft    BOMStatus = "draft"
	BOMStatusActive   BOMStatus = "active"
	BOMStatusObsolete BOMStatus = "obsolete"
)

// =============================================================================
// Work Order Models
// =============================================================================

// WorkOrder represents a manufacturing work order
type WorkOrder struct {
	ID               string  `json:"id"`
	WorkOrderNumber  string  `json:"work_order_number"`
	ProductID        string  `json:"product_id"`
	BOMID            *string `json:"bom_id,omitempty"`
	Status           string  `json:"status"`
	Priority         string  `json:"priority"`
	QuantityToBuild  string  `json:"quantity_to_build"`
	QuantityCompleted string `json:"quantity_completed"`
	PlannedStart     *string `json:"planned_start,omitempty"`
	PlannedEnd       *string `json:"planned_end,omitempty"`
	ActualStart      *string `json:"actual_start,omitempty"`
	ActualEnd        *string `json:"actual_end,omitempty"`
	Notes            *string `json:"notes,omitempty"`
	CreatedAt        *string `json:"created_at,omitempty"`
	UpdatedAt        *string `json:"updated_at,omitempty"`
}

// WorkOrderTask represents a task in a work order
type WorkOrderTask struct {
	ID             string  `json:"id"`
	WorkOrderID    string  `json:"work_order_id"`
	TaskName       string  `json:"task_name"`
	Description    *string `json:"description,omitempty"`
	Sequence       int32   `json:"sequence"`
	Status         string  `json:"status"`
	EstimatedHours *string `json:"estimated_hours,omitempty"`
	ActualHours    *string `json:"actual_hours,omitempty"`
	StartedAt      *string `json:"started_at,omitempty"`
	CompletedAt    *string `json:"completed_at,omitempty"`
}

// WorkOrderMaterial represents material used in a work order
type WorkOrderMaterial struct {
	ID              string  `json:"id"`
	WorkOrderID     string  `json:"work_order_id"`
	ComponentSKU    string  `json:"component_sku"`
	ComponentName   string  `json:"component_name"`
	QuantityRequired string `json:"quantity_required"`
	QuantityConsumed string `json:"quantity_consumed"`
}

// WorkOrderStatus represents the status of a work order
type WorkOrderStatus string

const (
	WOStatusPlanned            WorkOrderStatus = "planned"
	WOStatusInProgress         WorkOrderStatus = "in_progress"
	WOStatusOnHold             WorkOrderStatus = "on_hold"
	WOStatusCompleted          WorkOrderStatus = "completed"
	WOStatusPartiallyCompleted WorkOrderStatus = "partially_completed"
	WOStatusCancelled          WorkOrderStatus = "cancelled"
)

// WorkOrderPriority represents the priority of a work order
type WorkOrderPriority string

const (
	WOPriorityLow      WorkOrderPriority = "low"
	WOPriorityNormal   WorkOrderPriority = "normal"
	WOPriorityHigh     WorkOrderPriority = "high"
	WOPriorityUrgent   WorkOrderPriority = "urgent"
)

// TaskStatus represents the status of a work order task
type TaskStatus string

const (
	TaskStatusPending    TaskStatus = "pending"
	TaskStatusInProgress TaskStatus = "in_progress"
	TaskStatusCompleted  TaskStatus = "completed"
	TaskStatusSkipped    TaskStatus = "skipped"
)

// =============================================================================
// Currency Models
// =============================================================================

// ExchangeRate represents an exchange rate between currencies
type ExchangeRate struct {
	ID            string  `json:"id"`
	BaseCurrency  string  `json:"base_currency"`
	QuoteCurrency string  `json:"quote_currency"`
	Rate          string  `json:"rate"`
	Source        *string `json:"source,omitempty"`
	ValidFrom     string  `json:"valid_from"`
	ValidTo       *string `json:"valid_to,omitempty"`
	CreatedAt     *string `json:"created_at,omitempty"`
}

// ConversionResult represents the result of a currency conversion
type ConversionResult struct {
	FromCurrency    string `json:"from_currency"`
	ToCurrency      string `json:"to_currency"`
	OriginalAmount  string `json:"original_amount"`
	ConvertedAmount string `json:"converted_amount"`
	Rate            string `json:"rate"`
	RateAt          string `json:"rate_at"`
}

// StoreCurrencySettings represents store currency configuration
type StoreCurrencySettings struct {
	BaseCurrency      string   `json:"base_currency"`
	EnabledCurrencies []string `json:"enabled_currencies"`
	AutoConvert       bool     `json:"auto_convert"`
	RoundingMode      string   `json:"rounding_mode"`
}

// Currency represents a currency code
type Currency string

const (
	CurrencyUSD Currency = "USD"
	CurrencyEUR Currency = "EUR"
	CurrencyGBP Currency = "GBP"
	CurrencyJPY Currency = "JPY"
	CurrencyCAD Currency = "CAD"
	CurrencyAUD Currency = "AUD"
	CurrencyCHF Currency = "CHF"
	CurrencyCNY Currency = "CNY"
)

// =============================================================================
// Refund Models
// =============================================================================

// Refund represents a refund for a payment
type Refund struct {
	ID            string  `json:"id"`
	RefundNumber  string  `json:"refund_number"`
	PaymentID     string  `json:"payment_id"`
	Amount        string  `json:"amount"`
	Currency      string  `json:"currency"`
	Status        string  `json:"status"`
	Reason        *string `json:"reason,omitempty"`
	ExternalID    *string `json:"external_id,omitempty"`
	FailureReason *string `json:"failure_reason,omitempty"`
	RefundedAt    *string `json:"refunded_at,omitempty"`
	CreatedAt     *string `json:"created_at,omitempty"`
}

// RefundStatus represents the status of a refund
type RefundStatus string

const (
	RefundStatusPending   RefundStatus = "pending"
	RefundStatusCompleted RefundStatus = "completed"
	RefundStatusFailed    RefundStatus = "failed"
)

// =============================================================================
// Return Status
// =============================================================================

// ReturnStatus represents the status of a return
type ReturnStatus string

const (
	ReturnStatusRequested ReturnStatus = "requested"
	ReturnStatusApproved  ReturnStatus = "approved"
	ReturnStatusRejected  ReturnStatus = "rejected"
	ReturnStatusInTransit ReturnStatus = "in_transit"
	ReturnStatusReceived  ReturnStatus = "received"
	ReturnStatusCompleted ReturnStatus = "completed"
	ReturnStatusCancelled ReturnStatus = "cancelled"
)

// =============================================================================
// Inventory Reservation Models
// =============================================================================

// InventoryReservation represents a stock reservation
type InventoryReservation struct {
	ID           string  `json:"id"`
	SKU          string  `json:"sku"`
	Quantity     string  `json:"quantity"`
	OrderID      *string `json:"order_id,omitempty"`
	Status       string  `json:"status"`
	ExpiresAt    *string `json:"expires_at,omitempty"`
	ConfirmedAt  *string `json:"confirmed_at,omitempty"`
	ReleasedAt   *string `json:"released_at,omitempty"`
	CreatedAt    *string `json:"created_at,omitempty"`
}

// ReservationStatus represents the status of an inventory reservation
type ReservationStatus string

const (
	ReservationStatusPending   ReservationStatus = "pending"
	ReservationStatusConfirmed ReservationStatus = "confirmed"
	ReservationStatusReleased  ReservationStatus = "released"
	ReservationStatusExpired   ReservationStatus = "expired"
)
