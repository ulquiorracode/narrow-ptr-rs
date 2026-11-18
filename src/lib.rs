#![doc = include_str!("../README.md")]
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

        let mut nodes = [
            Node {
                value: 42,
                next: RelPtr::from_raw_offset(0),
            },
            Node {
                value: 99,
                next: RelPtr::from_raw_offset(0),
            },
        ];

        // Wire node 0 -> node 1 within contiguous allocation
        nodes[0].next = RelPtr::new(&nodes[0].next, &nodes[1]);

        // Resolve pointer safely
        unsafe {
            let resolved = &*nodes[0].next.as_ptr();
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

        let mut items = [
            DenseItem {
                id: 10,
                ptr: RelPtr::from_raw_offset(I24::ZERO),
            },
            DenseItem {
                id: 20,
                ptr: RelPtr::from_raw_offset(I24::ZERO),
            },
        ];

        items[0].ptr = RelPtr::new(&items[0].ptr, &items[1]);

        unsafe {
            let id = core::ptr::read_unaligned(core::ptr::addr_of!((*items[0].ptr.as_ptr()).id));
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
        struct AlignedArena([u8; 64]);

        let mut arena = AlignedArena([0u8; 64]);
        let target_ptr = arena.0.as_mut_ptr() as *mut AlignedTarget;
        unsafe {
            (*target_ptr).val = 42;
        }

        let mut rel_ptr = RelPtr::<AlignedTarget, i32>::from_raw_offset(0);
        rel_ptr = RelPtr::new(&rel_ptr, target_ptr);

        unsafe {
            let res_mut = rel_ptr.resolve_within_mut(&mut arena.0);
            assert!(res_mut.is_some());
            let r = res_mut.unwrap();
            assert_eq!(r.val, 42);
            r.val = 84;
        }

        let updated_val = unsafe { (*(arena.0.as_ptr() as *const AlignedTarget)).val };
        assert_eq!(updated_val, 84);

        // Test unaligned pointer rejection: offset target by 1 byte
        let unaligned_ptr: RelPtr<AlignedTarget, i32> =
            RelPtr::from_raw_offset(rel_ptr.raw_offset() + 1);
        unsafe {
            assert!(unaligned_ptr.resolve_within(&arena.0).is_none());
        }
    }

    #[test]
    fn test_rel_ptr_rebase() {
        #[repr(C)]
        struct Arena {
            target: u32,
            p1: RelPtr<u32, i32>,
            p2: RelPtr<u32, i32>,
        }
        let mut arena = Arena {
            target: 12345,
            p1: RelPtr::from_raw_offset(0),
            p2: RelPtr::from_raw_offset(0),
        };
        arena.p1 = RelPtr::new(&arena.p1, &arena.target);
        arena.p2 = arena.p1;

        let old_addr = &arena.p1 as *const _;
        let new_addr = &arena.p2 as *const _;

        arena.p2.rebase(old_addr, new_addr);
        unsafe {
            assert_eq!(*arena.p2.as_ptr(), 12345);
        }
    }

    #[test]
    fn test_rel_ptr_block_move_invariance() {
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

        // Moving the whole self-contained block preserves relative offset without rebasing
        let b2 = b1;
        // Expose new allocation address so exposed-provenance resolution can target it
        let _ = &b2 as *const Block as usize;
        unsafe {
            assert_eq!(*b2.ptr.as_ptr(), 12345);
        }
    }

    #[test]
    fn test_scaled_rel_ptr_rebase() {
        #[repr(C, align(4))]
        struct Arena {
            target: u32,
            p1: ScaledRelPtr<u32, i32, 2>,
            p2: ScaledRelPtr<u32, i32, 2>,
        }
        let mut arena = Arena {
            target: 54321,
            p1: ScaledRelPtr::from_raw_offset(0),
            p2: ScaledRelPtr::from_raw_offset(0),
        };
        arena.p1 = ScaledRelPtr::new(&arena.p1, &arena.target);
        arena.p2 = arena.p1;

        let old_addr = &arena.p1 as *const _;
        let new_addr = &arena.p2 as *const _;

        arena.p2.rebase(old_addr, new_addr);
        unsafe {
            assert_eq!(*arena.p2.as_ptr(), 54321);
        }
    }

    #[test]
    fn test_scaled_rel_ptr_alignment_validation() {
        #[repr(C, align(8))]
        struct Align8(u64);

        let val = Align8(999);
        let ptr_slot = ScaledRelPtr::<Align8, i32, 1>::from_raw_offset(0);
        // Valid creation when target is properly aligned
        assert!(ScaledRelPtr::try_new(&ptr_slot, &val).is_some());

        // Unaligned target rejection (e.g. pointer with odd address)
        let unaligned_raw = ((&val as *const _ as usize) + 1) as *const Align8;
        assert!(ScaledRelPtr::try_new(&ptr_slot, unaligned_raw).is_none());
    }
}
