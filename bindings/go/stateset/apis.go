package stateset

/*
#include <stdlib.h>

typedef void* StateSetHandle;

// Customers API
extern char* stateset_customer_create(StateSetHandle handle, const char* email, const char* first_name, const char* last_name, const char* phone);
extern char* stateset_customer_get(StateSetHandle handle, const char* id);
extern char* stateset_customer_list(StateSetHandle handle);
extern int stateset_customer_delete(StateSetHandle handle, const char* id);

// Products API
extern char* stateset_product_create(StateSetHandle handle, const char* name, const char* sku, double price, const char* description);
extern char* stateset_product_get(StateSetHandle handle, const char* id);
extern char* stateset_product_list(StateSetHandle handle);

// Orders API
extern char* stateset_order_create(StateSetHandle handle, const char* customer_id, const char* items_json, const char* currency);
extern char* stateset_order_get(StateSetHandle handle, const char* id);
extern char* stateset_order_list(StateSetHandle handle);
extern char* stateset_order_update_status(StateSetHandle handle, const char* id, const char* status);
extern char* stateset_order_ship(StateSetHandle handle, const char* id);
extern char* stateset_order_cancel(StateSetHandle handle, const char* id);

// Inventory API
extern char* stateset_inventory_create_item(StateSetHandle handle, const char* sku, const char* name, double initial_quantity);
extern int stateset_inventory_adjust(StateSetHandle handle, const char* sku, double quantity_delta, const char* reason);
extern char* stateset_inventory_get_level(StateSetHandle handle, const char* sku);

// Carts API
extern char* stateset_cart_create(StateSetHandle handle, const char* customer_id, const char* currency);
extern char* stateset_cart_add_item(StateSetHandle handle, const char* cart_id, const char* variant_id, int quantity);
extern char* stateset_cart_get(StateSetHandle handle, const char* cart_id);

// Returns API
extern char* stateset_return_create(StateSetHandle handle, const char* order_id, const char* reason, const char* notes);
extern char* stateset_return_get(StateSetHandle handle, const char* id);
extern char* stateset_return_list(StateSetHandle handle);
extern char* stateset_return_approve(StateSetHandle handle, const char* id);
extern char* stateset_return_reject(StateSetHandle handle, const char* id, const char* reason);
extern char* stateset_return_complete(StateSetHandle handle, const char* id);

// Payments API
extern char* stateset_payment_create(StateSetHandle handle, const char* order_id, double amount, const char* currency, const char* method);
extern char* stateset_payment_get(StateSetHandle handle, const char* id);
extern char* stateset_payment_list(StateSetHandle handle);
extern char* stateset_payment_complete(StateSetHandle handle, const char* id);
extern char* stateset_payment_fail(StateSetHandle handle, const char* id, const char* reason);
extern char* stateset_payment_refund(StateSetHandle handle, const char* payment_id, double amount, const char* reason);

// Analytics API
extern char* stateset_analytics_sales_summary(StateSetHandle handle, const char* period);
extern char* stateset_analytics_top_products(StateSetHandle handle, int limit);
extern char* stateset_analytics_top_customers(StateSetHandle handle, int limit);

// Shipments API
extern char* stateset_shipment_create(StateSetHandle handle, const char* order_id, const char* recipient_name, const char* shipping_address, const char* carrier);
extern char* stateset_shipment_get(StateSetHandle handle, const char* id);
extern char* stateset_shipment_list(StateSetHandle handle);
extern char* stateset_shipment_ship(StateSetHandle handle, const char* id, const char* tracking_number);
extern char* stateset_shipment_deliver(StateSetHandle handle, const char* id);
extern char* stateset_shipment_cancel(StateSetHandle handle, const char* id);

// Warranties API
extern char* stateset_warranty_create(StateSetHandle handle, const char* customer_id, const char* product_id, const char* warranty_type, int duration_months);
extern char* stateset_warranty_get(StateSetHandle handle, const char* id);
extern char* stateset_warranty_list(StateSetHandle handle);
extern char* stateset_warranty_create_claim(StateSetHandle handle, const char* warranty_id, const char* issue_description);
extern char* stateset_warranty_approve_claim(StateSetHandle handle, const char* claim_id);
extern char* stateset_warranty_deny_claim(StateSetHandle handle, const char* claim_id, const char* reason);
extern char* stateset_warranty_complete_claim(StateSetHandle handle, const char* claim_id, const char* resolution);

// Suppliers API
extern char* stateset_supplier_create(StateSetHandle handle, const char* name, const char* email, const char* phone);
extern char* stateset_supplier_get(StateSetHandle handle, const char* id);
extern char* stateset_supplier_list(StateSetHandle handle);

// Purchase Orders API
extern char* stateset_purchase_order_create(StateSetHandle handle, const char* supplier_id, const char* items_json);
extern char* stateset_purchase_order_get(StateSetHandle handle, const char* id);
extern char* stateset_purchase_order_list(StateSetHandle handle);
extern char* stateset_purchase_order_submit(StateSetHandle handle, const char* id);
extern char* stateset_purchase_order_approve(StateSetHandle handle, const char* id, const char* approved_by);
extern char* stateset_purchase_order_send(StateSetHandle handle, const char* id);
extern char* stateset_purchase_order_cancel(StateSetHandle handle, const char* id);

// Invoices API
extern char* stateset_invoice_create(StateSetHandle handle, const char* customer_id, const char* items_json, const char* billing_email);
extern char* stateset_invoice_get(StateSetHandle handle, const char* id);
extern char* stateset_invoice_list(StateSetHandle handle);
extern char* stateset_invoice_send(StateSetHandle handle, const char* id);
extern char* stateset_invoice_void(StateSetHandle handle, const char* id);
extern char* stateset_invoice_record_payment(StateSetHandle handle, const char* id, double amount, const char* payment_method);
extern char* stateset_invoice_get_overdue(StateSetHandle handle);

// BOM API
extern char* stateset_bom_create(StateSetHandle handle, const char* product_id, const char* name, const char* description);
extern char* stateset_bom_get(StateSetHandle handle, const char* id);
extern char* stateset_bom_list(StateSetHandle handle);
extern char* stateset_bom_add_component(StateSetHandle handle, const char* bom_id, const char* name, const char* component_sku, double quantity);
extern char* stateset_bom_get_components(StateSetHandle handle, const char* bom_id);
extern char* stateset_bom_activate(StateSetHandle handle, const char* id);

// Work Orders API
extern char* stateset_work_order_create(StateSetHandle handle, const char* product_id, double quantity_to_build, const char* bom_id);
extern char* stateset_work_order_get(StateSetHandle handle, const char* id);
extern char* stateset_work_order_list(StateSetHandle handle);
extern char* stateset_work_order_start(StateSetHandle handle, const char* id);
extern char* stateset_work_order_complete(StateSetHandle handle, const char* id, double quantity_completed);
extern char* stateset_work_order_cancel(StateSetHandle handle, const char* id);

// Currency API
extern char* stateset_currency_set_rate(StateSetHandle handle, const char* from_currency, const char* to_currency, double rate);
extern char* stateset_currency_get_rate(StateSetHandle handle, const char* from_currency, const char* to_currency);
extern char* stateset_currency_convert(StateSetHandle handle, double amount, const char* from_currency, const char* to_currency);
extern char* stateset_currency_get_settings(StateSetHandle handle);
*/
import "C"

