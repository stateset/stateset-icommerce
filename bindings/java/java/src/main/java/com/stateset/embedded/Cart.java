package com.stateset.embedded;

import java.util.Objects;

/**
 * Shopping cart entity.
 */
public final class Cart {

    private final String id;
    private final String customerId;
    private final String status;
    private final double subtotal;
    private final double total;
    private final String currency;
    private final String createdAt;
    private final String updatedAt;

    public Cart(
            String id,
            String customerId,
            String status,
            double subtotal,
            double total,
            String currency,
            String createdAt,
            String updatedAt) {
        this.id = id;
        this.customerId = customerId;
        this.status = status;
        this.subtotal = subtotal;
        this.total = total;
        this.currency = currency;
        this.createdAt = createdAt;
        this.updatedAt = updatedAt;
    }

    public String getId() { return id; }
    public String getCustomerId() { return customerId.isEmpty() ? null : customerId; }
    public String getStatus() { return status; }
    public double getSubtotal() { return subtotal; }
    public double getTotal() { return total; }
    public String getCurrency() { return currency; }
    public String getCreatedAt() { return createdAt; }
    public String getUpdatedAt() { return updatedAt; }

    @Override
    public String toString() {
        return "Cart{id=" + id + ", status=" + status + ", total=" + total + " " + currency + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Cart cart = (Cart) o;
        return Objects.equals(id, cart.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
