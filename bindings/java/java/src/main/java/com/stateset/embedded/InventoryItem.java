package com.stateset.embedded;

import java.util.Objects;

/**
 * Inventory item entity.
 */
public final class InventoryItem {

    private final String id;
    private final String sku;
    private final int quantityOnHand;
    private final int quantityReserved;
    private final int quantityAvailable;
    private final int reorderPoint;
    private final int reorderQuantity;

    public InventoryItem(
            String id,
            String sku,
            int quantityOnHand,
            int quantityReserved,
            int quantityAvailable,
            int reorderPoint,
            int reorderQuantity) {
        this.id = id;
        this.sku = sku;
        this.quantityOnHand = quantityOnHand;
        this.quantityReserved = quantityReserved;
        this.quantityAvailable = quantityAvailable;
        this.reorderPoint = reorderPoint;
        this.reorderQuantity = reorderQuantity;
    }

    public String getId() { return id; }
    public String getSku() { return sku; }
    public int getQuantityOnHand() { return quantityOnHand; }
    public int getQuantityReserved() { return quantityReserved; }
    public int getQuantityAvailable() { return quantityAvailable; }
    public int getReorderPoint() { return reorderPoint; }
    public int getReorderQuantity() { return reorderQuantity; }

    public boolean isLowStock() {
        return reorderPoint > 0 && quantityAvailable <= reorderPoint;
    }

    @Override
    public String toString() {
        return "InventoryItem{sku=" + sku + ", available=" + quantityAvailable + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        InventoryItem that = (InventoryItem) o;
        return Objects.equals(id, that.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