import (
	"encoding/json"
	"unsafe"
)

// =============================================================================
// Customers API
// =============================================================================

// CustomersAPI provides customer management operations
type CustomersAPI struct {
	commerce *Commerce
}

// Create creates a new customer
func (api *CustomersAPI) Create(email, firstName, lastName, phone string) (*Customer, error) {
	cEmail := C.CString(email)
	cFirstName := C.CString(firstName)
	cLastName := C.CString(lastName)
	cPhone := C.CString(phone)
	defer C.free(unsafe.Pointer(cEmail))
	defer C.free(unsafe.Pointer(cFirstName))
	defer C.free(unsafe.Pointer(cLastName))
	defer C.free(unsafe.Pointer(cPhone))

	result := C.stateset_customer_create(api.commerce.handle, cEmail, cFirstName, cLastName, cPhone)
	return parseJSON[Customer](result)
}

// Get retrieves a customer by ID
func (api *CustomersAPI) Get(id string) (*Customer, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_customer_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Customer](result)
}

// List retrieves all customers
func (api *CustomersAPI) List() ([]Customer, error) {
	result := C.stateset_customer_list(api.commerce.handle)
	return parseJSONArray[Customer](result)
}

// Delete removes a customer by ID
func (api *CustomersAPI) Delete(id string) bool {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	return C.stateset_customer_delete(api.commerce.handle, cID) == 1
}

// =============================================================================
// Products API
// =============================================================================

// ProductsAPI provides product management operations
type ProductsAPI struct {
	commerce *Commerce
}

// Create creates a new product
func (api *ProductsAPI) Create(name, sku string, price float64, description string) (*Product, error) {
	cName := C.CString(name)
	cSKU := C.CString(sku)
	cDesc := C.CString(description)
	defer C.free(unsafe.Pointer(cName))
	defer C.free(unsafe.Pointer(cSKU))
	defer C.free(unsafe.Pointer(cDesc))

	result := C.stateset_product_create(api.commerce.handle, cName, cSKU, C.double(price), cDesc)
	return parseJSON[Product](result)
}

