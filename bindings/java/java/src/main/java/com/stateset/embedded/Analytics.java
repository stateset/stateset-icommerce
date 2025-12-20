package com.stateset.embedded;

/**
 * Analytics API for sales reports.
 */
public final class Analytics {

    private final long nativePtr;

    Analytics(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Get sales summary for a time period.
     *
     * @param days Number of days to look back (0 for all time)
     * @return Sales summary
     */
    public SalesSummary salesSummary(int days) {
        return nativeSalesSummary(nativePtr, days);
    }

    /**
     * Get sales summary for the last 30 days.
     *
     * @return Sales summary
     */
    public SalesSummary salesSummary() {
        return salesSummary(30);
    }

    // Native methods
    private static native SalesSummary nativeSalesSummary(long ptr, int days);
}
