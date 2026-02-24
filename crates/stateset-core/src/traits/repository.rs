//! Generic repository trait with associated types.
//!
//! This module provides a generic [`Repository`] trait inspired by reth's `Table`
//! trait pattern. It defines CRUD operations using associated types for the entity,
//! its ID, create/update inputs, and filter criteria.
//!
//! Domain-specific repository traits (e.g. [`OrderRepository`](super::OrderRepository))
//! remain the primary API. The generic trait serves as a shared vocabulary and
//! enables writing code that's polymorphic over any entity type.
//!
//! # Example
//!
//! ```rust
//! use stateset_core::traits::Repository;
//!
//! fn count_all<R: Repository>(repo: &R) -> stateset_core::Result<u64>
//! where
//!     R::Filter: Default,
//! {
//!     repo.count(R::Filter::default())
//! }
//! ```

use crate::errors::{BatchResult, Result};
use std::fmt::Debug;

/// Generic CRUD repository interface with associated types.
///
/// This trait captures the common shape of all entity repositories:
/// create, get, update, delete, list, and count. Domain-specific operations
/// (like `OrderRepository::add_item`) live on the domain trait, not here.
///
/// # Type Parameters
///
/// All associated types require `Send + Sync` to be compatible with both
/// sync and async execution contexts.
///
/// # Blanket Implementations
///
/// The [`auto_impl`] attribute provides automatic implementations for
/// `&T`, `&mut T`, `Box<T>`, and `Arc<T>`.
#[auto_impl::auto_impl(&, &mut, Box, Arc)]
pub trait Repository: Send + Sync + Debug {
    /// The entity type returned by this repository.
    type Entity: Send + Sync;

    /// The identifier type used to look up entities.
    type Id: Send + Sync + Clone;

    /// The input type for creating new entities.
    type CreateInput: Send + Sync;

    /// The input type for updating existing entities.
    type UpdateInput: Send + Sync;

    /// The filter/query type for listing entities.
    type Filter: Send + Sync + Default;

    /// Create a new entity.
    fn create(&self, input: Self::CreateInput) -> Result<Self::Entity>;

    /// Get an entity by its ID. Returns `None` if not found.
    fn get(&self, id: Self::Id) -> Result<Option<Self::Entity>>;

    /// Update an existing entity.
    fn update(&self, id: Self::Id, input: Self::UpdateInput) -> Result<Self::Entity>;

    /// Delete an entity by ID.
    fn delete(&self, id: Self::Id) -> Result<()>;

    /// List entities matching the given filter.
    fn list(&self, filter: Self::Filter) -> Result<Vec<Self::Entity>>;

    /// Count entities matching the given filter.
    fn count(&self, filter: Self::Filter) -> Result<u64>;

    /// Get multiple entities by their IDs.
    ///
    /// The default implementation calls `get` in a loop. Backends may override
    /// this with a more efficient batch query.
    fn get_batch(&self, ids: Vec<Self::Id>) -> Result<Vec<Self::Entity>> {
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(entity) = self.get(id)? {
                results.push(entity);
            }
        }
        Ok(results)
    }

    /// Create multiple entities with partial-success semantics.
    ///
    /// The default implementation calls `create` in a loop, collecting
    /// successes and failures. Backends may override with a more efficient
    /// batch insert.
    fn create_batch(&self, inputs: Vec<Self::CreateInput>) -> Result<BatchResult<Self::Entity>> {
        let mut result = BatchResult::with_capacity(inputs.len());
        for (i, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(entity) => result.record_success(entity),
                Err(err) => result.record_failure(i, None, &err),
            }
        }
        Ok(result)
    }
}

/// Extension trait for repositories that support transactions.
///
/// This trait is intentionally separate from [`Repository`] because not all
/// backends support transactions (e.g. in-memory stores).
#[auto_impl::auto_impl(&, &mut, Box, Arc)]
pub trait Transactional: Send + Sync {
    /// Begin a transaction.
    fn begin_transaction(&self) -> Result<()>;

    /// Commit the current transaction.
    fn commit(&self) -> Result<()>;

    /// Rollback the current transaction.
    fn rollback(&self) -> Result<()>;

