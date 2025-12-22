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
