package com.stateset.embedded;

import java.util.Objects;

/**
 * Return request entity.
 */
public final class ReturnRequest {

    private final String id;
    private final String orderId;
    private final String customerId;
    private final String status;
    private final String reason;
    private final double refundAmount;
    private final String createdAt;
    private final String updatedAt;

    public ReturnRequest(
            String id,
            String orderId,
            String customerId,
            String status,
            String reason,
            double refundAmount,
            String createdAt,
            String updatedAt) {
        this.id = id;
        this.orderId = orderId;
        this.customerId = customerId;
        this.status = status;
        this.reason = reason;
        this.refundAmount = refundAmount;
        this.createdAt = createdAt;
        this.updatedAt = updatedAt;
    }

    public String getId() { return id; }
    public String getOrderId() { return orderId; }
    public String getCustomerId() { return customerId; }
    public String getStatus() { return status; }
    public String getReason() { return reason; }
    public double getRefundAmount() { return refundAmount; }
    public String getCreatedAt() { return createdAt; }
    public String getUpdatedAt() { return updatedAt; }

    @Override
    public String toString() {
        return "ReturnRequest{id=" + id + ", status=" + status + ", refund=" + refundAmount + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        ReturnRequest that = (ReturnRequest) o;
        return Objects.equals(id, that.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
