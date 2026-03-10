#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "runtime")]
pub use runtime::*;

#[cfg(not(feature = "runtime"))]
#[allow(dead_code)]
const BUILD_STUB: () = ();