// Get retrieves a product by ID
func (api *ProductsAPI) Get(id string) (*Product, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_product_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Product](result)
}

// List retrieves all products
func (api *ProductsAPI) List() ([]Product, error) {
	result := C.stateset_product_list(api.commerce.handle)
	return parseJSONArray[Product](result)
}

// =============================================================================
// Orders API
// =============================================================================

// OrdersAPI provides order management operations
type OrdersAPI struct {
	commerce *Commerce
}

// Create creates a new order
func (api *OrdersAPI) Create(customerID string, items []OrderItem, currency string) (*Order, error) {
	cCustomerID := C.CString(customerID)
	cCurrency := C.CString(currency)
	defer C.free(unsafe.Pointer(cCustomerID))
	defer C.free(unsafe.Pointer(cCurrency))

	itemsJSON, err := json.Marshal(items)
	if err != nil {
		return nil, err
	}
	cItemsJSON := C.CString(string(itemsJSON))
	defer C.free(unsafe.Pointer(cItemsJSON))

	result := C.stateset_order_create(api.commerce.handle, cCustomerID, cItemsJSON, cCurrency)
	return parseJSON[Order](result)
}

// Get retrieves an order by ID
func (api *OrdersAPI) Get(id string) (*Order, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_order_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Order](result)
}

// List retrieves all orders
func (api *OrdersAPI) List() ([]Order, error) {
	result := C.stateset_order_list(api.commerce.handle)
	return parseJSONArray[Order](result)
}

// UpdateStatus updates the status of an order
func (api *OrdersAPI) UpdateStatus(id string, status OrderStatus) (*Order, error) {
	cID := C.CString(id)
	cStatus := C.CString(string(status))
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cStatus))

	result := C.stateset_order_update_status(api.commerce.handle, cID, cStatus)
	return parseJSON[Order](result)
}

// Ship marks an order as shipped
func (api *OrdersAPI) Ship(id string) (*Order, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_order_ship(api.commerce.handle, cID)
	return parseJSON[Order](result)
}

// Cancel cancels an order
func (api *OrdersAPI) Cancel(id string) (*Order, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_order_cancel(api.commerce.handle, cID)
	return parseJSON[Order](result)
}

// =============================================================================
// Inventory API
// =============================================================================

// InventoryAPI provides inventory management operations
type InventoryAPI struct {
	commerce *Commerce
}

// CreateItem creates a new inventory item
func (api *InventoryAPI) CreateItem(sku, name string, initialQuantity float64) (*InventoryItem, error) {
	cSKU := C.CString(sku)
	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cSKU))
	defer C.free(unsafe.Pointer(cName))

	result := C.stateset_inventory_create_item(api.commerce.handle, cSKU, cName, C.double(initialQuantity))
	return parseJSON[InventoryItem](result)
}

// Adjust adjusts inventory quantity
func (api *InventoryAPI) Adjust(sku string, quantityDelta float64, reason string) bool {
	cSKU := C.CString(sku)
	cReason := C.CString(reason)
	defer C.free(unsafe.Pointer(cSKU))
	defer C.free(unsafe.Pointer(cReason))

	return C.stateset_inventory_adjust(api.commerce.handle, cSKU, C.double(quantityDelta), cReason) == 1
}

// GetLevel retrieves stock level for a SKU
func (api *InventoryAPI) GetLevel(sku string) (*StockLevel, error) {
	cSKU := C.CString(sku)
	defer C.free(unsafe.Pointer(cSKU))

	result := C.stateset_inventory_get_level(api.commerce.handle, cSKU)
	if result == nil {
		return nil, nil
	}
	return parseJSON[StockLevel](result)
}

// =============================================================================
// Carts API
// =============================================================================

// CartsAPI provides shopping cart operations
type CartsAPI struct {
	commerce *Commerce
}

// Create creates a new cart
func (api *CartsAPI) Create(customerID, currency string) (*Cart, error) {
	cCustomerID := C.CString(customerID)
	cCurrency := C.CString(currency)
	defer C.free(unsafe.Pointer(cCustomerID))
	defer C.free(unsafe.Pointer(cCurrency))

	result := C.stateset_cart_create(api.commerce.handle, cCustomerID, cCurrency)
	return parseJSON[Cart](result)
}

