package com.stateset.embedded;

import java.util.Arrays;
import java.util.List;
import java.util.Optional;

/**
 * Products API for managing products.
 */
public final class Products {

    private final long nativePtr;

    Products(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a new product.
     *
     * @param name Product name
     * @param description Product description (optional)
     * @param vendor Vendor name (optional)
     * @param productType Product type/category (optional)
     * @return The created product
     */
    public Product create(String name, String description, String vendor, String productType) {
        return nativeCreate(nativePtr, name,
            description != null ? description : "",
            vendor != null ? vendor : "",
            productType != null ? productType : "");
    }

    /**
     * Get a product by ID.
     *
     * @param id Product UUID
     * @return Optional containing the product if found
     */
    public Optional<Product> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * List all products.
     *
     * @return List of all products
     */
    public List<Product> list() {
        Product[] arr = nativeList(nativePtr);
        return arr != null ? Arrays.asList(arr) : List.of();
    }

    /**
     * Count products.
     *
     * @return Total number of products
     */
    public long count() {
        return nativeCount(nativePtr);
    }

    // Native methods
    private static native Product nativeCreate(long ptr, String name, String description, String vendor, String productType);
    private static native Product nativeGet(long ptr, String id);
    private static native Product[] nativeList(long ptr);
    private static native long nativeCount(long ptr);
}
