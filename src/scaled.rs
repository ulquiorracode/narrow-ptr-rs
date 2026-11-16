//! Scaled relative pointer for increased addressable range on aligned types.

use crate::offset::Offset;
use core::fmt;
use core::marker::PhantomData;

/// Relative pointer where the raw offset is scaled by `2^SHIFT` bytes.
///
/// For example, with `SHIFT = 2` (4-byte alignment):
/// - An `i16` offset reaches $\pm 128\text{ KB}$ instead of $\pm 32\text{ KB}$.
/// - An `i32` offset reaches $\pm 8\text{ GB}$ instead of $\pm 2\text{ GB}$.
/// - An `odd_int::I24` offset reaches $\pm 32\text{ MB}$ instead of $\pm 8\text{ MB}$.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScaledRelPtr<T, I = i32, const SHIFT: usize = 2> {
    offset: I,
    _marker: PhantomData<*const T>,
}

impl<T, I: Copy, const SHIFT: usize> ScaledRelPtr<T, I, SHIFT> {
    /// Creates a new `ScaledRelPtr` from a raw scaled offset.
    #[inline(always)]
    pub const fn from_raw_offset(offset: I) -> Self {
        Self {
            offset,
            _marker: PhantomData,
        }
    }

    /// Returns the raw scaled displacement offset.
    #[inline(always)]
    pub const fn raw_offset(&self) -> I {
        self.offset
    }
}

impl<T, I: Offset, const SHIFT: usize> ScaledRelPtr<T, I, SHIFT> {
    /// Creates a scaled relative pointer pointing from `from` to `to`.
    ///
    /// # Panics
    /// Panics if the byte difference is not aligned to `1 << SHIFT` or exceeds `I` range.
    pub fn new(from: *const Self, to: *const T) -> Self {
        Self::try_new(from, to).expect("scaled relative pointer offset out of range or unaligned")
    }

    /// Attempts to create a scaled relative pointer from `from` to `to`.
    pub fn try_new(from: *const Self, to: *const T) -> Option<Self> {
        let from_addr = from as usize as isize;
        let to_addr = to as usize as isize;
        let diff = to_addr - from_addr;
        let step = 1isize << SHIFT;
        if diff % step != 0 {
            return None;
        }
        let scaled = diff >> SHIFT;
        I::from_isize(scaled).map(Self::from_raw_offset)
    }

    /// Resolves the raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned instance of `T`.
    #[inline(always)]
    pub unsafe fn as_ptr(&self) -> *const T {
        // SAFETY: wrapping_offset computes the address without in-bounds LLVM provenance UB hazards.
        let base = self as *const _ as *const u8;
        let byte_diff = self.offset.to_isize().wrapping_shl(SHIFT as u32);
        base.wrapping_offset(byte_diff) as *const T
    }

    /// Resolves a mutable raw pointer to `T`.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned, mutable instance of `T`.
    #[inline(always)]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        // SAFETY: wrapping_offset computes the address without in-bounds LLVM provenance UB hazards.
        let base = self as *mut _ as *mut u8;
        let byte_diff = self.offset.to_isize().wrapping_shl(SHIFT as u32);
        base.wrapping_offset(byte_diff) as *mut T
    }

    /// Safely dereferences the relative pointer within a bounded arena/buffer slice.
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
            // SAFETY: Caller guarantees valid data initialization; bounds & alignment checks guarantee non-dangling, aligned reference.
            Some(unsafe { &*target_ptr })
        } else {
            None
        }
    }

    /// Safely dereferences the mutable relative pointer within a bounded arena/buffer slice.
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
            // SAFETY: Caller guarantees valid data initialization; bounds & alignment checks guarantee non-dangling, aligned reference.
            Some(unsafe { &mut *target_ptr })
        } else {
            None
        }
    }
}

impl<T, I: fmt::Debug, const SHIFT: usize> fmt::Debug for ScaledRelPtr<T, I, SHIFT> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScaledRelPtr")
            .field("offset", &self.offset)
            .field("shift", &SHIFT)
            .finish()
    }
}
