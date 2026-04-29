package com.stateset.embedded;

import java.util.List;
import java.util.Optional;

/**
 * Returns API for managing return requests.
 */
public final class Returns {

    private final long nativePtr;

    Returns(long nativePtr) {
        this.nativePtr = nativePtr;
    }

    /**
     * Create a return request.
     *
     * @param orderId Order UUID
     * @param reason Return reason
     * @return The created return request
     */
    public ReturnRequest create(String orderId, String reason) {
        return nativeCreate(nativePtr, orderId, reason);
    }

    /**
     * Get a return request by ID.
     *
     * @param id Return request UUID
     * @return Optional containing the return if found
     */
    public Optional<ReturnRequest> get(String id) {
        return Optional.ofNullable(nativeGet(nativePtr, id));
    }

    /**
     * List all return requests.
     *
     * @return List of all returns
     */
    public List<ReturnRequest> list() {
        List<ReturnRequest> returns = nativeList(nativePtr);
        return returns != null ? returns : List.of();
    }

    /**
     * Approve a return request.
     *
     * @param id Return request UUID
     * @param refundAmount Override refund amount (0 for default)
     * @return The updated return request
     */
    public ReturnRequest approve(String id, double refundAmount) {
        return nativeApprove(nativePtr, id, refundAmount);
    }

    /**
     * Reject a return request.
     *
     * @param id Return request UUID
     * @param reason Rejection reason (optional)
     * @return The updated return request
     */
    public ReturnRequest reject(String id, String reason) {
        return nativeReject(nativePtr, id, reason != null ? reason : "");
    }

    // Native methods
    private static native ReturnRequest nativeCreate(long ptr, String orderId, String reason);
    private static native ReturnRequest nativeGet(long ptr, String id);
    private static native List<ReturnRequest> nativeList(long ptr);
    private static native ReturnRequest nativeApprove(long ptr, String id, double refundAmount);
    private static native ReturnRequest nativeReject(long ptr, String id, String reason);
}
