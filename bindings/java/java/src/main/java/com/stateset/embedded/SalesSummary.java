package com.stateset.embedded;

/**
 * Sales summary analytics.
 */
public final class SalesSummary {

    private final double totalRevenue;
    private final int totalOrders;
    private final int totalItemsSold;
    private final double averageOrderValue;

    public SalesSummary(
            double totalRevenue,
            int totalOrders,
            int totalItemsSold,
            double averageOrderValue) {
        this.totalRevenue = totalRevenue;
        this.totalOrders = totalOrders;
        this.totalItemsSold = totalItemsSold;
        this.averageOrderValue = averageOrderValue;
    }

    public double getTotalRevenue() { return totalRevenue; }
    public int getTotalOrders() { return totalOrders; }
    public int getTotalItemsSold() { return totalItemsSold; }
    public double getAverageOrderValue() { return averageOrderValue; }

    @Override
    public String toString() {
        return String.format(
            "SalesSummary{revenue=%.2f, orders=%d, items=%d, aov=%.2f}",
            totalRevenue, totalOrders, totalItemsSold, averageOrderValue
        );
    }
}
