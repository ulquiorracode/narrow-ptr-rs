//! # narrow-ptr
//!
//! Position-independent relative and narrow pointer types for zero-copy memory mapping (`mmap`),
//! IPC, and cache-friendly data structures.
//!
//! Conforms to the "Scrooge" Systems Mindset:
//! - Replaces 64-bit absolute pointers with compact relative displacements (e.g. `i32` or `i16`).
//! - Guarantees position independence: structures can be directly dumped to disk or shared across
//!   processes without pointer swizzling.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

pub mod ptr;
pub use ptr::RelPtr;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[repr(C)]
    struct Node {
        value: u32,
        next: RelPtr<Node, i32>,
    }

    #[test]
    fn test_relative_pointer_size_and_resolution() {
        assert_eq!(size_of::<RelPtr<u32, i32>>(), 4);
        assert_eq!(size_of::<RelPtr<u32, i16>>(), 2);

        let mut node1 = Node {
            value: 42,
            next: RelPtr::from_raw_offset(0),
        };
        let node2 = Node {
            value: 99,
            next: RelPtr::from_raw_offset(0),
        };

        // Wire node1 -> node2
        node1.next = RelPtr::new(&node1.next, &node2);

        // Resolve pointer safely
        unsafe {
            let resolved = &*node1.next.as_ptr();
            assert_eq!(resolved.value, 99);
        }
    }

    #[test]
    fn test_resolve_within_arena_bounds() {
        #[repr(C, align(4))]
        struct ArenaBuffer {
            header: u32,
            target: u32,
            ptr: RelPtr<u32, i32>,
        }

        let mut buf = ArenaBuffer {
            header: 1,
            target: 1337,
            ptr: RelPtr::from_raw_offset(0),
        };

        buf.ptr = RelPtr::new(&buf.ptr, &buf.target);

        let arena_bytes = unsafe {
            core::slice::from_raw_parts(
                &buf as *const _ as *const u8,
                core::mem::size_of::<ArenaBuffer>(),
            )
        };

        unsafe {
            let target_ref = buf.ptr.resolve_within(arena_bytes);
            assert_eq!(target_ref, Some(&1337));
        }
    }
}
