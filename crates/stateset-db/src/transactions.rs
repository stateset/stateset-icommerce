//! Transaction abstraction with saga support
//!
//! Provides ACID transactions and distributed saga pattern for multi-step operations.

use thiserror::Error;

/// Error types for transaction operations
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Compensating action failed for step {step}: {error}")]
    CompensationError { step: String, error: String },

    #[error("Transaction already committed")]
    AlreadyCommitted,

    #[error("Transaction already rolled back")]
    AlreadyRolledback,

    #[error("Transaction not active")]
    NotActive,
}

/// Result type for transaction operations
pub type TransactionResult<T> = Result<T, TransactionError>;

/// Transaction lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction is active and can execute steps
    Active,
    /// Transaction completed successfully
    Committed,
    /// Transaction rolled back
    RolledBack,
    /// Transaction failed during execution
    Failed,
}

/// Trait for operations that can participate in transactions
pub trait Transactional {
    type Output;

    /// Execute the operation
    fn execute(&self) -> TransactionResult<Self::Output>;

    /// Compensate/rollback the operation
    fn compensate(&self) -> TransactionResult<()>;
}

/// Trait for repository-level transaction support
pub trait TransactionalRepository {
    /// Begin a new transaction
    fn begin_transaction(&self) -> TransactionResult<TransactionHandle>;

    /// Execute operations within a transaction
    fn with_transaction<F, T>(&self, f: F) -> TransactionResult<T>
    where
        F: FnOnce(&mut Self) -> TransactionResult<T>;
}

/// Handle for an active transaction
pub struct TransactionHandle {
    id: String,
    state: TransactionState,
    operations: Vec<Box<dyn Transactional<Output = ()>>>,
}

impl std::fmt::Debug for TransactionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionHandle")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("operations_len", &self.operations.len())
            .finish()
    }
}