// AddItem adds an item to a cart
func (api *CartsAPI) AddItem(cartID, variantID string, quantity int) (*Cart, error) {
	cCartID := C.CString(cartID)
	cVariantID := C.CString(variantID)
	defer C.free(unsafe.Pointer(cCartID))
	defer C.free(unsafe.Pointer(cVariantID))

	result := C.stateset_cart_add_item(api.commerce.handle, cCartID, cVariantID, C.int(quantity))
	return parseJSON[Cart](result)
}

// Get retrieves a cart by ID
func (api *CartsAPI) Get(cartID string) (*Cart, error) {
	cCartID := C.CString(cartID)
	defer C.free(unsafe.Pointer(cCartID))

	result := C.stateset_cart_get(api.commerce.handle, cCartID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Cart](result)
}

// =============================================================================
// Returns API
// =============================================================================

// ReturnsAPI provides return management operations
type ReturnsAPI struct {
	commerce *Commerce
}

// Create creates a return request
func (api *ReturnsAPI) Create(orderID string, reason ReturnReason, notes string) (*Return, error) {
	cOrderID := C.CString(orderID)
	cReason := C.CString(string(reason))
	cNotes := C.CString(notes)
	defer C.free(unsafe.Pointer(cOrderID))
	defer C.free(unsafe.Pointer(cReason))
	defer C.free(unsafe.Pointer(cNotes))

	result := C.stateset_return_create(api.commerce.handle, cOrderID, cReason, cNotes)
	return parseJSON[Return](result)
}

// Get retrieves a return by ID
func (api *ReturnsAPI) Get(id string) (*Return, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_return_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Return](result)
}

// List retrieves all returns
func (api *ReturnsAPI) List() ([]Return, error) {
	result := C.stateset_return_list(api.commerce.handle)
	return parseJSONArray[Return](result)
}

// Approve approves a return request
func (api *ReturnsAPI) Approve(id string) (*Return, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_return_approve(api.commerce.handle, cID)
	return parseJSON[Return](result)
}

// Reject rejects a return request
func (api *ReturnsAPI) Reject(id string, reason string) (*Return, error) {
	cID := C.CString(id)
	cReason := C.CString(reason)
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cReason))

	result := C.stateset_return_reject(api.commerce.handle, cID, cReason)
	return parseJSON[Return](result)
}

// Complete marks a return as completed
func (api *ReturnsAPI) Complete(id string) (*Return, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_return_complete(api.commerce.handle, cID)
	return parseJSON[Return](result)
}

// =============================================================================
// Payments API
// =============================================================================

// PaymentsAPI provides payment operations
type PaymentsAPI struct {
	commerce *Commerce
}

// Create creates a payment
func (api *PaymentsAPI) Create(orderID string, amount float64, currency string, method PaymentMethod) (*Payment, error) {
	cOrderID := C.CString(orderID)
	cCurrency := C.CString(currency)
	cMethod := C.CString(string(method))
	defer C.free(unsafe.Pointer(cOrderID))
	defer C.free(unsafe.Pointer(cCurrency))
	defer C.free(unsafe.Pointer(cMethod))

	result := C.stateset_payment_create(api.commerce.handle, cOrderID, C.double(amount), cCurrency, cMethod)
	return parseJSON[Payment](result)
}

// Get retrieves a payment by ID
func (api *PaymentsAPI) Get(id string) (*Payment, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_payment_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Payment](result)
}

// List retrieves all payments
func (api *PaymentsAPI) List() ([]Payment, error) {
	result := C.stateset_payment_list(api.commerce.handle)
	return parseJSONArray[Payment](result)
}

// Complete marks a payment as completed
func (api *PaymentsAPI) Complete(id string) (*Payment, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_payment_complete(api.commerce.handle, cID)
	return parseJSON[Payment](result)
}

// Fail marks a payment as failed
func (api *PaymentsAPI) Fail(id string, reason string) (*Payment, error) {
	cID := C.CString(id)
	cReason := C.CString(reason)
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cReason))

	result := C.stateset_payment_fail(api.commerce.handle, cID, cReason)
	return parseJSON[Payment](result)
}

// Refund creates a refund for a payment
func (api *PaymentsAPI) Refund(paymentID string, amount float64, reason string) (*Refund, error) {
	cPaymentID := C.CString(paymentID)
	cReason := C.CString(reason)
	defer C.free(unsafe.Pointer(cPaymentID))
	defer C.free(unsafe.Pointer(cReason))

	result := C.stateset_payment_refund(api.commerce.handle, cPaymentID, C.double(amount), cReason)
	return parseJSON[Refund](result)
}

// =============================================================================
// Analytics API
// =============================================================================

// AnalyticsAPI provides analytics operations
type AnalyticsAPI struct {
	commerce *Commerce
}

