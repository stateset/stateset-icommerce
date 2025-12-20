package com.stateset.embedded;

/**
 * Exception thrown by StateSet operations.
 */
public class StateSetException extends RuntimeException {

    public StateSetException(String message) {
        super(message);
    }

    public StateSetException(String message, Throwable cause) {
        super(message, cause);
    }
}
