package stateset

/*
#include <stdlib.h>

typedef void* StateSetHandle;

extern char* stateset_customer_create(StateSetHandle handle, const char* email, const char* first_name, const char* last_name, const char* phone);
extern char* stateset_customer_get(StateSetHandle handle, const char* id);
extern char* stateset_customer_list(StateSetHandle handle);
extern int stateset_customer_delete(StateSetHandle handle, const char* id);

extern char* stateset_product_create(StateSetHandle handle, const char* name, const char* sku, double price, const char* description);
extern char* stateset_product_get(StateSetHandle handle, const char* id);
extern char* stateset_product_list(StateSetHandle handle);

extern char* stateset_order_create(StateSetHandle handle, const char* customer_id, const char* items_json, const char* currency);
extern char* stateset_order_get(StateSetHandle handle, const char* id);
extern char* stateset_order_list(StateSetHandle handle);
extern char* stateset_order_update_status(StateSetHandle handle, const char* id, const char* status);

extern char* stateset_inventory_create_item(StateSetHandle handle, const char* sku, const char* name, double initial_quantity);
extern int stateset_inventory_adjust(StateSetHandle handle, const char* sku, double quantity_delta, const char* reason);
extern char* stateset_inventory_get_level(StateSetHandle handle, const char* sku);

extern char* stateset_cart_create(StateSetHandle handle, const char* customer_id, const char* currency);
extern char* stateset_cart_add_item(StateSetHandle handle, const char* cart_id, const char* variant_id, int quantity);
extern char* stateset_cart_get(StateSetHandle handle, const char* cart_id);

extern char* stateset_return_create(StateSetHandle handle, const char* order_id, const char* reason, const char* notes);
extern char* stateset_return_list(StateSetHandle handle);

extern char* stateset_payment_create(StateSetHandle handle, const char* order_id, double amount, const char* currency, const char* method);

extern char* stateset_analytics_sales_summary(StateSetHandle handle, const char* period);
extern char* stateset_analytics_top_products(StateSetHandle handle, int limit);
extern char* stateset_analytics_top_customers(StateSetHandle handle, int limit);
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

// List retrieves all returns
func (api *ReturnsAPI) List() ([]Return, error) {
	result := C.stateset_return_list(api.commerce.handle)
	return parseJSONArray[Return](result)
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
