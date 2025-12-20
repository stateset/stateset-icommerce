package com.stateset.embedded;

import java.util.Optional;

/**
 * Carts API for shopping cart management.
 */
public final class Carts {

    private final long nativePtr;

    Carts(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a new cart.
     *
     * @param customerId Customer UUID (optional for guest carts)
     * @param currency Currency code (optional, defaults to "USD")
     * @return The created cart
     */
    public Cart create(String customerId, String currency) {
        return nativeCreate(nativePtr,
            customerId != null ? customerId : "",
            currency != null ? currency : "");
    }

    /**
     * Get a cart by ID.
     *
     * @param id Cart UUID
     * @return Optional containing the cart if found
     */
    public Optional<Cart> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * Add an item to a cart.
     *
     * @param cartId Cart UUID
     * @param sku SKU code
     * @param name Item name
     * @param quantity Quantity
     * @param unitPrice Price per unit
     * @return The updated cart
     */
    public Cart addItem(String cartId, String sku, String name, int quantity, double unitPrice) {
        return nativeAddItem(nativePtr, cartId, sku, name, quantity, unitPrice);
    }

    /**
     * Checkout a cart, creating an order.
     *
     * @param cartId Cart UUID
     * @return The created order
     */
    public Order checkout(String cartId) {
        return nativeCheckout(nativePtr, cartId);
    }

    // Native methods
    private static native Cart nativeCreate(long ptr, String customerId, String currency);
    private static native Cart nativeGet(long ptr, String id);
    private static native Cart nativeAddItem(long ptr, String cartId, String sku, String name, int quantity, double unitPrice);
    private static native Order nativeCheckout(long ptr, String cartId);
}