// GetSalesSummary retrieves sales summary for a time period
func (api *AnalyticsAPI) GetSalesSummary(period TimePeriod) (*SalesSummary, error) {
	cPeriod := C.CString(string(period))
	defer C.free(unsafe.Pointer(cPeriod))

	result := C.stateset_analytics_sales_summary(api.commerce.handle, cPeriod)
	return parseJSON[SalesSummary](result)
}

// GetTopProducts retrieves top selling products
func (api *AnalyticsAPI) GetTopProducts(limit int) ([]TopProduct, error) {
	result := C.stateset_analytics_top_products(api.commerce.handle, C.int(limit))
	return parseJSONArray[TopProduct](result)
}

// GetTopCustomers retrieves top customers by spend
func (api *AnalyticsAPI) GetTopCustomers(limit int) ([]TopCustomer, error) {
	result := C.stateset_analytics_top_customers(api.commerce.handle, C.int(limit))
	return parseJSONArray[TopCustomer](result)
}

// =============================================================================
// Shipments API
// =============================================================================

// ShipmentsAPI provides shipment management operations
type ShipmentsAPI struct {
	commerce *Commerce
}

// Create creates a new shipment
func (api *ShipmentsAPI) Create(orderID, recipientName, shippingAddress, carrier string) (*Shipment, error) {
	cOrderID := C.CString(orderID)
	cRecipientName := C.CString(recipientName)
	cShippingAddress := C.CString(shippingAddress)
	cCarrier := C.CString(carrier)
	defer C.free(unsafe.Pointer(cOrderID))
	defer C.free(unsafe.Pointer(cRecipientName))
	defer C.free(unsafe.Pointer(cShippingAddress))
	defer C.free(unsafe.Pointer(cCarrier))

	result := C.stateset_shipment_create(api.commerce.handle, cOrderID, cRecipientName, cShippingAddress, cCarrier)
	return parseJSON[Shipment](result)
}

// Get retrieves a shipment by ID
func (api *ShipmentsAPI) Get(id string) (*Shipment, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_shipment_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Shipment](result)
}

// List retrieves all shipments
func (api *ShipmentsAPI) List() ([]Shipment, error) {
	result := C.stateset_shipment_list(api.commerce.handle)
	return parseJSONArray[Shipment](result)
}

// Ship marks a shipment as shipped with tracking number
func (api *ShipmentsAPI) Ship(id, trackingNumber string) (*Shipment, error) {
	cID := C.CString(id)
	cTracking := C.CString(trackingNumber)
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cTracking))

	result := C.stateset_shipment_ship(api.commerce.handle, cID, cTracking)
	return parseJSON[Shipment](result)
}

// Deliver marks a shipment as delivered
func (api *ShipmentsAPI) Deliver(id string) (*Shipment, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_shipment_deliver(api.commerce.handle, cID)
	return parseJSON[Shipment](result)
}

// Cancel cancels a shipment
func (api *ShipmentsAPI) Cancel(id string) (*Shipment, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_shipment_cancel(api.commerce.handle, cID)
	return parseJSON[Shipment](result)
}

// =============================================================================
// Warranties API
// =============================================================================

// WarrantiesAPI provides warranty management operations
type WarrantiesAPI struct {
	commerce *Commerce
}

// Create creates a new warranty
func (api *WarrantiesAPI) Create(customerID, productID string, warrantyType WarrantyType, durationMonths int) (*Warranty, error) {
	cCustomerID := C.CString(customerID)
	cProductID := C.CString(productID)
	cWarrantyType := C.CString(string(warrantyType))
	defer C.free(unsafe.Pointer(cCustomerID))
	defer C.free(unsafe.Pointer(cProductID))
	defer C.free(unsafe.Pointer(cWarrantyType))

	result := C.stateset_warranty_create(api.commerce.handle, cCustomerID, cProductID, cWarrantyType, C.int(durationMonths))
	return parseJSON[Warranty](result)
}

// Get retrieves a warranty by ID
func (api *WarrantiesAPI) Get(id string) (*Warranty, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_warranty_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Warranty](result)
}

// List retrieves all warranties
func (api *WarrantiesAPI) List() ([]Warranty, error) {
	result := C.stateset_warranty_list(api.commerce.handle)
	return parseJSONArray[Warranty](result)
}

// CreateClaim creates a warranty claim
func (api *WarrantiesAPI) CreateClaim(warrantyID, issueDescription string) (*WarrantyClaim, error) {
	cWarrantyID := C.CString(warrantyID)
	cIssue := C.CString(issueDescription)
	defer C.free(unsafe.Pointer(cWarrantyID))
	defer C.free(unsafe.Pointer(cIssue))

	result := C.stateset_warranty_create_claim(api.commerce.handle, cWarrantyID, cIssue)
	return parseJSON[WarrantyClaim](result)
}

