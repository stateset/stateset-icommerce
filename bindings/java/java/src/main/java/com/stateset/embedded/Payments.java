package com.stateset.embedded;

/**
 * Payments API for recording payments.
 */
public final class Payments {

    private final long nativePtr;

    Payments(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Record a payment for an order.
     *
     * @param orderId Order UUID
     * @param amount Payment amount
     * @param method Payment method (optional)
     * @return true if successful
     */
    public boolean record(String orderId, double amount, String method) {
        return nativeRecord(nativePtr, orderId, amount, method != null ? method : "");
    }

    // Native methods
    private static native boolean nativeRecord(long ptr, String orderId, double amount, String method);
}
