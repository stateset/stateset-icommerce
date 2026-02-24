//! Async counterparts to the generic [`super::Repository`] and [`super::Transactional`] traits.
//!
//! These traits mirror [`super::Repository`] and [`super::Transactional`] but use
//! `async fn` (via [`async_trait`]) so they can be implemented by async database
//! backends (e.g. `sqlx`, `sea-orm`).
//!
//! # Example
//!
//! ```rust
//! use stateset_core::traits::AsyncRepository;
//!
//! async fn count_all<R: AsyncRepository>(repo: &R) -> stateset_core::Result<u64>
//! where
//!     R::Filter: Default,
//! {
//!     repo.count(R::Filter::default()).await
//! }
//! ```

use crate::errors::{BatchResult, Result};
use std::fmt::Debug;

/// Async generic CRUD repository interface.
///
/// This is the async counterpart to [`super::Repository`]. It defines the same
/// CRUD operations but as async methods, suitable for use with async database
/// drivers like `sqlx` or `sea-orm`.
///
/// # Blanket Implementations
///
/// The [`auto_impl`] attribute provides automatic implementations for
/// `&T`, `Box<T>`, and `Arc<T>`.
#[async_trait::async_trait]
#[auto_impl::auto_impl(&, &mut, Box, Arc)]
pub trait AsyncRepository: Send + Sync + Debug {
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
    async fn create(&self, input: Self::CreateInput) -> Result<Self::Entity>;

    /// Get an entity by its ID. Returns `None` if not found.
    async fn get(&self, id: Self::Id) -> Result<Option<Self::Entity>>;

    /// Update an existing entity.
    async fn update(&self, id: Self::Id, input: Self::UpdateInput) -> Result<Self::Entity>;

    /// Delete an entity by ID.
    async fn delete(&self, id: Self::Id) -> Result<()>;

    /// List entities matching the given filter.
    async fn list(&self, filter: Self::Filter) -> Result<Vec<Self::Entity>>;

    /// Count entities matching the given filter.
    async fn count(&self, filter: Self::Filter) -> Result<u64>;

    /// Get multiple entities by their IDs.
    ///
    /// The default implementation calls `get` in a loop. Backends should override
    /// this with a batch query for better performance.
    async fn get_batch(&self, ids: Vec<Self::Id>) -> Result<Vec<Self::Entity>> {
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(entity) = self.get(id).await? {
                results.push(entity);
            }
        }
        Ok(results)
    }

    /// Create multiple entities with partial-success semantics.
    ///
    /// The default implementation calls `create` in a loop. Backends should override
    /// with a batch insert for better performance.
    async fn create_batch(
        &self,
        inputs: Vec<Self::CreateInput>,
    ) -> Result<BatchResult<Self::Entity>> {
        let mut result = BatchResult::with_capacity(inputs.len());
        for (i, input) in inputs.into_iter().enumerate() {
            match self.create(input).await {
                Ok(entity) => result.record_success(entity),
                Err(err) => result.record_failure(i, None, &err),
            }
        }
        Ok(result)
    }
}

/// Async extension trait for repositories that support transactions.
///
/// This is the async counterpart to [`super::Transactional`].
#[async_trait::async_trait]
#[auto_impl::auto_impl(&, &mut, Box, Arc)]
pub trait AsyncTransactional: Send + Sync {
    /// Begin a transaction.
    async fn begin_transaction(&self) -> Result<()>;

    /// Commit the current transaction.
    async fn commit(&self) -> Result<()>;

    /// Rollback the current transaction.
    async fn rollback(&self) -> Result<()>;

