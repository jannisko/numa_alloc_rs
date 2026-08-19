use std::ptr::NonNull;

use thiserror::Error;

use crate::alloc_api::{AllocError, Allocator, Layout};

/// The nodemask handed to `mbind` is a single `u64`, so this is the highest node
/// this crate can address.
pub const MAX_NODE: u32 = u64::BITS - 1;

#[derive(Error, Debug)]
pub enum NumaAllocError {
    #[error("NUMA node pinning is only supported on Linux")]
    UnsupportedPlatform,
    #[error("node {node} is out of range (must be <= {MAX_NODE})")]
    NodeOutOfRange { node: u32 },
    #[error("mmap of {size} bytes failed: {source}")]
    Map {
        size: usize,
        #[source]
        source: std::io::Error,
    },
    #[error("mbind of {size} bytes to node {node} failed: {source}")]
    Bind {
        node: u32,
        size: usize,
        #[source]
        source: std::io::Error,
    },
}

/// Maps `size` bytes and binds them to NUMA `node`, so that the pages are taken
/// from that node's memory when first touched.
///
/// The returned pointer is page-aligned and must be released with [`free_numa`]
/// and the same `size`.
#[cfg(target_os = "linux")]
pub fn alloc_on_node(size: usize, node: u32) -> Result<NonNull<u8>, NumaAllocError> {
    use libc::{MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, MPOL_BIND, PROT_READ, PROT_WRITE, mmap};

    let nodemask: u64 = 1u64
        .checked_shl(node)
        .ok_or(NumaAllocError::NodeOutOfRange { node })?;

    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_ANONYMOUS | MAP_PRIVATE,
            -1,
            0,
        )
    };

    if ptr == MAP_FAILED {
        return Err(NumaAllocError::Map {
            size,
            source: std::io::Error::last_os_error(),
        });
    }

    // Bind before the pages are touched: MPOL_BIND governs where a page is
    // taken from when it is first faulted in, and does nothing for pages that
    // are already resident.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_mbind,
            ptr,
            size,
            MPOL_BIND,
            &nodemask as *const u64,
            // maxnode counts the bits of the nodemask, not the nodes in use.
            u64::BITS as libc::c_ulong,
            0,
        )
    };

    if rc != 0 {
        let source = std::io::Error::last_os_error();
        unsafe { libc::munmap(ptr, size) };
        return Err(NumaAllocError::Bind { node, size, source });
    }

    // SAFETY: a successful mmap never returns NULL.
    Ok(unsafe { NonNull::new_unchecked(ptr as *mut u8) })
}

#[cfg(not(target_os = "linux"))]
pub fn alloc_on_node(_size: usize, _node: u32) -> Result<NonNull<u8>, NumaAllocError> {
    Err(NumaAllocError::UnsupportedPlatform)
}

/// Releases a mapping obtained from [`alloc_on_node`].
///
/// # Safety
///
/// `ptr` must come from [`alloc_on_node`], `size` must be the size it was
/// allocated with, and neither the mapping nor any pointer into it may be used
/// afterwards.
#[cfg(target_os = "linux")]
pub unsafe fn free_numa(ptr: *mut u8, size: usize) {
    unsafe { libc::munmap(ptr as *mut libc::c_void, size) };
}

#[cfg(not(target_os = "linux"))]
pub unsafe fn free_numa(_ptr: *mut u8, _size: usize) {}

/// An [`Allocator`] that serves every allocation from one NUMA node.
///
/// Each allocation is a separate mapping, which makes this a coarse tool: use
/// it for a few large buffers, or wrap it in [`NumaBumpAllocator`] for many
/// small ones.
///
/// [`NumaBumpAllocator`]: crate::NumaBumpAllocator
#[derive(Debug, Clone, Copy)]
pub struct NumaAllocator {
    pub node: u32,
}

unsafe impl Allocator for NumaAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            let dangling = NonNull::new(layout.align() as *mut u8).ok_or(AllocError)?;
            return Ok(NonNull::slice_from_raw_parts(dangling, 0));
        }

        // TODO: alignments larger than page size are not respected
        let ptr = alloc_on_node(layout.size(), self.node).map_err(|_| AllocError)?;
        Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        unsafe { free_numa(ptr.as_ptr(), layout.size()) };
    }
}
