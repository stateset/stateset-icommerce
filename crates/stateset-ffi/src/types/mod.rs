//! ABI-safe (`#[repr(C)]`) versions of core domain types.

pub mod customer;
pub mod ids;
pub mod inventory;
pub mod money;
pub mod order;
pub mod product;

pub use customer::FfiCustomer;
pub use ids::FfiUuid;
pub use inventory::FfiInventoryLevel;
pub use money::FfiMoney;
pub use order::{FfiOrder, FfiOrderStatus};
pub use product::FfiProduct;