// ApproveClaim approves a warranty claim
func (api *WarrantiesAPI) ApproveClaim(claimID string) (*WarrantyClaim, error) {
	cClaimID := C.CString(claimID)
	defer C.free(unsafe.Pointer(cClaimID))

	result := C.stateset_warranty_approve_claim(api.commerce.handle, cClaimID)
	return parseJSON[WarrantyClaim](result)
}

// DenyClaim denies a warranty claim
func (api *WarrantiesAPI) DenyClaim(claimID, reason string) (*WarrantyClaim, error) {
	cClaimID := C.CString(claimID)
	cReason := C.CString(reason)
	defer C.free(unsafe.Pointer(cClaimID))
	defer C.free(unsafe.Pointer(cReason))

	result := C.stateset_warranty_deny_claim(api.commerce.handle, cClaimID, cReason)
	return parseJSON[WarrantyClaim](result)
}

// CompleteClaim completes a warranty claim with resolution
func (api *WarrantiesAPI) CompleteClaim(claimID string, resolution ClaimResolution) (*WarrantyClaim, error) {
	cClaimID := C.CString(claimID)
	cResolution := C.CString(string(resolution))
	defer C.free(unsafe.Pointer(cClaimID))
	defer C.free(unsafe.Pointer(cResolution))

	result := C.stateset_warranty_complete_claim(api.commerce.handle, cClaimID, cResolution)
	return parseJSON[WarrantyClaim](result)
}

// =============================================================================
// Suppliers API
// =============================================================================

// SuppliersAPI provides supplier management operations
type SuppliersAPI struct {
	commerce *Commerce
}

// Create creates a new supplier
func (api *SuppliersAPI) Create(name, email, phone string) (*Supplier, error) {
	cName := C.CString(name)
	cEmail := C.CString(email)
	cPhone := C.CString(phone)
	defer C.free(unsafe.Pointer(cName))
	defer C.free(unsafe.Pointer(cEmail))
	defer C.free(unsafe.Pointer(cPhone))

	result := C.stateset_supplier_create(api.commerce.handle, cName, cEmail, cPhone)
	return parseJSON[Supplier](result)
}

// Get retrieves a supplier by ID
func (api *SuppliersAPI) Get(id string) (*Supplier, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_supplier_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Supplier](result)
}

// List retrieves all suppliers
func (api *SuppliersAPI) List() ([]Supplier, error) {
	result := C.stateset_supplier_list(api.commerce.handle)
	return parseJSONArray[Supplier](result)
}

// =============================================================================
// Purchase Orders API
// =============================================================================

// PurchaseOrdersAPI provides purchase order management operations
type PurchaseOrdersAPI struct {
	commerce *Commerce
}

// Create creates a new purchase order
func (api *PurchaseOrdersAPI) Create(supplierID string, items []PurchaseOrderItem) (*PurchaseOrder, error) {
	cSupplierID := C.CString(supplierID)
	defer C.free(unsafe.Pointer(cSupplierID))

	itemsJSON, err := json.Marshal(items)
	if err != nil {
		return nil, err
	}
	cItemsJSON := C.CString(string(itemsJSON))
	defer C.free(unsafe.Pointer(cItemsJSON))

	result := C.stateset_purchase_order_create(api.commerce.handle, cSupplierID, cItemsJSON)
	return parseJSON[PurchaseOrder](result)
}

// Get retrieves a purchase order by ID
func (api *PurchaseOrdersAPI) Get(id string) (*PurchaseOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_purchase_order_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[PurchaseOrder](result)
}

// List retrieves all purchase orders
func (api *PurchaseOrdersAPI) List() ([]PurchaseOrder, error) {
	result := C.stateset_purchase_order_list(api.commerce.handle)
	return parseJSONArray[PurchaseOrder](result)
}

// Submit submits a purchase order for approval
func (api *PurchaseOrdersAPI) Submit(id string) (*PurchaseOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_purchase_order_submit(api.commerce.handle, cID)
	return parseJSON[PurchaseOrder](result)
}

// Approve approves a purchase order
func (api *PurchaseOrdersAPI) Approve(id, approvedBy string) (*PurchaseOrder, error) {
	cID := C.CString(id)
	cApprovedBy := C.CString(approvedBy)
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cApprovedBy))

	result := C.stateset_purchase_order_approve(api.commerce.handle, cID, cApprovedBy)
	return parseJSON[PurchaseOrder](result)
}

