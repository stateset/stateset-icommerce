#ifndef STATESET_H
#define STATESET_H

#include <stdint.h>
#include <stdbool.h>

typedef struct Arc_Mutex_RustCommerce Arc_Mutex_RustCommerce;

typedef struct Arc_Mutex_RustCommerce CommerceHandle;

/**
 * Free a string allocated by Rust
 */
void stateset_string_free(char *s);

/**
 * Create a new Commerce instance
 * Returns a handle pointer, or null on error
 */
CommerceHandle *stateset_commerce_new(const char *db_path);

/**
 * Destroy a Commerce instance
 */
void stateset_commerce_free(CommerceHandle *handle);

/**
 * Create a customer, returns JSON string (caller must free)
 */
char *stateset_customer_create(CommerceHandle *handle,
                               const char *email,
                               const char *first_name,
                               const char *last_name,
                               const char *phone);

/**
 * Get a customer by ID, returns JSON string (caller must free)
 */
char *stateset_customer_get(CommerceHandle *handle, const char *id);

/**
 * List all customers, returns JSON array string (caller must free)
 */
char *stateset_customer_list(CommerceHandle *handle);

/**
 * Delete a customer by ID, returns 1 on success, 0 on failure
 */
int stateset_customer_delete(CommerceHandle *handle, const char *id);

/**
 * Create a product, returns JSON string (caller must free)
 */
char *stateset_product_create(CommerceHandle *handle,
                              const char *name,
                              const char *sku,
                              double price,
                              const char *description);

/**
 * Get a product by ID, returns JSON string (caller must free)
 */
char *stateset_product_get(CommerceHandle *handle, const char *id);

/**
 * List all products, returns JSON array string (caller must free)
 */
char *stateset_product_list(CommerceHandle *handle);

/**
 * Create an order, returns JSON string (caller must free)
 */
char *stateset_order_create(CommerceHandle *handle,
                            const char *customer_id,
                            const char *items_json,
                            const char *currency);

/**
 * Get an order by ID, returns JSON string (caller must free)
 */
char *stateset_order_get(CommerceHandle *handle, const char *id);

/**
 * List all orders, returns JSON array string (caller must free)
 */
char *stateset_order_list(CommerceHandle *handle);

/**
 * Update order status, returns JSON string (caller must free)
 */
char *stateset_order_update_status(CommerceHandle *handle, const char *id, const char *status);

/**
 * Create an inventory item, returns JSON string (caller must free)
 */
char *stateset_inventory_create_item(CommerceHandle *handle,
                                     const char *sku,
                                     const char *name,
                                     double initial_quantity);

/**
 * Adjust inventory, returns 1 on success, 0 on failure
 */
int stateset_inventory_adjust(CommerceHandle *handle,
                              const char *sku,
                              double quantity_delta,
                              const char *reason);

/**
 * Get stock level, returns JSON string (caller must free)
 */
char *stateset_inventory_get_level(CommerceHandle *handle, const char *sku);

/**
 * Create a cart, returns JSON string (caller must free)
 */
char *stateset_cart_create(CommerceHandle *handle, const char *customer_id, const char *currency);

/**
 * Add item to cart, returns JSON string (caller must free)
 */
char *stateset_cart_add_item(CommerceHandle *handle,
                             const char *cart_id,
                             const char *variant_id,
                             int quantity);

/**
 * Get cart, returns JSON string (caller must free)
 */
char *stateset_cart_get(CommerceHandle *handle, const char *cart_id);

/**
 * Create a return, returns JSON string (caller must free)
 */
char *stateset_return_create(CommerceHandle *handle,
                             const char *order_id,
                             const char *reason,
                             const char *notes);

/**
 * List all returns, returns JSON array string (caller must free)
 */
char *stateset_return_list(CommerceHandle *handle);

/**
 * Create a payment, returns JSON string (caller must free)
 */
char *stateset_payment_create(CommerceHandle *handle,
                              const char *order_id,
                              double amount,
                              const char *currency,
                              const char *method);

/**
 * Get sales summary, returns JSON string (caller must free)
 */
char *stateset_analytics_sales_summary(CommerceHandle *handle, const char *period);

/**
 * Get top products, returns JSON array string (caller must free)
 */
char *stateset_analytics_top_products(CommerceHandle *handle, int limit);

/**
 * Get top customers, returns JSON array string (caller must free)
 */
char *stateset_analytics_top_customers(CommerceHandle *handle, int limit);

/**
 * Create a quality inspection
 */
char *stateset_quality_create_inspection(CommerceHandle *handle,
                                         const char *reference_type,
                                         const char *reference_id,
                                         const char *inspection_type);

/**
 * List quality inspections
 */
char *stateset_quality_list_inspections(CommerceHandle *handle);

/**
 * Create a warehouse
 */
char *stateset_warehouse_create(CommerceHandle *handle, const char *code, const char *name);

/**
 * List warehouses
 */
char *stateset_warehouse_list(CommerceHandle *handle);

/**
 * Create a location in a warehouse
 */
char *stateset_warehouse_create_location(CommerceHandle *handle,
                                         int warehouse_id,
                                         const char *code);

/**
 * Create a lot
 */
char *stateset_lots_create(CommerceHandle *handle,
                           const char *sku,
                           const char *lot_number,
                           double quantity);

/**
 * List lots
 */
char *stateset_lots_list(CommerceHandle *handle);

/**
 * Create a serial number
 */
char *stateset_serials_create(CommerceHandle *handle, const char *sku, const char *serial);

/**
 * List serial numbers
 */
char *stateset_serials_list(CommerceHandle *handle);

/**
 * Create a bill
 */
char *stateset_ap_create_bill(CommerceHandle *handle, const char *supplier_id, double amount);

/**
 * List bills
 */
char *stateset_ap_list_bills(CommerceHandle *handle);

/**
 * Get AP aging summary
 */
char *stateset_ap_aging_summary(CommerceHandle *handle);

/**
 * Get AR aging summary
 */
char *stateset_ar_aging_summary(CommerceHandle *handle);

/**
 * Get Days Sales Outstanding
 */
double stateset_ar_get_dso(CommerceHandle *handle, int days);

/**
 * Set item cost
 */
char *stateset_cost_set_item_cost(CommerceHandle *handle, const char *sku, double standard_cost);

/**
 * Get item cost
 */
char *stateset_cost_get_item_cost(CommerceHandle *handle, const char *sku);

/**
 * Create credit account
 */
char *stateset_credit_create_account(CommerceHandle *handle,
                                     const char *customer_id,
                                     double credit_limit);

/**
 * Check credit
 */
int stateset_credit_check(CommerceHandle *handle, const char *customer_id, double amount);

/**
 * Create backorder
 */
char *stateset_backorder_create(CommerceHandle *handle,
                                const char *order_id,
                                const char *customer_id,
                                const char *sku,
                                double quantity);

/**
 * List backorders
 */
char *stateset_backorder_list(CommerceHandle *handle);

/**
 * Create GL account
 */
char *stateset_gl_create_account(CommerceHandle *handle,
                                 const char *account_number,
                                 const char *name,
                                 const char *account_type);

/**
 * Get trial balance
 */
char *stateset_gl_trial_balance(CommerceHandle *handle);

#endif  /* STATESET_H */
