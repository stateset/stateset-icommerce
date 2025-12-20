package com.stateset.embedded;

/**
 * StateSet Embedded Commerce - Main entry point.
 *
 * <p>Provides a complete commerce API with embedded SQLite storage.
 * All operations are performed locally without network calls.
 *
 * <p>Example usage:
 * <pre>{@code
 * Commerce commerce = new Commerce("store.db");
 *
 * Customer customer = commerce.customers().create(
 *     "alice@example.com", "Alice", "Smith", null
 * );
 *
 * Order order = commerce.orders().create(
 *     customer.getId(),
 *     "[{\"sku\":\"SKU-001\",\"name\":\"Widget\",\"quantity\":2,\"unit_price\":29.99}]",
 *     "USD"
 * );
 *
 * commerce.close();
 * }</pre>
 */
public class Commerce implements AutoCloseable {

    static {
        NativeLoader.load();
    }

    private long nativePtr;
    private final Customers customers;
    private final Orders orders;
    private final Products products;
    private final Inventory inventory;
    private final Returns returns;
    private final Payments payments;
    private final Carts carts;
    private final Analytics analytics;

    /**
     * Create a new Commerce instance.
     *
     * @param dbPath Path to SQLite database file, or ":memory:" for in-memory database
     * @throws StateSetException if database initialization fails
     */
    public Commerce(String dbPath) {
        this.nativePtr = nativeCreate(dbPath);
        if (this.nativePtr == 0) {
            throw new StateSetException("Failed to create commerce instance");
        }

        this.customers = new Customers(this.nativePtr);
        this.orders = new Orders(this.nativePtr);
        this.products = new Products(this.nativePtr);
        this.inventory = new Inventory(this.nativePtr);
        this.returns = new Returns(this.nativePtr);
        this.payments = new Payments(this.nativePtr);
        this.carts = new Carts(this.nativePtr);
        this.analytics = new Analytics(this.nativePtr);
    }

    /**
     * Get the Customers API.
     * @return Customers API instance
     */
    public Customers customers() {
        return customers;
    }

    /**
     * Get the Orders API.
     * @return Orders API instance
     */
    public Orders orders() {
        return orders;
    }

    /**
     * Get the Products API.
     * @return Products API instance
     */
    public Products products() {
        return products;
    }

    /**
     * Get the Inventory API.
     * @return Inventory API instance
     */
    public Inventory inventory() {
        return inventory;
    }

    /**
     * Get the Returns API.
     * @return Returns API instance
     */
    public Returns returns() {
        return returns;
    }

    /**
     * Get the Payments API.
     * @return Payments API instance
     */
    public Payments payments() {
        return payments;
    }

    /**
     * Get the Carts API.
     * @return Carts API instance
     */
    public Carts carts() {
        return carts;
    }

    /**
     * Get the Analytics API.
     * @return Analytics API instance
     */
    public Analytics analytics() {
        return analytics;
    }

    /**
     * Close the commerce instance and release native resources.
     */
    @Override
    public void close() {
        if (nativePtr != 0) {
            nativeDestroy(nativePtr);
            nativePtr = 0;
        }
    }

    // Native methods
    private static native long nativeCreate(String dbPath);
    private static native void nativeDestroy(long ptr);
}
