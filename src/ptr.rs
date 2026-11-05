//! Core implementation of relative pointer `RelPtr<T, I>`.

use core::fmt;
use core::marker::PhantomData;

/// Position-independent relative pointer resolving target `T` via signed or unsigned offset `I`.
///
/// Unlike standard 64-bit raw pointers (`*const T`), `RelPtr` stores the byte displacement
/// relative to its own location in memory. This enables:
/// 1. **Zero-Copy Memory Mapping (`mmap`)**: The entire structure can be persisted or sent
///    across processes without pointer swizzling or base address fixups.
/// 2. **Pointer Compression**: Using `i32` or `i16` instead of a 64-bit pointer saves 50–75%
///    of pointer metadata in node structures.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPtr<T, I = i32> {
    offset: I,
    _marker: PhantomData<*const T>,
}

impl<T, I: Copy> RelPtr<T, I> {
    /// Creates a new `RelPtr` with the given raw offset.
    #[inline(always)]
    pub const fn from_raw_offset(offset: I) -> Self {
        Self {
            offset,
            _marker: PhantomData,
        }
    }

    /// Returns the raw relative displacement offset.
    #[inline(always)]
    pub const fn raw_offset(&self) -> I {
        self.offset
    }
}

impl<T> RelPtr<T, i32> {
    /// Creates a relative pointer pointing from `from` to `to`.
    ///
    /// # Panics
    /// Panics if the byte distance between `from` and `to` exceeds the range of `i32`.
    pub fn new(from: *const Self, to: *const T) -> Self {
        let from_addr = from as usize as isize;
        let to_addr = to as usize as isize;
        let diff = to_addr - from_addr;
        let offset = i32::try_from(diff).expect("relative pointer displacement exceeds i32 range");
        Self::from_raw_offset(offset)
    }

    /// Resolves the raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned instance of `T`.
    #[inline(always)]
    pub unsafe fn as_ptr(&self) -> *const T {
        // SAFETY: Pointer offset within caller-guaranteed valid range.
        unsafe {
            let base = self as *const _ as *const u8;
            base.offset(self.offset as isize) as *const T
        }
    }

    /// Resolves a mutable raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned, mutable instance of `T`.
    #[inline(always)]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        // SAFETY: Pointer offset within caller-guaranteed valid range.
        unsafe {
            let base = self as *mut _ as *mut u8;
            base.offset(self.offset as isize) as *mut T
        }
    }

    /// Safely dereferences the relative pointer within a bounded arena/buffer slice.
    ///
    /// Returns `Some(&'a T)` if the resolved target lies strictly within the byte bounds of `arena`.
    ///
    /// # Safety
    /// Target data in `arena` at the offset must be an initialized, valid instance of `T` with correct alignment.
    pub unsafe fn resolve_within<'a>(&self, arena: &'a [u8]) -> Option<&'a T> {
        // SAFETY: Validated by bounds check below.
        let target_ptr = unsafe { self.as_ptr() };
        let target_addr = target_ptr as usize;
        let arena_start = arena.as_ptr() as usize;
        let arena_end = arena_start + arena.len();

        let target_size = core::mem::size_of::<T>();
        if target_addr >= arena_start && target_addr + target_size <= arena_end {
            // SAFETY: Caller guarantees valid data initialization; bounds check guarantees non-dangling arena reference.
            Some(unsafe { &*target_ptr })
        } else {
            None
        }
    }
}

impl<T, I: fmt::Debug> fmt::Debug for RelPtr<T, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RelPtr")
            .field("offset", &self.offset)
            .finish()
    }
}
