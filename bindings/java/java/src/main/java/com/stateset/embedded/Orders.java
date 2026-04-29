package com.stateset.embedded;

import java.util.List;
import java.util.Optional;

/**
 * Orders API for managing orders.
 */
public final class Orders {

    private final long nativePtr;

    Orders(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a new order.
     *
     * @param customerId Customer UUID
     * @param itemsJson JSON array of items: [{"sku":"SKU","name":"Name","quantity":1,"unit_price":9.99}]
     * @param currency Currency code (optional, defaults to "USD")
     * @return The created order
     */
    public Order create(String customerId, String itemsJson, String currency) {
        return nativeCreate(nativePtr, customerId, itemsJson, currency != null ? currency : "");
    }

    /**
     * Get an order by ID.
     *
     * @param id Order UUID
     * @return Optional containing the order if found
     */
    public Optional<Order> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * List all orders.
     *
     * @return List of all orders
     */
    public List<Order> list() {
        List<Order> orders = nativeList(nativePtr);
        return orders != null ? orders : List.of();
    }

    /**
     * Count orders.
     *
     * @return Total number of orders
     */
    public long count() {
        return nativeCount(nativePtr);
    }

    /**
     * Ship an order.
     *
     * @param id Order UUID
     * @param trackingNumber Tracking number (optional)
     * @param carrier Shipping carrier (optional)
     * @return The updated order
     */
    public Order ship(String id, String trackingNumber, String carrier) {
        return nativeShip(nativePtr, id,
            trackingNumber != null ? trackingNumber : "",
            carrier != null ? carrier : "");
    }

    /**
     * Cancel an order.
     *
     * @param id Order UUID
     * @param reason Cancellation reason (optional)
     * @return The updated order
     */
    public Order cancel(String id, String reason) {
        return nativeCancel(nativePtr, id, reason != null ? reason : "");
    }

    /**
     * Confirm an order.
     *
     * @param id Order UUID
     * @return The updated order
     */
    public Order confirm(String id) {
        return nativeConfirm(nativePtr, id);
    }

    /**
     * Mark an order as delivered.
     *
     * @param id Order UUID
     * @return The updated order
     */
    public Order deliver(String id) {
        return nativeDeliver(nativePtr, id);
    }

    // Native methods
    private static native Order nativeCreate(long ptr, String customerId, String itemsJson, String currency);
    private static native Order nativeGet(long ptr, String id);
    private static native List<Order> nativeList(long ptr);
    private static native long nativeCount(long ptr);
    private static native Order nativeShip(long ptr, String id, String trackingNumber, String carrier);
    private static native Order nativeCancel(long ptr, String id, String reason);
    private static native Order nativeConfirm(long ptr, String id);
    private static native Order nativeDeliver(long ptr, String id);
}