impl TransactionHandle {
    /// Create a new transaction handle
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            state: TransactionState::Active,
            operations: Vec::new(),
        }
    }

    /// Get the transaction ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current state
    pub const fn state(&self) -> TransactionState {
        self.state
    }

    /// Add an operation to the transaction
    pub fn add_operation(&mut self, operation: Box<dyn Transactional<Output = ()>>) {
        self.operations.push(operation);
    }

    /// Commit the transaction (execute all operations)
    pub fn commit(&mut self) -> TransactionResult<()> {
        if self.state != TransactionState::Active {
            return Err(TransactionError::NotActive);
        }

        let mut completed = 0usize;

        // Execute all operations
        for op in &self.operations {
            match op.execute() {
                Ok(_) => {
                    completed += 1;
                }
                Err(e) => {
                    self.state = TransactionState::Failed;
                    // Try to compensate
                    Self::compensate(&self.operations[..completed])?;
                    return Err(e);
                }
            }
        }

        self.state = TransactionState::Committed;
        Ok(())
    }

    /// Rollback the transaction by compensating completed operations
    pub fn rollback(&mut self) -> TransactionResult<()> {
        if self.state != TransactionState::Active {
            return Err(TransactionError::NotActive);
        }

        Self::compensate(&self.operations)?;
        self.state = TransactionState::RolledBack;
        Ok(())
    }

    /// Compensate completed operations (called on rollback or failure)
    fn compensate(completed: &[Box<dyn Transactional<Output = ()>>]) -> TransactionResult<()> {
        // Compensate in reverse order
        for op in completed.iter().rev() {
            if let Err(e) = op.compensate() {
                return Err(TransactionError::CompensationError {
                    step: "unknown".to_string(),
                    error: e.to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for TransactionHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Saga pattern for distributed transactions
///
/// Sagas execute a sequence of operations with compensating actions
/// to handle failures gracefully.
#[derive(Debug)]
pub struct Saga {
    handle: TransactionHandle,
}

impl Saga {
    /// Create a new saga
    pub fn new() -> Self {
        Self { handle: TransactionHandle::new() }
    }

    /// Add a step to the saga
    pub fn add_step(&mut self, operation: Box<dyn Transactional<Output = ()>>) -> &mut Self {
        self.handle.add_operation(operation);
        self
    }

    /// Execute the saga
    pub fn execute(&mut self) -> TransactionResult<()> {
        self.handle.commit()
    }
}

impl Default for Saga {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for transaction batches
#[must_use]
pub struct TransactionBuilder {
    operations: Vec<Box<dyn Transactional<Output = ()>>>,
}

impl std::fmt::Debug for TransactionBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionBuilder")
            .field("operations_len", &self.operations.len())
            .finish()
    }
}

impl TransactionBuilder {
    /// Create a new transaction builder
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    /// Add an operation to the batch.
    pub fn push(mut self, operation: Box<dyn Transactional<Output = ()>>) -> Self {
        self.operations.push(operation);
        self
    }

    /// Build and execute the transaction
    pub fn execute(self) -> TransactionResult<()> {
        let mut handle = TransactionHandle::new();
        for op in self.operations {
            handle.add_operation(op);
        }
        handle.commit()
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct SimpleOperation {
        name: String,
        executed: Arc<Mutex<bool>>,
        compensated: Arc<Mutex<bool>>,
    }

    impl Transactional for SimpleOperation {
        type Output = ();

        fn execute(&self) -> TransactionResult<()> {
            *self.executed.lock().unwrap() = true;
            // Simulate failure for testing
            if self.name.contains("fail") {
                Err(TransactionError::TransactionFailed("Simulated failure".into()))
            } else {
                Ok(())
            }
        }

        fn compensate(&self) -> TransactionResult<()> {
            *self.compensated.lock().unwrap() = true;
            Ok(())
        }
    }

    #[test]
    fn test_successful_transaction() {
        let mut handle = TransactionHandle::new();

        let executed = Arc::new(Mutex::new(false));
        let compensated = Arc::new(Mutex::new(false));

        handle.add_operation(Box::new(SimpleOperation {
            name: "op1".into(),
            executed: executed.clone(),
            compensated: compensated.clone(),
        }));

        handle.commit().unwrap();
        assert_eq!(handle.state(), TransactionState::Committed);
        assert!(*executed.lock().unwrap());
        assert!(!(*compensated.lock().unwrap()));
    }

    #[test]
    fn test_failed_transaction_with_compensation() {
        let mut handle = TransactionHandle::new();

        let executed1 = Arc::new(Mutex::new(false));
        let compensated1 = Arc::new(Mutex::new(false));
        let executed2 = Arc::new(Mutex::new(false));
        let compensated2 = Arc::new(Mutex::new(false));

        handle.add_operation(Box::new(SimpleOperation {
            name: "op1".into(),
            executed: executed1.clone(),
            compensated: compensated1.clone(),
        }));

        handle.add_operation(Box::new(SimpleOperation {
            name: "op2-fail".into(),
            executed: executed2.clone(),
            compensated: compensated2.clone(),
        }));

        assert!(handle.commit().is_err());
        assert_eq!(handle.state(), TransactionState::Failed);
        assert!(*executed1.lock().unwrap());
        assert!(*executed2.lock().unwrap());
        assert!(*compensated1.lock().unwrap());
        assert!(!(*compensated2.lock().unwrap()));
    }

    #[test]
    fn test_saga_builder() {
        let executed1 = Arc::new(Mutex::new(false));
        let executed2 = Arc::new(Mutex::new(false));

        let result = TransactionBuilder::new()
            .push(Box::new(SimpleOperation {
                name: "op1".into(),
                executed: executed1.clone(),
                compensated: Arc::new(Mutex::new(false)),
            }))
            .push(Box::new(SimpleOperation {
                name: "op2".into(),
                executed: executed2.clone(),
                compensated: Arc::new(Mutex::new(false)),
            }))
            .execute();

        assert!(result.is_ok());
        assert!(*executed1.lock().unwrap());
        assert!(*executed2.lock().unwrap());
    }
}
