use std::cell::Cell;
use std::ptr::NonNull;

use thiserror::Error;

use crate::alloc_api::{AllocError, Allocator, Layout};
use crate::numa_alloc::{NumaAllocError, alloc_on_node, free_numa};

/// A bump allocator over a single node-pinned mapping.
///
/// Memory is never returned by [`deallocate`](Allocator::deallocate); only
/// [`reset`](Self::reset) recycles it, and only once every allocation has been
/// dropped.
pub struct NumaBumpAllocator {
    base: NonNull<u8>,
    capacity: usize,
    cursor: Cell<usize>,
    refcount: Cell<usize>,
}

#[derive(Error, Debug)]
pub enum NumaBumpAllocError {
    #[error("bump allocator refcount was non-empty ({remaining_refs} refs)")]
    ResetError { remaining_refs: usize },
}

impl NumaBumpAllocator {
    /// Reserves `capacity` bytes on NUMA `node`.
    pub fn try_new(node: u32, capacity: usize) -> Result<Self, NumaAllocError> {
        // A zero-length mapping is not a thing, and an arena that can never
        // hand out anything is not worth a distinct error.
        let capacity = capacity.max(1);
        let base = alloc_on_node(capacity, node)?;
        Ok(Self {
            base,
            capacity,
            cursor: Cell::new(0),
            refcount: Cell::new(0),
        })
    }

    /// Like [`try_new`](Self::try_new), but panics if the arena cannot be
    /// allocated.
    pub fn new(node: u32, capacity: usize) -> Self {
        Self::try_new(node, capacity)
            .unwrap_or_else(|e| panic!("NUMA allocation for bump arena failed: {e}"))
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn reset(&self) -> Result<(), NumaBumpAllocError> {
        let refs = self.refcount.get();
        if refs == 0 {
            self.cursor.set(0);
            Ok(())
        } else {
            Err(NumaBumpAllocError::ResetError {
                remaining_refs: refs,
            })
        }
    }
}

unsafe impl Allocator for NumaBumpAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let cursor = self.cursor.get();
        let aligned = (cursor + layout.align() - 1) & !(layout.align() - 1);
        let end = aligned
            .checked_add(layout.size())
            .filter(|&end| end <= self.capacity)
            .ok_or(AllocError)?;
        self.cursor.set(end);
        // SAFETY: [aligned, end) lies within the capacity-byte mapping.
        let ptr = unsafe { self.base.as_ptr().add(aligned) };
        self.refcount.update(|x| x.checked_add(1).unwrap());
        Ok(NonNull::new(std::ptr::slice_from_raw_parts_mut(ptr, layout.size())).unwrap())
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.refcount.update(|x| x.checked_sub(1).unwrap());
        let offset = ptr.as_ptr() as usize - self.base.as_ptr() as usize;
        if offset + layout.size() == self.cursor.get() {
            // this is the last allocated block, we can properly reuse the space
            self.cursor.set(offset);
        }
    }
}

impl Drop for NumaBumpAllocator {
    fn drop(&mut self) {
        unsafe { free_numa(self.base.as_ptr(), self.capacity) };
    }
}
