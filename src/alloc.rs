//! A small-block allocator.
//!
//! A scripting language makes a great many small, short-lived blocks (texts, list
//! buffers, struct fields). The system allocator needs roughly 80 ns for each one on
//! Windows; this allocator hands out blocks of up to 512 bytes from per-size free lists
//! in a few nanoseconds and passes everything larger straight to the system.
//!
//! Small blocks are reused but never returned to the system, so the memory of a run
//! stays at its high-water mark for small objects. The VM's own memory limit is
//! counted separately (`heap::Heap::limit`) and is not affected.
//!
//! Together with `ffi.rs` this file holds all the `unsafe` code of the project.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Block sizes go up in steps of this many bytes; it is also the strictest alignment served.
const STEP: usize = 16;
/// The largest block served from the pool.
const MAX_SMALL: usize = 512;
const CLASSES: usize = MAX_SMALL / STEP;
/// Fresh memory is taken from the system in pieces of this size.
const CHUNK: usize = 1 << 18;

struct State {
    /// One singly linked list of free blocks per size class; the link is stored in the block.
    free: [*mut u8; CLASSES],
    /// The unused tail of the current chunk.
    next: *mut u8,
    left: usize,
}

pub struct Pool {
    locked: AtomicBool,
    state: UnsafeCell<State>,
}

// SAFETY: `state` is only touched between `lock` and `unlock`.
unsafe impl Sync for Pool {}

/// The size class of a layout, or `None` when the system allocator serves it.
#[inline]
fn class_of(size: usize, align: usize) -> Option<usize> {
    if size <= MAX_SMALL && align <= STEP {
        Some((size.max(1) + STEP - 1) / STEP - 1)
    } else {
        None
    }
}

impl Pool {
    pub const fn new() -> Pool {
        Pool {
            locked: AtomicBool::new(false),
            state: UnsafeCell::new(State { free: [std::ptr::null_mut(); CLASSES], next: std::ptr::null_mut(), left: 0 }),
        }
    }

    #[inline]
    fn lock(&self) {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
    }

    #[inline]
    fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl Default for Pool {
    fn default() -> Pool {
        Pool::new()
    }
}

unsafe impl GlobalAlloc for Pool {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class_of(layout.size(), layout.align()) else {
            return System.alloc(layout);
        };
        self.lock();
        // SAFETY: the lock is held, so this is the only reference to the state.
        let state = &mut *self.state.get();
        let head = state.free[class];
        let block = if !head.is_null() {
            // SAFETY: a free block is at least STEP bytes, STEP-aligned, and holds the next link.
            state.free[class] = *(head as *mut *mut u8);
            head
        } else {
            let size = (class + 1) * STEP;
            if state.left < size {
                // SAFETY: CHUNK is not zero and STEP is a power of two.
                let chunk = System.alloc(Layout::from_size_align_unchecked(CHUNK, STEP));
                if chunk.is_null() {
                    self.unlock();
                    return chunk;
                }
                state.next = chunk;
                state.left = CHUNK;
            }
            let block = state.next;
            // SAFETY: `size <= left`, so the new tail stays inside the chunk.
            state.next = block.add(size);
            state.left -= size;
            block
        };
        self.unlock();
        block
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(class) = class_of(layout.size(), layout.align()) else {
            return System.dealloc(ptr, layout);
        };
        self.lock();
        // SAFETY: the lock is held; `ptr` came from `alloc` with the same layout, so it is a
        // block of this class and large enough to hold the link.
        let state = &mut *self.state.get();
        *(ptr as *mut *mut u8) = state.free[class];
        state.free[class] = ptr;
        self.unlock();
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = class_of(layout.size(), layout.align());
        let new = class_of(new_size, layout.align());
        match (old, new) {
            (None, None) => System.realloc(ptr, layout, new_size),
            (Some(a), Some(b)) if a == b => ptr,
            _ => {
                // SAFETY: the caller guarantees that `new_size` is valid for this alignment.
                let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
                let fresh = self.alloc(new_layout);
                if !fresh.is_null() {
                    std::ptr::copy_nonoverlapping(ptr, fresh, layout.size().min(new_size));
                    self.dealloc(ptr, layout);
                }
                fresh
            }
        }
    }
}