// Send sends a purchase order to the supplier
func (api *PurchaseOrdersAPI) Send(id string) (*PurchaseOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_purchase_order_send(api.commerce.handle, cID)
	return parseJSON[PurchaseOrder](result)
}

// Cancel cancels a purchase order
func (api *PurchaseOrdersAPI) Cancel(id string) (*PurchaseOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_purchase_order_cancel(api.commerce.handle, cID)
	return parseJSON[PurchaseOrder](result)
}

// =============================================================================
// Invoices API
// =============================================================================

// InvoicesAPI provides invoice management operations
type InvoicesAPI struct {
	commerce *Commerce
}

// Create creates a new invoice
func (api *InvoicesAPI) Create(customerID string, items []InvoiceItem, billingEmail string) (*Invoice, error) {
	cCustomerID := C.CString(customerID)
	cBillingEmail := C.CString(billingEmail)
	defer C.free(unsafe.Pointer(cCustomerID))
	defer C.free(unsafe.Pointer(cBillingEmail))

	itemsJSON, err := json.Marshal(items)
	if err != nil {
		return nil, err
	}
	cItemsJSON := C.CString(string(itemsJSON))
	defer C.free(unsafe.Pointer(cItemsJSON))

	result := C.stateset_invoice_create(api.commerce.handle, cCustomerID, cItemsJSON, cBillingEmail)
	return parseJSON[Invoice](result)
}

// Get retrieves an invoice by ID
func (api *InvoicesAPI) Get(id string) (*Invoice, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_invoice_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[Invoice](result)
}

// List retrieves all invoices
func (api *InvoicesAPI) List() ([]Invoice, error) {
	result := C.stateset_invoice_list(api.commerce.handle)
	return parseJSONArray[Invoice](result)
}

// Send sends an invoice
func (api *InvoicesAPI) Send(id string) (*Invoice, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_invoice_send(api.commerce.handle, cID)
	return parseJSON[Invoice](result)
}

// Void voids an invoice
func (api *InvoicesAPI) Void(id string) (*Invoice, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_invoice_void(api.commerce.handle, cID)
	return parseJSON[Invoice](result)
}

// RecordPayment records a payment against an invoice
func (api *InvoicesAPI) RecordPayment(id string, amount float64, paymentMethod string) (*Invoice, error) {
	cID := C.CString(id)
	cPaymentMethod := C.CString(paymentMethod)
	defer C.free(unsafe.Pointer(cID))
	defer C.free(unsafe.Pointer(cPaymentMethod))

	result := C.stateset_invoice_record_payment(api.commerce.handle, cID, C.double(amount), cPaymentMethod)
	return parseJSON[Invoice](result)
}

// GetOverdue retrieves all overdue invoices
func (api *InvoicesAPI) GetOverdue() ([]Invoice, error) {
	result := C.stateset_invoice_get_overdue(api.commerce.handle)
	return parseJSONArray[Invoice](result)
}

// =============================================================================
// BOM (Bill of Materials) API
// =============================================================================

// BOMAPI provides bill of materials management operations
type BOMAPI struct {
	commerce *Commerce
}

// Create creates a new BOM
func (api *BOMAPI) Create(productID, name, description string) (*BillOfMaterials, error) {
	cProductID := C.CString(productID)
	cName := C.CString(name)
	cDescription := C.CString(description)
	defer C.free(unsafe.Pointer(cProductID))
	defer C.free(unsafe.Pointer(cName))
	defer C.free(unsafe.Pointer(cDescription))

	result := C.stateset_bom_create(api.commerce.handle, cProductID, cName, cDescription)
	return parseJSON[BillOfMaterials](result)
}

// Get retrieves a BOM by ID
func (api *BOMAPI) Get(id string) (*BillOfMaterials, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_bom_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[BillOfMaterials](result)
}

// List retrieves all BOMs
func (api *BOMAPI) List() ([]BillOfMaterials, error) {
	result := C.stateset_bom_list(api.commerce.handle)
	return parseJSONArray[BillOfMaterials](result)
}

// AddComponent adds a component to a BOM
func (api *BOMAPI) AddComponent(bomID, name, componentSKU string, quantity float64) (*BOMComponent, error) {
	cBOMID := C.CString(bomID)
	cName := C.CString(name)
	cComponentSKU := C.CString(componentSKU)
	defer C.free(unsafe.Pointer(cBOMID))
	defer C.free(unsafe.Pointer(cName))
	defer C.free(unsafe.Pointer(cComponentSKU))

	result := C.stateset_bom_add_component(api.commerce.handle, cBOMID, cName, cComponentSKU, C.double(quantity))
	return parseJSON[BOMComponent](result)
}

