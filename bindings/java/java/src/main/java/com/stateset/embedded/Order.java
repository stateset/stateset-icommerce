package com.stateset.embedded;

import java.util.Objects;

/**
 * Order entity.
 */
public final class Order {

    private final String id;
    private final String orderNumber;
    private final String customerId;
    private final String status;
    private final double totalAmount;
    private final String currency;
    private final String paymentStatus;
    private final String fulfillmentStatus;
    private final String createdAt;
    private final String updatedAt;

    public Order(
            String id,
            String orderNumber,
            String customerId,
            String status,
            double totalAmount,
            String currency,
            String paymentStatus,
            String fulfillmentStatus,
            String createdAt,
            String updatedAt) {
        this.id = id;
        this.orderNumber = orderNumber;
        this.customerId = customerId;
        this.status = status;
        this.totalAmount = totalAmount;
        this.currency = currency;
        this.paymentStatus = paymentStatus;
        this.fulfillmentStatus = fulfillmentStatus;
        this.createdAt = createdAt;
        this.updatedAt = updatedAt;
    }

    public String getId() { return id; }
    public String getOrderNumber() { return orderNumber; }
    public String getCustomerId() { return customerId; }
    public String getStatus() { return status; }
    public double getTotalAmount() { return totalAmount; }
    public String getCurrency() { return currency; }
    public String getPaymentStatus() { return paymentStatus; }
    public String getFulfillmentStatus() { return fulfillmentStatus; }
    public String getCreatedAt() { return createdAt; }
    public String getUpdatedAt() { return updatedAt; }

    @Override
    public String toString() {
        return "Order{number=" + orderNumber + ", status=" + status + ", total=" + totalAmount + " " + currency + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Order order = (Order) o;
        return Objects.equals(id, order.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
