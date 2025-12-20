package com.stateset.embedded;

import java.util.Arrays;
import java.util.List;
import java.util.Optional;

/**
 * Inventory API for managing inventory.
 */
public final class Inventory {

    private final long nativePtr;

    Inventory(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a new inventory item.
     *
     * @param sku SKU code
     * @param quantity Initial quantity
     * @param reorderPoint Low stock threshold (0 to disable)
     * @param reorderQuantity Reorder quantity (0 to disable)
     * @return The created inventory item
     */
    public InventoryItem create(String sku, int quantity, int reorderPoint, int reorderQuantity) {
        return nativeCreate(nativePtr, sku, quantity, reorderPoint, reorderQuantity);
    }

    /**
     * Get an inventory item by ID.
     *
     * @param id Inventory item UUID
     * @return Optional containing the item if found
     */
    public Optional<InventoryItem> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * Get an inventory item by SKU.
     *
     * @param sku SKU code
     * @return Optional containing the item if found
     */
    public Optional<InventoryItem> getBySku(String sku) {
        return Optional.ofNullable(nativeGetBySku(nativePtr, sku));
    }

    /**
     * List all inventory items.
     *
     * @return List of all inventory items
     */
    public List<InventoryItem> list() {
        InventoryItem[] arr = nativeList(nativePtr);
        return arr != null ? Arrays.asList(arr) : List.of();
    }

    /**
     * Adjust inventory quantity.
     *
     * @param id Inventory item UUID
     * @param adjustment Quantity change (positive or negative)
     * @param reason Adjustment reason (optional)
     * @return The updated inventory item
     */
    public InventoryItem adjust(String id, int adjustment, String reason) {
        return nativeAdjust(nativePtr, id, adjustment, reason != null ? reason : "");
    }

    /**
     * Reserve inventory for an order.
     *
     * @param id Inventory item UUID
     * @param quantity Quantity to reserve
     * @param orderId Order UUID (optional)
     * @return The updated inventory item
     */
    public InventoryItem reserve(String id, int quantity, String orderId) {
        return nativeReserve(nativePtr, id, quantity, orderId != null ? orderId : "");
    }

    /**
     * Release reserved inventory.
     *
     * @param id Inventory item UUID
     * @param quantity Quantity to release
     * @return The updated inventory item
     */
    public InventoryItem release(String id, int quantity) {
        return nativeRelease(nativePtr, id, quantity);
    }

    // Native methods
    private static native InventoryItem nativeCreate(long ptr, String sku, int quantity, int reorderPoint, int reorderQuantity);
    private static native InventoryItem nativeGet(long ptr, String id);
    private static native InventoryItem nativeGetBySku(long ptr, String sku);
    private static native InventoryItem[] nativeList(long ptr);
    private static native InventoryItem nativeAdjust(long ptr, String id, int adjustment, String reason);
    private static native InventoryItem nativeReserve(long ptr, String id, int quantity, String orderId);
    private static native InventoryItem nativeRelease(long ptr, String id, int quantity);
}
