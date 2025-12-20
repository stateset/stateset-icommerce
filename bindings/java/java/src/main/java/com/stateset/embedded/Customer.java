package com.stateset.embedded;

import java.util.Objects;

/**
 * Customer entity.
 */
public final class Customer {

    private final String id;
    private final String email;
    private final String firstName;
    private final String lastName;
    private final String phone;
    private final String status;
    private final String createdAt;
    private final String updatedAt;

    public Customer(
            String id,
            String email,
            String firstName,
            String lastName,
            String phone,
            String status,
            String createdAt,
            String updatedAt) {
        this.id = id;
        this.email = email;
        this.firstName = firstName;
        this.lastName = lastName;
        this.phone = phone;
        this.status = status;
        this.createdAt = createdAt;
        this.updatedAt = updatedAt;
    }

    public String getId() { return id; }
    public String getEmail() { return email; }
    public String getFirstName() { return firstName; }
    public String getLastName() { return lastName; }
    public String getPhone() { return phone.isEmpty() ? null : phone; }
    public String getStatus() { return status; }
    public String getCreatedAt() { return createdAt; }
    public String getUpdatedAt() { return updatedAt; }

    public String getFullName() {
        return firstName + " " + lastName;
    }

    @Override
    public String toString() {
        return "Customer{id=" + id + ", email=" + email + ", name=" + getFullName() + "}";
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Customer customer = (Customer) o;
        return Objects.equals(id, customer.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }
}
