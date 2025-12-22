// Package stateset provides Go bindings for StateSet Embedded Commerce.
//
// StateSet is the SQLite of commerce - a zero-dependency, local-first
// commerce engine that runs anywhere.
//
// Example usage:
//
//	commerce, err := stateset.New("store.db")
//	if err != nil {
//	    log.Fatal(err)
//	}
//	defer commerce.Close()
//
//	customer, err := commerce.Customers().Create("alice@example.com", "Alice", "Smith", "")
//	if err != nil {
//	    log.Fatal(err)
//	}
//
//	fmt.Printf("Created customer: %s\n", customer.ID)
package stateset

/*
#cgo LDFLAGS: -L${SRCDIR}/../../../target/release -lstateset_go -lm -ldl -lpthread
#cgo linux LDFLAGS: -Wl,-rpath,${SRCDIR}/../../../target/release
#cgo darwin LDFLAGS: -Wl,-rpath,${SRCDIR}/../../../target/release

#include <stdlib.h>

typedef void* StateSetHandle;

// Memory management
extern void stateset_free_string(char* s);

// Commerce lifecycle
extern StateSetHandle stateset_new(const char* db_path);
extern void stateset_free(StateSetHandle handle);

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
extern char* stateset_return_list(StateSetHandle handle);

// Payments API
extern char* stateset_payment_create(StateSetHandle handle, const char* order_id, double amount, const char* currency, const char* method);

// Analytics API
extern char* stateset_analytics_sales_summary(StateSetHandle handle, const char* period);
extern char* stateset_analytics_top_products(StateSetHandle handle, int limit);
extern char* stateset_analytics_top_customers(StateSetHandle handle, int limit);
*/
import "C"

import (
	"encoding/json"
	"errors"
	"unsafe"
)

// ErrNullHandle is returned when the commerce handle is null
var ErrNullHandle = errors.New("null commerce handle")

// ErrOperationFailed is returned when a native operation fails
var ErrOperationFailed = errors.New("operation failed")

// Commerce is the main entry point for the StateSet commerce engine
type Commerce struct {
	handle         C.StateSetHandle
	customers      *CustomersAPI
	products       *ProductsAPI
	orders         *OrdersAPI
	inventory      *InventoryAPI
	carts          *CartsAPI
	returns        *ReturnsAPI
	payments       *PaymentsAPI
	analytics      *AnalyticsAPI
	shipments      *ShipmentsAPI
	warranties     *WarrantiesAPI
	suppliers      *SuppliersAPI
	purchaseOrders *PurchaseOrdersAPI
	invoices       *InvoicesAPI
	bom            *BOMAPI
	workOrders     *WorkOrdersAPI
	currency       *CurrencyAPI
}

// New creates a new Commerce instance
// dbPath can be a file path or ":memory:" for an in-memory database
func New(dbPath string) (*Commerce, error) {
	cPath := C.CString(dbPath)
	defer C.free(unsafe.Pointer(cPath))

	handle := C.stateset_new(cPath)
	if handle == nil {
		return nil, errors.New("failed to create commerce instance")
	}

	c := &Commerce{handle: handle}
	c.customers = &CustomersAPI{commerce: c}
	c.products = &ProductsAPI{commerce: c}
	c.orders = &OrdersAPI{commerce: c}
	c.inventory = &InventoryAPI{commerce: c}
	c.carts = &CartsAPI{commerce: c}
	c.returns = &ReturnsAPI{commerce: c}
	c.payments = &PaymentsAPI{commerce: c}
	c.analytics = &AnalyticsAPI{commerce: c}
	c.shipments = &ShipmentsAPI{commerce: c}
	c.warranties = &WarrantiesAPI{commerce: c}
	c.suppliers = &SuppliersAPI{commerce: c}
	c.purchaseOrders = &PurchaseOrdersAPI{commerce: c}
	c.invoices = &InvoicesAPI{commerce: c}
	c.bom = &BOMAPI{commerce: c}
	c.workOrders = &WorkOrdersAPI{commerce: c}
	c.currency = &CurrencyAPI{commerce: c}

	return c, nil
}

// Close releases the commerce instance resources
func (c *Commerce) Close() {
	if c.handle != nil {
		C.stateset_free(c.handle)
		c.handle = nil
	}
}

// Customers returns the Customers API
func (c *Commerce) Customers() *CustomersAPI {
	return c.customers
}

// Products returns the Products API
func (c *Commerce) Products() *ProductsAPI {
	return c.products
}

// Orders returns the Orders API
func (c *Commerce) Orders() *OrdersAPI {
	return c.orders
}

// Inventory returns the Inventory API
func (c *Commerce) Inventory() *InventoryAPI {
	return c.inventory
}

// Carts returns the Carts API
func (c *Commerce) Carts() *CartsAPI {
	return c.carts
}

// Returns returns the Returns API
func (c *Commerce) Returns() *ReturnsAPI {
	return c.returns
}

// Payments returns the Payments API
func (c *Commerce) Payments() *PaymentsAPI {
	return c.payments
}

// Analytics returns the Analytics API
func (c *Commerce) Analytics() *AnalyticsAPI {
	return c.analytics
}

// Shipments returns the Shipments API
func (c *Commerce) Shipments() *ShipmentsAPI {
	return c.shipments
}

// Warranties returns the Warranties API
func (c *Commerce) Warranties() *WarrantiesAPI {
	return c.warranties
}

// Suppliers returns the Suppliers API
func (c *Commerce) Suppliers() *SuppliersAPI {
	return c.suppliers
}

// PurchaseOrders returns the Purchase Orders API
func (c *Commerce) PurchaseOrders() *PurchaseOrdersAPI {
	return c.purchaseOrders
}

// Invoices returns the Invoices API
func (c *Commerce) Invoices() *InvoicesAPI {
	return c.invoices
}

// BOM returns the Bill of Materials API
func (c *Commerce) BOM() *BOMAPI {
	return c.bom
}

// WorkOrders returns the Work Orders API
func (c *Commerce) WorkOrders() *WorkOrdersAPI {
	return c.workOrders
}

// Currency returns the Currency API
func (c *Commerce) Currency() *CurrencyAPI {
	return c.currency
}

// Helper to convert C string to Go string and free the C string
func goStringFree(cstr *C.char) string {
	if cstr == nil {
		return ""
	}
	s := C.GoString(cstr)
	C.stateset_free_string(cstr)
	return s
}

// Helper to parse JSON response
func parseJSON[T any](cstr *C.char) (*T, error) {
	if cstr == nil {
		return nil, ErrOperationFailed
	}
	jsonStr := goStringFree(cstr)
	if jsonStr == "" {
		return nil, ErrOperationFailed
	}

	var result T
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// Helper to parse JSON array response
func parseJSONArray[T any](cstr *C.char) ([]T, error) {
	if cstr == nil {
		return nil, nil
	}
	jsonStr := goStringFree(cstr)
	if jsonStr == "" {
		return nil, nil
	}

	var result []T
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, err
	}
	return result, nil
}