    /// Execute a closure within a transaction, automatically rolling back on
    /// error and committing on success.
    ///
    /// The default implementation uses `begin_transaction`, `commit`, and
    /// `rollback`.
    fn with_transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.begin_transaction()?;
        match f() {
            Ok(val) => {
                self.commit()?;
                Ok(val)
            }
            Err(err) => {
                // Best-effort rollback — the original error is more important.
                let _ = self.rollback();
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::CommerceError;
    use std::sync::Arc;

    // A minimal in-memory repository for testing the generic trait.
    #[derive(Debug)]
    struct InMemoryRepo {
        items: std::sync::Mutex<Vec<(u64, String)>>,
        next_id: std::sync::Mutex<u64>,
    }

    impl InMemoryRepo {
        fn new() -> Self {
            Self { items: std::sync::Mutex::new(Vec::new()), next_id: std::sync::Mutex::new(1) }
        }
    }

    #[derive(Debug, Default)]
    struct NoFilter;

    impl Repository for InMemoryRepo {
        type Entity = (u64, String);
        type Id = u64;
        type CreateInput = String;
        type UpdateInput = String;
        type Filter = NoFilter;

        fn create(&self, input: String) -> Result<(u64, String)> {
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            let entity = (current, input);
            self.items.lock().unwrap().push(entity.clone());
            Ok(entity)
        }

        fn get(&self, id: u64) -> Result<Option<(u64, String)>> {
            Ok(self.items.lock().unwrap().iter().find(|(i, _)| *i == id).cloned())
        }

        fn update(&self, id: u64, input: String) -> Result<(u64, String)> {
            let mut items = self.items.lock().unwrap();
            if let Some(item) = items.iter_mut().find(|(i, _)| *i == id) {
                item.1 = input;
                Ok(item.clone())
            } else {
                Err(CommerceError::NotFound)
            }
        }

        fn delete(&self, id: u64) -> Result<()> {
            let mut items = self.items.lock().unwrap();
            items.retain(|(i, _)| *i != id);
            Ok(())
        }

        fn list(&self, _filter: NoFilter) -> Result<Vec<(u64, String)>> {
            Ok(self.items.lock().unwrap().clone())
        }

        fn count(&self, _filter: NoFilter) -> Result<u64> {
            Ok(self.items.lock().unwrap().len() as u64)
        }
    }

    #[test]
    fn crud_operations() {
        let repo = InMemoryRepo::new();
        let entity = repo.create("hello".to_string()).unwrap();
        assert_eq!(entity.0, 1);
        assert_eq!(entity.1, "hello");

        let found = repo.get(1).unwrap();
        assert_eq!(found, Some((1, "hello".to_string())));

        let updated = repo.update(1, "world".to_string()).unwrap();
        assert_eq!(updated.1, "world");

        assert_eq!(repo.count(NoFilter).unwrap(), 1);

        repo.delete(1).unwrap();
        assert_eq!(repo.count(NoFilter).unwrap(), 0);
    }

    #[test]
    fn get_batch_default() {
        let repo = InMemoryRepo::new();
        repo.create("a".to_string()).unwrap();
        repo.create("b".to_string()).unwrap();
        repo.create("c".to_string()).unwrap();

        let batch = repo.get_batch(vec![1, 3]).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].1, "a");
        assert_eq!(batch[1].1, "c");
    }

    #[test]
    fn create_batch_default() {
        let repo = InMemoryRepo::new();
        let result =
            repo.create_batch(vec!["x".to_string(), "y".to_string(), "z".to_string()]).unwrap();
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failure_count, 0);
        assert!(result.all_succeeded());
    }

    // Test that auto_impl works with Arc<T>
    fn _takes_arc_repo(
        _: Arc<
            dyn Repository<
                    Entity = (),
                    Id = u64,
                    CreateInput = (),
                    UpdateInput = (),
                    Filter = NoFilter,
                >,
        >,
    ) {
    }

    // Test that auto_impl works with Box<T>
    fn _takes_box_repo(
        _: Box<
            dyn Repository<
                    Entity = (),
                    Id = u64,
                    CreateInput = (),
                    UpdateInput = (),
                    Filter = NoFilter,
                >,
        >,
    ) {
    }

    // Test that auto_impl works with &T
    fn _takes_ref_repo<R: Repository>(_: &R) {}

    #[test]
    fn arc_wrapped() {
        let repo = Arc::new(InMemoryRepo::new());
        repo.create("hello".to_string()).unwrap();
        assert_eq!(repo.count(NoFilter).unwrap(), 1);
    }

    #[test]
    fn polymorphic_count() {
        fn count_all<R: Repository>(repo: &R) -> Result<u64>
        where
            R::Filter: Default,
        {
            repo.count(R::Filter::default())
        }

        let repo = InMemoryRepo::new();
        repo.create("one".to_string()).unwrap();
        assert_eq!(count_all(&repo).unwrap(), 1);
    }
}
