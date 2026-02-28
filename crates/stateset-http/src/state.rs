//! Application state shared across all route handlers.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use axum::http::HeaderMap;
use stateset_embedded::Commerce;

use crate::{error::HttpError, middleware::X_TENANT_ID};

/// Shared application state injected into every route handler via
/// [`axum::extract::State`].
#[derive(Clone)]
pub struct AppState {
    commerce: Arc<Commerce>,
    tenant_db_dir: Option<Arc<PathBuf>>,
    tenant_cache: Arc<RwLock<HashMap<String, Arc<Commerce>>>>,
}

impl fmt::Debug for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
            .field("commerce", &"Commerce { .. }")
            .field(
                "tenant_db_dir",
                &self.tenant_db_dir.as_deref().map(|path| path.display().to_string()),
            )
            .finish()
    }
}

fn is_valid_tenant_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }
    trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

/// Parse `x-tenant-id` from request headers.
#[must_use]
pub(crate) fn tenant_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(&X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

impl AppState {
    /// Create a new `AppState` wrapping a [`Commerce`] instance.
    #[must_use]
    pub fn new(commerce: Commerce) -> Self {
        Self::new_with_tenant_db_dir(commerce, None)
    }

    /// Create a new `AppState` with optional per-tenant database directory.
    #[must_use]
    pub fn new_with_tenant_db_dir(commerce: Commerce, tenant_db_dir: Option<PathBuf>) -> Self {
        Self {
            commerce: Arc::new(commerce),
            tenant_db_dir: tenant_db_dir.map(Arc::new),
            tenant_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enable per-tenant SQLite routing under a base directory.
    #[must_use]
    pub fn with_tenant_db_dir(mut self, tenant_db_dir: impl Into<PathBuf>) -> Self {
        self.tenant_db_dir = Some(Arc::new(tenant_db_dir.into()));
        self
    }

    /// Returns the configured tenant DB directory when per-tenant routing is enabled.
    #[must_use]
    pub fn tenant_db_dir(&self) -> Option<&Path> {
        self.tenant_db_dir.as_deref().map(|path| path.as_path())
    }

    /// Access the underlying default [`Commerce`] engine.
    #[must_use]
    pub fn commerce(&self) -> &Commerce {
        &self.commerce
    }

    /// Resolve the [`Commerce`] engine for a tenant.
    ///
    /// When per-tenant routing is disabled, this always returns the default engine.
    pub fn commerce_for_tenant(&self, tenant_id: Option<&str>) -> Result<Arc<Commerce>, HttpError> {
        let Some(base_dir) = self.tenant_db_dir.as_deref() else {
            return Ok(Arc::clone(&self.commerce));
        };

        let tenant_id = tenant_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| HttpError::BadRequest("missing x-tenant-id header".to_string()))?;

        if !is_valid_tenant_id(tenant_id) {
            return Err(HttpError::BadRequest("invalid x-tenant-id header".to_string()));
        }

        let cache = self.tenant_cache.read().map_err(|_| {
            HttpError::InternalError("tenant cache lock poisoned while reading".to_string())
        })?;
        if let Some(existing) = cache.get(tenant_id) {
            return Ok(Arc::clone(existing));
        }
        drop(cache);

        std::fs::create_dir_all(base_dir).map_err(|error| {
            HttpError::InternalError(format!(
                "failed to create tenant database directory {}: {error}",
                base_dir.display()
            ))
        })?;

        let db_path = base_dir.join(format!("{tenant_id}.db"));
        let db_path_str = db_path.to_string_lossy().into_owned();
        let created = Arc::new(Commerce::new(&db_path_str).map_err(|error| {
            HttpError::InternalError(format!(
                "failed to initialize tenant database for '{tenant_id}': {error}"
            ))
        })?);

        let mut cache = self.tenant_cache.write().map_err(|_| {
            HttpError::InternalError("tenant cache lock poisoned while writing".to_string())
        })?;
        let entry = cache.entry(tenant_id.to_string()).or_insert_with(|| Arc::clone(&created));
        Ok(Arc::clone(entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::CreateCustomer;
    use uuid::Uuid;

    fn test_commerce() -> Commerce {
        Commerce::new(":memory:").expect("in-memory Commerce")
    }

    #[test]
    fn state_debug_impl() {
        let state = AppState::new(test_commerce());
        let dbg = format!("{state:?}");
        assert!(dbg.contains("AppState"));
    }

    #[test]
    fn state_clone() {
        let state = AppState::new(test_commerce());
        let cloned = state.clone();
        // Both point to the same Arc
        assert!(Arc::ptr_eq(&state.commerce, &cloned.commerce));
    }

    #[test]
    fn state_accessor() {
        let state = AppState::new(test_commerce());
        // Just verify it doesn't panic
        let _commerce = state.commerce();
    }

    #[test]
    fn tenant_header_parser_reads_valid_value() {
        let mut headers = HeaderMap::new();
        headers.insert(X_TENANT_ID.clone(), "tenant-1".parse().unwrap());
        assert_eq!(tenant_id_from_headers(&headers).as_deref(), Some("tenant-1"));
    }

    #[test]
    fn tenant_header_parser_rejects_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert(X_TENANT_ID.clone(), " ".parse().unwrap());
        assert!(tenant_id_from_headers(&headers).is_none());
    }

    #[test]
    fn tenant_routing_isolated_when_configured() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()));

        let tenant_a = state.commerce_for_tenant(Some("tenant-a")).expect("tenant-a commerce");
        let tenant_b = state.commerce_for_tenant(Some("tenant-b")).expect("tenant-b commerce");

        tenant_a
            .customers()
            .create(CreateCustomer {
                email: "tenant-a@example.com".into(),
                first_name: "Tenant".into(),
                last_name: "A".into(),
                ..Default::default()
            })
            .unwrap();

        let a_count = tenant_a.customers().list(Default::default()).unwrap().len();
        let b_count = tenant_b.customers().list(Default::default()).unwrap().len();
        assert_eq!(a_count, 1);
        assert_eq!(b_count, 0);

        let _ = std::fs::remove_dir_all(tenant_dir);
    }

    #[test]
    fn tenant_routing_requires_header_when_enabled() {
        let tenant_dir =
            std::env::temp_dir().join(format!("stateset-http-state-{}", Uuid::new_v4()));
        let state = AppState::new_with_tenant_db_dir(test_commerce(), Some(tenant_dir.clone()));

        let err = state.commerce_for_tenant(None).expect_err("expected missing tenant error");
        assert!(matches!(err, HttpError::BadRequest(_)));

        let _ = std::fs::remove_dir_all(tenant_dir);
    }
}