    /// Execute a closure within a transaction, automatically rolling back on
    /// error and committing on success.
    async fn with_transaction<T, F, Fut>(&self, f: F) -> Result<T>
    where
        T: Send,
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        self.begin_transaction().await?;
        match f().await {
            Ok(val) => {
                self.commit().await?;
                Ok(val)
            }
            Err(err) => {
                let _ = self.rollback().await;
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

    #[derive(Debug)]
    struct AsyncInMemoryRepo {
        items: tokio::sync::Mutex<Vec<(u64, String)>>,
        next_id: tokio::sync::Mutex<u64>,
    }

    impl AsyncInMemoryRepo {
        fn new() -> Self {
            Self { items: tokio::sync::Mutex::new(Vec::new()), next_id: tokio::sync::Mutex::new(1) }
        }
    }

    #[derive(Debug, Default)]
    struct NoFilter;

    #[async_trait::async_trait]
    impl AsyncRepository for AsyncInMemoryRepo {
        type Entity = (u64, String);
        type Id = u64;
        type CreateInput = String;
        type UpdateInput = String;
        type Filter = NoFilter;

        async fn create(&self, input: String) -> Result<(u64, String)> {
            let mut id = self.next_id.lock().await;
            let current = *id;
            *id += 1;
            let entity = (current, input);
            self.items.lock().await.push(entity.clone());
            Ok(entity)
        }

        async fn get(&self, id: u64) -> Result<Option<(u64, String)>> {
            Ok(self.items.lock().await.iter().find(|(i, _)| *i == id).cloned())
        }

        async fn update(&self, id: u64, input: String) -> Result<(u64, String)> {
            let mut items = self.items.lock().await;
            if let Some(item) = items.iter_mut().find(|(i, _)| *i == id) {
                item.1 = input;
                Ok(item.clone())
            } else {
                Err(CommerceError::NotFound)
            }
        }

        async fn delete(&self, id: u64) -> Result<()> {
            let mut items = self.items.lock().await;
            items.retain(|(i, _)| *i != id);
            Ok(())
        }

        async fn list(&self, _filter: NoFilter) -> Result<Vec<(u64, String)>> {
            Ok(self.items.lock().await.clone())
        }

        async fn count(&self, _filter: NoFilter) -> Result<u64> {
            Ok(self.items.lock().await.len() as u64)
        }
    }

    #[tokio::test]
    async fn async_crud_operations() {
        let repo = AsyncInMemoryRepo::new();
        let entity = repo.create("hello".to_string()).await.unwrap();
        assert_eq!(entity.0, 1);
        assert_eq!(entity.1, "hello");

        let found = repo.get(1).await.unwrap();
        assert_eq!(found, Some((1, "hello".to_string())));

        let updated = repo.update(1, "world".to_string()).await.unwrap();
        assert_eq!(updated.1, "world");

        assert_eq!(repo.count(NoFilter).await.unwrap(), 1);

        repo.delete(1).await.unwrap();
        assert_eq!(repo.count(NoFilter).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn async_get_batch_default() {
        let repo = AsyncInMemoryRepo::new();
        repo.create("a".to_string()).await.unwrap();
        repo.create("b".to_string()).await.unwrap();
        repo.create("c".to_string()).await.unwrap();

        let batch = repo.get_batch(vec![1, 3]).await.unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].1, "a");
        assert_eq!(batch[1].1, "c");
    }

    #[tokio::test]
    async fn async_create_batch_default() {
        let repo = AsyncInMemoryRepo::new();
        let result = repo
            .create_batch(vec!["x".to_string(), "y".to_string(), "z".to_string()])
            .await
            .unwrap();
        assert_eq!(result.success_count, 3);
        assert_eq!(result.failure_count, 0);
        assert!(result.all_succeeded());
    }

    #[tokio::test]
    async fn async_arc_wrapped() {
        let repo = Arc::new(AsyncInMemoryRepo::new());
        repo.create("hello".to_string()).await.unwrap();
        assert_eq!(repo.count(NoFilter).await.unwrap(), 1);
    }

    async fn async_count_all<R: AsyncRepository>(repo: &R) -> Result<u64>
    where
        R::Filter: Default,
    {
        repo.count(R::Filter::default()).await
    }

    #[tokio::test]
    async fn async_polymorphic_count() {
        let repo = AsyncInMemoryRepo::new();
        repo.create("one".to_string()).await.unwrap();
        assert_eq!(async_count_all(&repo).await.unwrap(), 1);
    }
}
