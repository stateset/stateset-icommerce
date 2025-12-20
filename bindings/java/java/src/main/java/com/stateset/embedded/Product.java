package com.stateset.embedded;

import java.util.Objects;

/**
 * Product entity.
 */
public final class Product {

    private final String id;
    private final String name;
    private final String description;
    private final String vendor;
    private final String productType;
    private final String status;
    private final String createdAt;
    private final String updatedAt;

    public Product(
            String id,
            String name,
            String description,
            String vendor,
            String productType,
            String status,
            String createdAt,
            String updatedAt) {
        this.id = id;
        this.name = name;
        this.description = description;
        this.vendor = vendor;
        this.productType = productType;
        this.status = status;
        this.createdAt = createdAt;
        this.updatedAt = updatedAt;
    }

    public String getId() { return id; }
    public String getName() { return name; }
    public String getDescription() { return description.isEmpty() ? null : description; }
    public String getVendor() { return vendor.isEmpty() ? null : vendor; }
    public String getProductType() { return productType.isEmpty() ? null : productType; }
    public String getStatus() { return status; }
    public String getCreatedAt() { return createdAt; }
    public String getUpdatedAt() { return updatedAt; }

    @Override
    public String toString() {
        return "Product{id=" + id + ", name=" + name + ", status=" + status + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Product product = (Product) o;
        return Objects.equals(id, product.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
