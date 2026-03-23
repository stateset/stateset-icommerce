//! Transaction abstraction with saga support
//!
//! Provides ACID transactions and distributed saga pattern for multi-step operations.

use thiserror::Error;

/// Error types for transaction operations
#[derive(Error, Debug)]
#[non_exhaustive]
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
    completed_operations: usize,
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
            completed_operations: 0,
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

        self.completed_operations = 0;

        // Execute all operations
        for op in &self.operations {
            match op.execute() {
                Ok(_) => {
                    self.completed_operations += 1;
                }
                Err(e) => {
                    self.state = TransactionState::Failed;
                    // Try to compensate
                    Self::compensate(&self.operations[..self.completed_operations])?;
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

        Self::compensate(&self.operations[..self.completed_operations])?;
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

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

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

    /// An operation whose compensation also fails.
    struct FailingCompensation;

    impl Transactional for FailingCompensation {
        type Output = ();

        fn execute(&self) -> TransactionResult<()> {
            Ok(())
        }

        fn compensate(&self) -> TransactionResult<()> {
            Err(TransactionError::CompensationError {
                step: "failing_comp".into(),
                error: "comp failed".into(),
            })
        }
    }

    /// Tracks execution order via an append-to-vec pattern.
    struct OrderTracker {
        log: Arc<Mutex<Vec<String>>>,
        label: String,
        should_fail: bool,
    }

    impl Transactional for OrderTracker {
        type Output = ();

        fn execute(&self) -> TransactionResult<()> {
            self.log.lock().unwrap().push(format!("exec:{}", self.label));
            if self.should_fail {
                Err(TransactionError::TransactionFailed(format!("{} failed", self.label)))
            } else {
                Ok(())
            }
        }

        fn compensate(&self) -> TransactionResult<()> {
            self.log.lock().unwrap().push(format!("comp:{}", self.label));
            Ok(())
        }
    }

    fn simple_op(name: &str) -> (SimpleOperation, Arc<Mutex<bool>>, Arc<Mutex<bool>>) {
        let executed = Arc::new(Mutex::new(false));
        let compensated = Arc::new(Mutex::new(false));
        (
            SimpleOperation {
                name: name.into(),
                executed: executed.clone(),
                compensated: compensated.clone(),
            },
            executed,
            compensated,
        )
    }

    // ------------------------------------------------------------------
    // Original tests (preserved)
    // ------------------------------------------------------------------

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
    fn test_rollback_without_executing_does_not_compensate() {
        let mut handle = TransactionHandle::new();

        let executed = Arc::new(Mutex::new(false));
        let compensated = Arc::new(Mutex::new(false));

        handle.add_operation(Box::new(SimpleOperation {
            name: "op1".into(),
            executed: executed.clone(),
            compensated: compensated.clone(),
        }));

        handle.rollback().unwrap();
        assert_eq!(handle.state(), TransactionState::RolledBack);
        assert!(!(*executed.lock().unwrap()));
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

    // ------------------------------------------------------------------
    // New tests — TransactionError Display
    // ------------------------------------------------------------------

    #[test]
    fn test_transaction_failed_display() {
        let err = TransactionError::TransactionFailed("timeout".into());
        let msg = format!("{err}");
        assert_eq!(msg, "Transaction failed: timeout");
    }

    #[test]
    fn test_compensation_error_display() {
        let err = TransactionError::CompensationError {
            step: "debit_account".into(),
            error: "insufficient funds".into(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("debit_account"));
        assert!(msg.contains("insufficient funds"));
    }

    #[test]
    fn test_already_committed_display() {
        let err = TransactionError::AlreadyCommitted;
        let msg = format!("{err}");
        assert_eq!(msg, "Transaction already committed");
    }

    #[test]
    fn test_already_rolledback_display() {
        let err = TransactionError::AlreadyRolledback;
        let msg = format!("{err}");
        assert_eq!(msg, "Transaction already rolled back");
    }

    #[test]
    fn test_not_active_display() {
        let err = TransactionError::NotActive;
        let msg = format!("{err}");
        assert_eq!(msg, "Transaction not active");
    }

    #[test]
    fn test_transaction_error_debug() {
        let err = TransactionError::TransactionFailed("boom".into());
        let dbg = format!("{err:?}");
        assert!(dbg.contains("TransactionFailed"));
    }

    // ------------------------------------------------------------------
    // New tests — TransactionState
    // ------------------------------------------------------------------

    #[test]
    fn test_transaction_state_initial_is_active() {
        let handle = TransactionHandle::new();
        assert_eq!(handle.state(), TransactionState::Active);
    }

    #[test]
    fn test_state_transitions_to_committed_on_success() {
        let mut handle = TransactionHandle::new();
        handle.commit().unwrap();
        assert_eq!(handle.state(), TransactionState::Committed);
    }

    #[test]
    fn test_state_transitions_to_rolled_back() {
        let mut handle = TransactionHandle::new();
        handle.rollback().unwrap();
        assert_eq!(handle.state(), TransactionState::RolledBack);
    }

    #[test]
    fn test_state_transitions_to_failed_on_error() {
        let mut handle = TransactionHandle::new();
        let (op, _, _) = simple_op("fail-me");
        handle.add_operation(Box::new(op));
        assert!(handle.commit().is_err());
        assert_eq!(handle.state(), TransactionState::Failed);
    }

    #[test]
    fn test_commit_after_commit_returns_not_active() {
        let mut handle = TransactionHandle::new();
        handle.commit().unwrap();
        let err = handle.commit().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_rollback_after_commit_returns_not_active() {
        let mut handle = TransactionHandle::new();
        handle.commit().unwrap();
        let err = handle.rollback().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_commit_after_rollback_returns_not_active() {
        let mut handle = TransactionHandle::new();
        handle.rollback().unwrap();
        let err = handle.commit().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_rollback_after_rollback_returns_not_active() {
        let mut handle = TransactionHandle::new();
        handle.rollback().unwrap();
        let err = handle.rollback().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_commit_after_failure_returns_not_active() {
        let mut handle = TransactionHandle::new();
        let (op, _, _) = simple_op("fail-op");
        handle.add_operation(Box::new(op));
        assert!(handle.commit().is_err());
        let err = handle.commit().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_rollback_after_failure_returns_not_active() {
        let mut handle = TransactionHandle::new();
        let (op, _, _) = simple_op("fail-op");
        handle.add_operation(Box::new(op));
        assert!(handle.commit().is_err());
        let err = handle.rollback().unwrap_err();
        assert!(matches!(err, TransactionError::NotActive));
    }

    #[test]
    fn test_transaction_state_equality() {
        assert_eq!(TransactionState::Active, TransactionState::Active);
        assert_ne!(TransactionState::Active, TransactionState::Committed);
        assert_ne!(TransactionState::Committed, TransactionState::RolledBack);
        assert_ne!(TransactionState::RolledBack, TransactionState::Failed);
    }

    #[test]
    fn test_transaction_state_clone() {
        let s = TransactionState::Active;
        let s2 = s;
        assert_eq!(s, s2);
    }

    #[test]
    fn test_transaction_state_debug() {
        let dbg = format!("{:?}", TransactionState::Committed);
        assert_eq!(dbg, "Committed");
    }

    // ------------------------------------------------------------------
    // New tests — TransactionHandle
    // ------------------------------------------------------------------

    #[test]
    fn test_handle_has_unique_id() {
        let h1 = TransactionHandle::new();
        let h2 = TransactionHandle::new();
        assert_ne!(h1.id(), h2.id());
    }

    #[test]
    fn test_handle_id_is_uuid_format() {
        let handle = TransactionHandle::new();
        // UUID v4 format: 8-4-4-4-12
        assert_eq!(handle.id().len(), 36);
        assert_eq!(handle.id().chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_handle_default_is_active() {
        let handle = TransactionHandle::default();
        assert_eq!(handle.state(), TransactionState::Active);
    }

    #[test]
    fn test_handle_debug() {
        let handle = TransactionHandle::new();
        let dbg = format!("{handle:?}");
        assert!(dbg.contains("TransactionHandle"));
        assert!(dbg.contains("Active"));
    }

    #[test]
    fn test_commit_empty_transaction_succeeds() {
        let mut handle = TransactionHandle::new();
        // No operations added — commit should succeed vacuously
        handle.commit().unwrap();
        assert_eq!(handle.state(), TransactionState::Committed);
    }

    #[test]
    fn test_rollback_empty_transaction_succeeds() {
        let mut handle = TransactionHandle::new();
        handle.rollback().unwrap();
        assert_eq!(handle.state(), TransactionState::RolledBack);
    }

    #[test]
    fn test_operations_execute_in_order() {
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        let mut handle = TransactionHandle::new();
        for label in ["first", "second", "third"] {
            handle.add_operation(Box::new(OrderTracker {
                log: log.clone(),
                label: label.into(),
                should_fail: false,
            }));
        }

        handle.commit().unwrap();
        let entries = log.lock().unwrap();
        assert_eq!(*entries, vec!["exec:first", "exec:second", "exec:third"]);
    }

    #[test]
    fn test_compensation_runs_in_reverse_order() {
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        let mut handle = TransactionHandle::new();
        handle.add_operation(Box::new(OrderTracker {
            log: log.clone(),
            label: "A".into(),
            should_fail: false,
        }));
        handle.add_operation(Box::new(OrderTracker {
            log: log.clone(),
            label: "B".into(),
            should_fail: false,
        }));
        handle.add_operation(Box::new(OrderTracker {
            log: log.clone(),
            label: "C".into(),
            should_fail: true, // This one fails
        }));

        assert!(handle.commit().is_err());
        let entries = log.lock().unwrap();
        // exec:A, exec:B, exec:C (fails), comp:B, comp:A (reverse of completed)
        assert_eq!(*entries, vec!["exec:A", "exec:B", "exec:C", "comp:B", "comp:A"]);
    }

    #[test]
    fn test_first_operation_fails_no_compensation_needed() {
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        let mut handle = TransactionHandle::new();
        handle.add_operation(Box::new(OrderTracker {
            log: log.clone(),
            label: "first".into(),
            should_fail: true,
        }));
        handle.add_operation(Box::new(OrderTracker {
            log: log.clone(),
            label: "second".into(),
            should_fail: false,
        }));

        assert!(handle.commit().is_err());
        let entries = log.lock().unwrap();
        // Only the first exec runs, and since completed_operations=0, no compensation
        assert_eq!(*entries, vec!["exec:first"]);
    }

    #[test]
    fn test_many_operations_all_succeed() {
        let mut handle = TransactionHandle::new();
        let mut flags = Vec::new();

        for i in 0..20 {
            let (op, exec, _comp) = simple_op(&format!("op{i}"));
            handle.add_operation(Box::new(op));
            flags.push(exec);
        }

        handle.commit().unwrap();
        assert_eq!(handle.state(), TransactionState::Committed);
        for (i, flag) in flags.iter().enumerate() {
            assert!(*flag.lock().unwrap(), "Operation {i} should have been executed");
        }
    }

    // ------------------------------------------------------------------
    // New tests — Saga
    // ------------------------------------------------------------------

    #[test]
    fn test_saga_new_creates_active_handle() {
        let saga = Saga::new();
        assert_eq!(saga.handle.state(), TransactionState::Active);
    }

    #[test]
    fn test_saga_default() {
        let saga = Saga::default();
        assert_eq!(saga.handle.state(), TransactionState::Active);
    }

    #[test]
    fn test_saga_debug() {
        let saga = Saga::new();
        let dbg = format!("{saga:?}");
        assert!(dbg.contains("Saga"));
    }

    #[test]
    fn test_saga_executes_all_steps() {
        let (op1, exec1, _) = simple_op("step1");
        let (op2, exec2, _) = simple_op("step2");

        let mut saga = Saga::new();
        saga.add_step(Box::new(op1));
        saga.add_step(Box::new(op2));
        saga.execute().unwrap();

        assert!(*exec1.lock().unwrap());
        assert!(*exec2.lock().unwrap());
    }

    #[test]
    fn test_saga_compensates_on_failure() {
        let (op1, exec1, comp1) = simple_op("step1");
        let (op2, _exec2, _comp2) = simple_op("step2-fail");

        let mut saga = Saga::new();
        saga.add_step(Box::new(op1));
        saga.add_step(Box::new(op2));

        assert!(saga.execute().is_err());
        assert!(*exec1.lock().unwrap());
        assert!(*comp1.lock().unwrap());
    }

    #[test]
    fn test_saga_add_step_returns_self() {
        let (op, _, _) = simple_op("x");
        let mut saga = Saga::new();
        let returned = saga.add_step(Box::new(op));
        // Should return &mut Self for chaining
        assert_eq!(returned.handle.state(), TransactionState::Active);
    }

    #[test]
    fn test_saga_empty_execute_succeeds() {
        let mut saga = Saga::new();
        saga.execute().unwrap();
    }

    // ------------------------------------------------------------------
    // New tests — TransactionBuilder
    // ------------------------------------------------------------------

    #[test]
    fn test_builder_new() {
        let builder = TransactionBuilder::new();
        let dbg = format!("{builder:?}");
        assert!(dbg.contains("TransactionBuilder"));
        assert!(dbg.contains("0"));
    }

    #[test]
    fn test_builder_default() {
        let builder = TransactionBuilder::default();
        let dbg = format!("{builder:?}");
        assert!(dbg.contains("TransactionBuilder"));
    }

    #[test]
    fn test_builder_empty_execute_succeeds() {
        let result = TransactionBuilder::new().execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_chaining() {
        let (op1, exec1, _) = simple_op("a");
        let (op2, exec2, _) = simple_op("b");
        let (op3, exec3, _) = simple_op("c");

        TransactionBuilder::new()
            .push(Box::new(op1))
            .push(Box::new(op2))
            .push(Box::new(op3))
            .execute()
            .unwrap();

        assert!(*exec1.lock().unwrap());
        assert!(*exec2.lock().unwrap());
        assert!(*exec3.lock().unwrap());
    }

    #[test]
    fn test_builder_failure_compensates() {
        let (op1, _exec1, comp1) = simple_op("ok");
        let (op2, _, _) = simple_op("fail-here");

        let result = TransactionBuilder::new().push(Box::new(op1)).push(Box::new(op2)).execute();

        assert!(result.is_err());
        assert!(*comp1.lock().unwrap());
    }

    // ------------------------------------------------------------------
    // TransactionResult type alias
    // ------------------------------------------------------------------

    #[test]
    fn test_transaction_result_ok() {
        fn returns_ok() -> TransactionResult<i32> {
            Ok(42)
        }
        let value = returns_ok();
        assert_eq!(value.unwrap(), 42);
    }

    #[test]
    fn test_transaction_result_err() {
        let result: TransactionResult<()> = Err(TransactionError::NotActive);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Compensation failure
    // ------------------------------------------------------------------

    #[test]
    fn test_failing_compensation_propagates_error() {
        let mut handle = TransactionHandle::new();
        handle.add_operation(Box::new(FailingCompensation));

        let (fail_op, _, _) = simple_op("fail-trigger");
        handle.add_operation(Box::new(fail_op));

        let err = handle.commit().unwrap_err();
        // The commit fails due to the second op, then compensation of the first
        // op also fails, so we should get a CompensationError
        assert!(
            matches!(err, TransactionError::CompensationError { .. }),
            "Expected CompensationError, got: {err:?}"
        );
    }
}
