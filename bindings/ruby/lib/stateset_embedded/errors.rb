# frozen_string_literal: true

module StateSet
  # Base class for every error the engine reports.
  #
  # @!attribute [r] code
  #   Stable machine code: the engine invariant code when there is one
  #   (`"commerce.refund.exceeds_captured"`), else the engine error variant
  #   (`"OrderNotFound"`), else a `"binding.*"` code.
  # @!attribute [r] status
  #   The engine's suggested HTTP-style status (404, 400, 409, ...).
  class Error < StandardError
    attr_reader :code, :status

    def initialize(message = nil, code: nil, status: nil)
      super(message)
      @code = code
      @status = status
    end
  end

  # The referenced entity does not exist.
  class NotFoundError < Error; end

  # The input is malformed or breaks a money/validation invariant. Also raised
  # for bad binding arguments (unknown keyword, invalid UUID, Float money).
  class ValidationError < Error; end

  # Duplicate key (email, SKU, slug) or a concurrent-modification conflict.
  class ConflictError < Error; end

  # The operation is not permitted for the caller.
  class NotPermittedError < Error; end

  # The request is well-formed but refused in the entity's current state
  # (invalid status transition, order not cancellable, ...).
  class InvalidOperationError < Error; end

  # Not enough available stock to satisfy the request.
  class InsufficientStockError < InvalidOperationError; end

  # The storage layer failed.
  class DatabaseError < Error; end

  # An external provider the engine called failed.
  class ExternalServiceError < Error; end

  # An engine bug: an internal error or a panic caught at the boundary.
  class InternalError < Error; end

  # Engine error kind (from the native envelope) -> exception class.
  ERROR_CLASSES = {
    'not_found' => NotFoundError,
    'validation' => ValidationError,
    'conflict' => ConflictError,
    'not_permitted' => NotPermittedError,
    'invalid_operation' => InvalidOperationError,
    'insufficient_stock' => InsufficientStockError,
    'database' => DatabaseError,
    'external_service' => ExternalServiceError,
    'internal' => InternalError
  }.freeze
end
