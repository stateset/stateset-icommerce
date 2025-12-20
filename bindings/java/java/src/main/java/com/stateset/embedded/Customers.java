package com.stateset.embedded;

import java.util.Arrays;
import java.util.List;
import java.util.Optional;

/**
 * Customers API for managing customer records.
 */
public final class Customers {

    private final long nativePtr;

    Customers(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a new customer.
     *
     * @param email Customer email address
     * @param firstName First name
     * @param lastName Last name
     * @param phone Phone number (optional)
     * @return The created customer
     */
    public Customer create(String email, String firstName, String lastName, String phone) {
        return nativeCreate(nativePtr, email, firstName, lastName, phone != null ? phone : "");
    }

    /**
     * Get a customer by ID.
     *
     * @param id Customer UUID
     * @return Optional containing the customer if found
     */
    public Optional<Customer> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * Get a customer by email.
     *
     * @param email Customer email
     * @return Optional containing the customer if found
     */
    public Optional<Customer> getByEmail(String email) {
        return Optional.ofNullable(nativeGetByEmail(nativePtr, email));
    }

    /**
     * List all customers.
     *
     * @return List of all customers
     */
    public List<Customer> list() {
        Customer[] arr = nativeList(nativePtr);
        return arr != null ? Arrays.asList(arr) : List.of();
    }

    /**
     * Count customers.
     *
     * @return Total number of customers
     */
    public long count() {
        return nativeCount(nativePtr);
    }

    // Native methods
    private static native Customer nativeCreate(long ptr, String email, String firstName, String lastName, String phone);
    private static native Customer nativeGet(long ptr, String id);
    private static native Customer nativeGetByEmail(long ptr, String email);
    private static native Customer[] nativeList(long ptr);
    private static native long nativeCount(long ptr);
}
