//! Migration registry for managing ordered migration definitions.

use std::collections::BTreeMap;

use crate::error::{MigrationError, Result};
use crate::migration::{Migration, MigrationRecord};

/// A registry of migration definitions, ordered by version.
///
/// Migrations must be registered with unique version numbers.
/// The registry provides methods to list, query pending, and validate migrations.
///
/// # Examples
///
/// ```
/// use stateset_migrations::{Migration, MigrationRegistry};
///
/// let registry = MigrationRegistry::builder()
///     .add(Migration::new(1, "create_users", "CREATE TABLE users (id TEXT);"))
///     .add(Migration::new(2, "add_email", "ALTER TABLE users ADD COLUMN email TEXT;"))
///     .build()
///     .unwrap();
///
/// assert_eq!(registry.list().len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct MigrationRegistry {
    migrations: BTreeMap<u32, Migration>,
}

impl MigrationRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self { migrations: BTreeMap::new() }
    }

    /// Create a builder for fluent registry construction.
    #[must_use]
    pub const fn builder() -> MigrationRegistryBuilder {
        MigrationRegistryBuilder::new()
    }

    /// Register a migration. Returns an error if a migration with the same
    /// version is already registered.
    pub fn register(&mut self, migration: Migration) -> Result<()> {
        if let Some(existing) = self.migrations.get(&migration.version) {
            return Err(MigrationError::VersionConflict {
                version: migration.version,
                existing_name: existing.name.clone(),
                new_name: migration.name.clone(),
            });
        }
        self.migrations.insert(migration.version, migration);
        Ok(())
    }

    /// Return all migrations sorted by version.
    #[must_use]
    pub fn list(&self) -> Vec<&Migration> {
        self.migrations.values().collect()
    }

    /// Return the number of registered migrations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.migrations.len()
    }

    /// Return `true` if no migrations are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.migrations.is_empty()
    }

    /// Get a migration by version.
    #[must_use]
    pub fn get(&self, version: u32) -> Option<&Migration> {
        self.migrations.get(&version)
    }

    /// Return the latest (highest) version, or `None` if the registry is empty.
    #[must_use]
    pub fn latest_version(&self) -> Option<u32> {
        self.migrations.keys().next_back().copied()
    }

    /// Return migrations that have not yet been applied, based on the given
    /// list of applied records.
    #[must_use]
    pub fn pending<'a>(&'a self, applied: &[MigrationRecord]) -> Vec<&'a Migration> {
        let applied_versions: std::collections::HashSet<u32> =
            applied.iter().map(|r| r.version).collect();
        self.migrations
            .values()
            .filter(|m| !applied_versions.contains(&m.version))
            .collect()
    }

    /// Validate that all applied migration checksums match the registered
    /// migrations. Returns `Ok(())` if all checksums match.
    pub fn validate_checksums(&self, applied: &[MigrationRecord]) -> Result<()> {
        for record in applied {
            if let Some(migration) = self.migrations.get(&record.version) {
                if record.checksum != migration.checksum {
                    return Err(MigrationError::ChecksumMismatch {
                        version: record.version,
                        name: migration.name.clone(),
                        expected: migration.checksum.clone(),
                        actual: record.checksum.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Return migrations in reverse order from `from_version` down to
    /// (but not including) `to_version`. Used for rollback planning.
    #[must_use]
    pub fn range_reverse(&self, to_version: u32, from_version: u32) -> Vec<&Migration> {
        self.migrations
            .range((
                std::ops::Bound::Excluded(to_version),
                std::ops::Bound::Included(from_version),
            ))
            .rev()
            .map(|(_, m)| m)
            .collect()
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing a [`MigrationRegistry`] fluently.
#[derive(Debug)]
pub struct MigrationRegistryBuilder {
    migrations: Vec<Migration>,
}

impl MigrationRegistryBuilder {
    /// Create a new empty builder.
    #[must_use]
    pub const fn new() -> Self {
        Self { migrations: Vec::new() }
    }

    /// Add a migration to the builder.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, migration: Migration) -> Self {
        self.migrations.push(migration);
        self
    }

    /// Build the registry, validating that all versions are unique.
    pub fn build(self) -> Result<MigrationRegistry> {
        let mut registry = MigrationRegistry::new();
        for migration in self.migrations {
            registry.register(migration)?;
        }
        Ok(registry)
    }
}

impl Default for MigrationRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_migration(version: u32, name: &str) -> Migration {
        Migration::new(version, name, format!("-- migration {version}"))
    }

    fn make_record(version: u32, name: &str, checksum: &str) -> MigrationRecord {
        MigrationRecord {
            version,
            name: name.to_string(),
            applied_at: Utc::now(),
            checksum: checksum.to_string(),
            execution_time_ms: 10,
        }
    }

    #[test]
    fn register_and_list() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(1, "first")).unwrap();
        reg.register(make_migration(2, "second")).unwrap();
        let list = reg.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].version, 1);
        assert_eq!(list[1].version, 2);
    }

    #[test]
    fn register_duplicate_version_fails() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(1, "first")).unwrap();
        let err = reg.register(make_migration(1, "duplicate")).unwrap_err();
        assert!(matches!(err, MigrationError::VersionConflict { version: 1, .. }));
    }

    #[test]
    fn list_is_sorted_by_version() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(3, "third")).unwrap();
        reg.register(make_migration(1, "first")).unwrap();
        reg.register(make_migration(2, "second")).unwrap();
        let versions: Vec<u32> = reg.list().iter().map(|m| m.version).collect();
        assert_eq!(versions, vec![1, 2, 3]);
    }

    #[test]
    fn pending_returns_unapplied() {
        let mut reg = MigrationRegistry::new();
        let m1 = make_migration(1, "first");
        let m2 = make_migration(2, "second");
        let m3 = make_migration(3, "third");
        reg.register(m1.clone()).unwrap();
        reg.register(m2).unwrap();
        reg.register(m3).unwrap();

        let applied = vec![make_record(1, "first", &m1.checksum)];
        let pending = reg.pending(&applied);
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].version, 2);
        assert_eq!(pending[1].version, 3);
    }

    #[test]
    fn pending_empty_when_all_applied() {
        let mut reg = MigrationRegistry::new();
        let m1 = make_migration(1, "first");
        let m2 = make_migration(2, "second");
        reg.register(m1.clone()).unwrap();
        reg.register(m2.clone()).unwrap();

        let applied = vec![
            make_record(1, "first", &m1.checksum),
            make_record(2, "second", &m2.checksum),
        ];
        assert!(reg.pending(&applied).is_empty());
    }

    #[test]
    fn pending_all_when_none_applied() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(1, "first")).unwrap();
        reg.register(make_migration(2, "second")).unwrap();
        assert_eq!(reg.pending(&[]).len(), 2);
    }

    #[test]
    fn validate_checksums_ok() {
        let mut reg = MigrationRegistry::new();
        let m1 = make_migration(1, "first");
        reg.register(m1.clone()).unwrap();

        let applied = vec![make_record(1, "first", &m1.checksum)];
        assert!(reg.validate_checksums(&applied).is_ok());
    }

    #[test]
    fn validate_checksums_mismatch() {
        let mut reg = MigrationRegistry::new();
        let m1 = make_migration(1, "first");
        reg.register(m1).unwrap();

        let applied = vec![make_record(1, "first", "wrong_checksum")];
        let err = reg.validate_checksums(&applied).unwrap_err();
        assert!(matches!(err, MigrationError::ChecksumMismatch { version: 1, .. }));
    }

    #[test]
    fn validate_checksums_ignores_unknown_applied() {
        let reg = MigrationRegistry::new();
        let applied = vec![make_record(99, "unknown", "whatever")];
        // Unknown applied migrations are not in the registry, so no mismatch
        assert!(reg.validate_checksums(&applied).is_ok());
    }

    #[test]
    fn builder_builds_correctly() {
        let registry = MigrationRegistry::builder()
            .add(make_migration(1, "first"))
            .add(make_migration(2, "second"))
            .build()
            .unwrap();
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn builder_detects_duplicate() {
        let err = MigrationRegistry::builder()
            .add(make_migration(1, "first"))
            .add(make_migration(1, "duplicate"))
            .build()
            .unwrap_err();
        assert!(matches!(err, MigrationError::VersionConflict { .. }));
    }

    #[test]
    fn len_and_is_empty() {
        let reg = MigrationRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn get_by_version() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(5, "fifth")).unwrap();
        assert!(reg.get(5).is_some());
        assert!(reg.get(1).is_none());
    }

    #[test]
    fn latest_version_empty() {
        let reg = MigrationRegistry::new();
        assert!(reg.latest_version().is_none());
    }

    #[test]
    fn latest_version_returns_highest() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(2, "b")).unwrap();
        reg.register(make_migration(5, "e")).unwrap();
        reg.register(make_migration(3, "c")).unwrap();
        assert_eq!(reg.latest_version(), Some(5));
    }

    #[test]
    fn range_reverse_for_rollback() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(1, "a")).unwrap();
        reg.register(make_migration(2, "b")).unwrap();
        reg.register(make_migration(3, "c")).unwrap();
        reg.register(make_migration(4, "d")).unwrap();

        // Rollback from v4 to v2 means reversing v4, v3
        let rollback = reg.range_reverse(2, 4);
        assert_eq!(rollback.len(), 2);
        assert_eq!(rollback[0].version, 4);
        assert_eq!(rollback[1].version, 3);
    }

    #[test]
    fn range_reverse_empty_range() {
        let mut reg = MigrationRegistry::new();
        reg.register(make_migration(1, "a")).unwrap();
        let rollback = reg.range_reverse(1, 1);
        assert!(rollback.is_empty());
    }

    #[test]
    fn default_registry_is_empty() {
        let reg = MigrationRegistry::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn default_builder_builds_empty() {
        let reg = MigrationRegistryBuilder::default().build().unwrap();
        assert!(reg.is_empty());
    }
}
