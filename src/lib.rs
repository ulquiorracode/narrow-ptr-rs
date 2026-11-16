//! # narrow-ptr
//!
//! Position-independent relative and narrow pointer types for zero-copy memory mapping (`mmap`),
//! IPC, and cache-friendly data structures.
//!
//! Conforms to the "Scrooge" Systems Mindset:
//! - Replaces 64-bit absolute pointers with compact relative displacements (e.g. `i32`, `i16`, or `odd_int::I24`).
//! - Guarantees position independence: structures can be directly dumped to disk or shared across
//!   processes without pointer swizzling.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

pub mod offset;
pub mod ptr;
pub mod scaled;

pub use offset::Offset;
pub use ptr::RelPtr;
pub use scaled::ScaledRelPtr;

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

    #[cfg(feature = "odd-int")]
    #[test]
    fn test_narrow_ptr_with_odd_int_i24() {
        use odd_int::I24;

        #[repr(C, packed)]
        #[derive(Clone, Copy)]
        struct DenseItem {
            id: u16,
            ptr: RelPtr<DenseItem, I24>,
        }

        assert_eq!(size_of::<RelPtr<DenseItem, I24>>(), 3);
        assert_eq!(size_of::<DenseItem>(), 5);

        let mut item1 = DenseItem {
            id: 10,
            ptr: RelPtr::from_raw_offset(I24::ZERO),
        };
        let item2 = DenseItem {
            id: 20,
            ptr: RelPtr::from_raw_offset(I24::ZERO),
        };

        item1.ptr = RelPtr::new(&item1.ptr, &item2);

        unsafe {
            let id = core::ptr::read_unaligned(core::ptr::addr_of!((*item1.ptr.as_ptr()).id));
            assert_eq!(id, 20);
        }
    }

    #[test]
    fn test_scaled_rel_ptr() {
        #[repr(C, align(4))]
        struct Target {
            val: u32,
        }

        #[repr(C, align(4))]
        struct Container {
            first: Target,
            second: Target,
            ptr: ScaledRelPtr<Target, i16, 2>,
        }

        let mut c = Container {
            first: Target { val: 111 },
            second: Target { val: 222 },
            ptr: ScaledRelPtr::from_raw_offset(0),
        };

        c.ptr = ScaledRelPtr::new(&c.ptr, &c.second);

        unsafe {
            let resolved = &*c.ptr.as_ptr();
            assert_eq!(resolved.val, 222);
        }
    }

    #[test]
    fn test_resolve_within_mut_and_alignment() {
        #[repr(C, align(8))]
        struct AlignedTarget {
            val: u64,
        }

        #[repr(C, align(8))]
        struct Container {
            target: AlignedTarget,
            ptr: RelPtr<AlignedTarget, i32>,
        }

        let mut arena = [0u8; 64];
        let c_ptr = arena.as_mut_ptr() as *mut Container;
        unsafe {
            (*c_ptr).target.val = 42;
            (*c_ptr).ptr = RelPtr::new(&(*c_ptr).ptr, &(*c_ptr).target);

            // Access ptr via raw pointer so as not to alias with &mut arena
            let rel_ptr = &mut (*c_ptr).ptr;
            let res_mut = rel_ptr.resolve_within_mut(&mut arena);
            assert!(res_mut.is_some());
            let r = res_mut.unwrap();
            assert_eq!(r.val, 42);
            r.val = 84;
            assert_eq!((*c_ptr).target.val, 84);
        }

        // Test unaligned pointer rejection: offset target by 1 byte
        let unaligned_ptr: RelPtr<AlignedTarget, i32> =
            unsafe { RelPtr::from_raw_offset((*c_ptr).ptr.raw_offset() + 1) };
        unsafe {
            assert!(unaligned_ptr.resolve_within(&arena).is_none());
        }
    }

    #[test]
    fn test_rel_ptr_rebase() {
        #[repr(C)]
        struct Block {
            target: u32,
            ptr: RelPtr<u32, i32>,
        }

        let mut b1 = Block {
            target: 12345,
            ptr: RelPtr::from_raw_offset(0),
        };
        b1.ptr = RelPtr::new(&b1.ptr, &b1.target);

        let old_addr = &b1.ptr as *const _;
        let mut b2 = b1;
        let new_addr = &b2.ptr as *const _;

        b2.ptr.rebase(old_addr, new_addr);
        unsafe {
            assert_eq!(*b2.ptr.as_ptr(), 12345);
        }
    }

    #[test]
    fn test_scaled_rel_ptr_rebase() {
        #[repr(C, align(4))]
        struct Block {
            target: u32,
            ptr: ScaledRelPtr<u32, i32, 2>,
        }

        let mut b1 = Block {
            target: 54321,
            ptr: ScaledRelPtr::from_raw_offset(0),
        };
        b1.ptr = ScaledRelPtr::new(&b1.ptr, &b1.target);

        let old_addr = &b1.ptr as *const _;
        let mut b2 = b1;
        let new_addr = &b2.ptr as *const _;

        b2.ptr.rebase(old_addr, new_addr);
        unsafe {
            assert_eq!(*b2.ptr.as_ptr(), 54321);
        }
    }
}
