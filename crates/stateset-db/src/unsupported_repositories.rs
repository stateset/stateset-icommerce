//! Placeholder repositories for backends that do not implement a domain.
//!
//! Previously this module held `Unsupported{GiftCard,StoreCredit,Review,Wishlist}Repository`
//! shims that returned [`stateset_core::CommerceError::NotPermitted`] for the Postgres
//! backend. Those domains now have real Postgres implementations
//! (`postgres::Pg{GiftCard,StoreCredit,Review,Wishlist}Repository`), wired into the
//! Postgres `NewDomainRepositoryFactory`, so the shims are no longer referenced and have
//! been removed. The module is intentionally left empty to preserve history; it is not
//! declared in `lib.rs`.
