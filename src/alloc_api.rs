//! The allocator API this crate's [`Allocator`] impls are written against.
//!
//! Re-exported so that dependents do not have to work out which of the two
//! sources is in play, and do not need their own `allocator-api2` dependency.

#[cfg(not(feature = "nightly"))]
pub use allocator_api2::alloc::{AllocError, Allocator};

#[cfg(feature = "nightly")]
pub use std::alloc::{AllocError, Allocator};

pub use std::alloc::Layout;