// GetComponents retrieves components for a BOM
func (api *BOMAPI) GetComponents(bomID string) ([]BOMComponent, error) {
	cBOMID := C.CString(bomID)
	defer C.free(unsafe.Pointer(cBOMID))

	result := C.stateset_bom_get_components(api.commerce.handle, cBOMID)
	return parseJSONArray[BOMComponent](result)
}

// Activate activates a BOM
func (api *BOMAPI) Activate(id string) (*BillOfMaterials, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_bom_activate(api.commerce.handle, cID)
	return parseJSON[BillOfMaterials](result)
}

// =============================================================================
// Work Orders API
// =============================================================================

// WorkOrdersAPI provides work order management operations
type WorkOrdersAPI struct {
	commerce *Commerce
}

// Create creates a new work order
func (api *WorkOrdersAPI) Create(productID string, quantityToBuild float64, bomID string) (*WorkOrder, error) {
	cProductID := C.CString(productID)
	cBOMID := C.CString(bomID)
	defer C.free(unsafe.Pointer(cProductID))
	defer C.free(unsafe.Pointer(cBOMID))

	result := C.stateset_work_order_create(api.commerce.handle, cProductID, C.double(quantityToBuild), cBOMID)
	return parseJSON[WorkOrder](result)
}

// Get retrieves a work order by ID
func (api *WorkOrdersAPI) Get(id string) (*WorkOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_work_order_get(api.commerce.handle, cID)
	if result == nil {
		return nil, nil
	}
	return parseJSON[WorkOrder](result)
}

// List retrieves all work orders
func (api *WorkOrdersAPI) List() ([]WorkOrder, error) {
	result := C.stateset_work_order_list(api.commerce.handle)
	return parseJSONArray[WorkOrder](result)
}

// Start starts a work order
func (api *WorkOrdersAPI) Start(id string) (*WorkOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_work_order_start(api.commerce.handle, cID)
	return parseJSON[WorkOrder](result)
}

// Complete completes a work order with quantity produced
func (api *WorkOrdersAPI) Complete(id string, quantityCompleted float64) (*WorkOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_work_order_complete(api.commerce.handle, cID, C.double(quantityCompleted))
	return parseJSON[WorkOrder](result)
}

// Cancel cancels a work order
func (api *WorkOrdersAPI) Cancel(id string) (*WorkOrder, error) {
	cID := C.CString(id)
	defer C.free(unsafe.Pointer(cID))

	result := C.stateset_work_order_cancel(api.commerce.handle, cID)
	return parseJSON[WorkOrder](result)
}

// =============================================================================
// Currency API
// =============================================================================

// CurrencyAPI provides currency and exchange rate operations
type CurrencyAPI struct {
	commerce *Commerce
}

// SetRate sets an exchange rate between two currencies
func (api *CurrencyAPI) SetRate(fromCurrency, toCurrency Currency, rate float64) (*ExchangeRate, error) {
	cFrom := C.CString(string(fromCurrency))
	cTo := C.CString(string(toCurrency))
	defer C.free(unsafe.Pointer(cFrom))
	defer C.free(unsafe.Pointer(cTo))

	result := C.stateset_currency_set_rate(api.commerce.handle, cFrom, cTo, C.double(rate))
	return parseJSON[ExchangeRate](result)
}

// GetRate retrieves an exchange rate between two currencies
func (api *CurrencyAPI) GetRate(fromCurrency, toCurrency Currency) (*ExchangeRate, error) {
	cFrom := C.CString(string(fromCurrency))
	cTo := C.CString(string(toCurrency))
	defer C.free(unsafe.Pointer(cFrom))
	defer C.free(unsafe.Pointer(cTo))

	result := C.stateset_currency_get_rate(api.commerce.handle, cFrom, cTo)
	if result == nil {
		return nil, nil
	}
	return parseJSON[ExchangeRate](result)
}

// Convert converts an amount from one currency to another
func (api *CurrencyAPI) Convert(amount float64, fromCurrency, toCurrency Currency) (*ConversionResult, error) {
	cFrom := C.CString(string(fromCurrency))
	cTo := C.CString(string(toCurrency))
	defer C.free(unsafe.Pointer(cFrom))
	defer C.free(unsafe.Pointer(cTo))

	result := C.stateset_currency_convert(api.commerce.handle, C.double(amount), cFrom, cTo)
	return parseJSON[ConversionResult](result)
}

// GetSettings retrieves store currency settings
func (api *CurrencyAPI) GetSettings() (*StoreCurrencySettings, error) {
	result := C.stateset_currency_get_settings(api.commerce.handle)
	return parseJSON[StoreCurrencySettings](result)
}
