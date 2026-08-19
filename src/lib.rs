//! NUMA-node-pinned allocation, in the spirit of `numa.h`'s `numa_alloc_onnode`,
//! plus a bump allocator built on top of it.
//!
//! Node pinning is a Linux facility (`mmap` + `mbind`). The crate compiles
//! everywhere, but on other platforms every allocation fails with
//! [`NumaAllocError::UnsupportedPlatform`] rather than falling back to an unpinned
//! allocation, so a test that silently stopped measuring what it meant to
//! measure is impossible.
//!
//! # Toolchain
//!
//! By default the [`Allocator`](alloc_api::Allocator) impls target the
//! [`allocator-api2`] mirror of the unstable `allocator_api`, which works on
//! stable Rust. Enable the `nightly` feature to target `std`'s own
//! `allocator_api` instead, which is what you want if you pass these allocators
//! to `std`'s `Box`/`Vec`.
//!
//! # Example
//! ```
//! use numa_alloc_rs::alloc_on_node;
//!
//! let mut ptr = alloc_on_node(512, 0).unwrap();
//! unsafe { ptr.write_bytes(100, 512) };
//!
//! assert_eq!(unsafe { ptr.add(400).read() }, 100);
//! ```
//!
//! ```
//! #![feature(allocator_api)]
//! # #[cfg(feature="nightly")]
//! # {
//! use numa_alloc_rs::NumaBumpAllocator;
//!
//! let alloc = NumaBumpAllocator::new(0, 8196);
//! let buffer = Vec::<u64, _>::with_capacity_in(500, &alloc);
//!
//! // not all allocations have been freed
//! assert!(alloc.reset().is_err())
//! # }
//! ```
//!
//! [`allocator-api2`]: https://docs.rs/allocator-api2

#![cfg_attr(feature = "nightly", feature(allocator_api))]

pub mod alloc_api;
pub mod bump_alloc;
pub mod numa_alloc;

pub use bump_alloc::{NumaBumpAllocError, NumaBumpAllocator};
pub use numa_alloc::{NumaAllocError, NumaAllocator, alloc_on_node, free_numa};
