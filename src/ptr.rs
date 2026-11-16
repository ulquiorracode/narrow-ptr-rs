//! Core implementation of relative pointer `RelPtr<T, I>`.

use crate::offset::Offset;
use core::fmt;
use core::marker::PhantomData;

/// Position-independent relative pointer resolving target `T` via displacement `I`.
///
/// Unlike standard 64-bit raw pointers (`*const T`), `RelPtr` stores the byte displacement
/// relative to its own location in memory. This enables:
/// 1. **Zero-Copy Memory Mapping (`mmap`)**: The entire structure can be persisted or sent
///    across processes without pointer swizzling or base address fixups.
/// 2. **Pointer Compression**: Using `i32`, `i16`, or `odd_int::I24` saves 50–75% of
///    pointer metadata in node structures.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
#[cfg_attr(
    feature = "zerocopy",
    derive(
        zerocopy::IntoBytes,
        zerocopy::FromBytes,
        zerocopy::KnownLayout,
        zerocopy::Immutable
    )
)]
pub struct RelPtr<T, I = i32> {
    offset: I,
    _marker: PhantomData<fn() -> *const T>,
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

impl<T, I: Offset> RelPtr<T, I> {
    /// Creates a relative pointer pointing from `from` to `to`.
    ///
    /// # Panics
    /// Panics if the byte distance between `from` and `to` exceeds the range of `I`.
    pub fn new(from: *const Self, to: *const T) -> Self {
        let from_addr = from as usize as isize;
        let to_addr = to as usize as isize;
        let diff = to_addr - from_addr;
        let offset =
            I::from_isize(diff).expect("relative pointer displacement exceeds offset range");
        Self::from_raw_offset(offset)
    }

    /// Attempts to create a relative pointer from `from` to `to`, returning `None` if out of range.
    pub fn try_new(from: *const Self, to: *const T) -> Option<Self> {
        let from_addr = from as usize as isize;
        let to_addr = to as usize as isize;
        let diff = to_addr - from_addr;
        I::from_isize(diff).map(Self::from_raw_offset)
    }

    /// Rebases this relative pointer when the containing struct/buffer has moved in memory.
    ///
    /// Given the old address of `self` (`old_from`) and the new address of `self` (`new_from`),
    /// this adjusts the stored offset so that it continues pointing to the original target.
    ///
    /// Returns `None` if the new displacement exceeds the range of `I`.
    pub fn try_rebase(&mut self, old_from: *const Self, new_from: *const Self) -> Option<()> {
        let old_addr = old_from as usize as isize;
        let new_addr = new_from as usize as isize;
        let current_offset = self.offset.to_isize();
        let target_addr = old_addr.checked_add(current_offset)?;
        let new_diff = target_addr.checked_sub(new_addr)?;
        let new_offset = I::from_isize(new_diff)?;
        self.offset = new_offset;
        Some(())
    }

    /// Rebases this relative pointer when the containing struct/buffer has moved in memory.
    ///
    /// # Panics
    /// Panics if the rebased displacement exceeds the range of `I`.
    pub fn rebase(&mut self, old_from: *const Self, new_from: *const Self) {
        self.try_rebase(old_from, new_from)
            .expect("rebased relative pointer displacement exceeds offset range");
    }

    /// Resolves the raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned instance of `T`.
    #[inline(always)]
    pub unsafe fn as_ptr(&self) -> *const T {
        let base_addr = self as *const _ as usize;
        let target_addr = (base_addr as isize).wrapping_add(self.offset.to_isize()) as usize;
        target_addr as *const T
    }

    /// Resolves a mutable raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned, mutable instance of `T`.
    #[inline(always)]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        let base_addr = self as *mut _ as usize;
        let target_addr = (base_addr as isize).wrapping_add(self.offset.to_isize()) as usize;
        target_addr as *mut T
    }

    /// Bounds- and alignment-checked dereferencing of the relative pointer within a bounded arena/buffer slice.
    ///
    /// Returns `Some(&'a T)` if the resolved target lies strictly within the byte bounds of `arena`
    /// and matches the memory alignment required by `T`.
    ///
    /// # Safety
    /// Target data in `arena` at the offset must be an initialized, valid instance of `T`.
    pub unsafe fn resolve_within<'a>(&self, arena: &'a [u8]) -> Option<&'a T> {
        let target_ptr = unsafe { self.as_ptr() };
        let target_addr = target_ptr as usize;
        let arena_start = arena.as_ptr() as usize;

        let align = core::mem::align_of::<T>();
        if target_addr % align != 0 {
            return None;
        }

        let target_size = core::mem::size_of::<T>();
        let target_end = target_addr.checked_add(target_size)?;
        let arena_end = arena_start.checked_add(arena.len())?;

        if target_addr >= arena_start && target_end <= arena_end {
            let offset_in_arena = target_addr - arena_start;
            let ptr = unsafe { arena.as_ptr().add(offset_in_arena) as *const T };
            // SAFETY: Caller guarantees valid data initialization; bounds & alignment checks guarantee non-dangling, aligned reference with arena's provenance.
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }

    /// Bounds- and alignment-checked dereferencing of the mutable relative pointer within a bounded arena/buffer slice.
    ///
    /// Returns `Some(&'a mut T)` if the resolved target lies strictly within the byte bounds of `arena`
    /// and matches the memory alignment required by `T`.
    ///
    /// # Safety
    /// Target data in `arena` at the offset must be an initialized, valid instance of `T`.
    pub unsafe fn resolve_within_mut<'a>(&mut self, arena: &'a mut [u8]) -> Option<&'a mut T> {
        let target_ptr = unsafe { self.as_mut_ptr() };
        let target_addr = target_ptr as usize;
        let arena_start = arena.as_ptr() as usize;

        let align = core::mem::align_of::<T>();
        if target_addr % align != 0 {
            return None;
        }

        let target_size = core::mem::size_of::<T>();
        let target_end = target_addr.checked_add(target_size)?;
        let arena_end = arena_start.checked_add(arena.len())?;

        if target_addr >= arena_start && target_end <= arena_end {
            let offset_in_arena = target_addr - arena_start;
            let ptr = unsafe { arena.as_mut_ptr().add(offset_in_arena) as *mut T };
            // SAFETY: Caller guarantees valid data initialization; bounds & alignment checks guarantee non-dangling, aligned reference with arena's provenance.
            Some(unsafe { &mut *ptr })
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
