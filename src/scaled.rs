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
pub struct ScaledRelPtr<T, I = i32, const SHIFT: usize = 2> {
    offset: I,
    _marker: PhantomData<*const T>,
}

unsafe impl<T: Send, I: Send, const SHIFT: usize> Send for ScaledRelPtr<T, I, SHIFT> {}
unsafe impl<T: Sync, I: Sync, const SHIFT: usize> Sync for ScaledRelPtr<T, I, SHIFT> {}

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
        if SHIFT >= usize::BITS as usize {
            return None;
        }
        let align = core::mem::align_of::<T>();
        if (to as usize) % align != 0 {
            return None;
        }
        let from_addr = from as usize as isize;
        let to_addr = to as usize as isize;
        let diff = to_addr - from_addr;
        let step = 1isize.checked_shl(SHIFT as u32)?;
        if diff % step != 0 {
            return None;
        }
        let scaled = diff >> SHIFT;
        I::from_isize(scaled).map(Self::from_raw_offset)
    }

    /// Rebases this scaled relative pointer when the containing struct/buffer has moved in memory.
    ///
    /// Given the old address of `self` (`old_from`) and the new address of `self` (`new_from`),
    /// this adjusts the stored offset so that it continues pointing to the original target.
    ///
    /// Returns `None` if the new displacement exceeds the range of `I` or is not properly aligned to `2^SHIFT`.
    pub fn try_rebase(&mut self, old_from: *const Self, new_from: *const Self) -> Option<()> {
        if SHIFT >= usize::BITS as usize {
            return None;
        }
        let old_addr = old_from as usize as isize;
        let new_addr = new_from as usize as isize;
        let current_byte_diff = self.offset.to_isize().checked_shl(SHIFT as u32)?;
        let target_addr = old_addr.checked_add(current_byte_diff)?;
        let align = core::mem::align_of::<T>();
        if (target_addr as usize) % align != 0 {
            return None;
        }
        let new_diff = target_addr.checked_sub(new_addr)?;
        let step = 1isize.checked_shl(SHIFT as u32)?;
        if new_diff % step != 0 {
            return None;
        }
        let scaled = new_diff >> SHIFT;
        let new_offset = I::from_isize(scaled)?;
        self.offset = new_offset;
        Some(())
    }

    /// Rebases this scaled relative pointer when the containing struct/buffer has moved in memory.
    ///
    /// # Panics
    /// Panics if the rebased displacement exceeds the range of `I` or is unaligned.
    pub fn rebase(&mut self, old_from: *const Self, new_from: *const Self) {
        self.try_rebase(old_from, new_from).expect(
            "rebased scaled relative pointer displacement exceeds offset range or is unaligned",
        );
    }

    /// Resolves the raw pointer to `T` using exposed provenance.
    ///
    /// # Memory Model & Provenance
    /// This method computes the absolute target address from `self`'s address and the scaled relative displacement,
    /// restoring the pointer via exposed provenance (`target_addr as *const T`).
    ///
    /// For strict bounds checking and single-allocation provenance, prefer [`ScaledRelPtr::resolve_within`],
    /// which derives the reference directly from the enclosing arena slice.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned instance of `T`.
    #[inline(always)]
    pub unsafe fn as_ptr(&self) -> *const T {
        if SHIFT >= usize::BITS as usize {
            return core::ptr::null();
        }
        let base_addr = self as *const _ as usize;
        let byte_diff = (self.offset.to_isize() as usize).wrapping_shl(SHIFT as u32);
        let target_addr = base_addr.wrapping_add(byte_diff);
        target_addr as *const T
    }

    /// Resolves a mutable raw pointer to `T` using exposed provenance.
    ///
    /// # Safety
    /// Caller must ensure that `self` is placed at a valid memory location and that
    /// the calculated address points to a valid, properly aligned, mutable instance of `T`.
    #[inline(always)]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        if SHIFT >= usize::BITS as usize {
            return core::ptr::null_mut();
        }
        let base_addr = self as *mut _ as usize;
        let byte_diff = (self.offset.to_isize() as usize).wrapping_shl(SHIFT as u32);
        let target_addr = base_addr.wrapping_add(byte_diff);
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
        if target_ptr.is_null() {
            return None;
        }
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
            let ptr = unsafe { arena.as_ptr().add(offset_in_arena).cast::<T>() };
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
    pub unsafe fn resolve_within_mut<'a>(&self, arena: &'a mut [u8]) -> Option<&'a mut T> {
        let target_ptr = unsafe { self.as_ptr() };
        if target_ptr.is_null() {
            return None;
        }
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
            let ptr = unsafe { arena.as_mut_ptr().add(offset_in_arena).cast::<T>() };
            // SAFETY: Caller guarantees valid data initialization; bounds & alignment checks guarantee non-dangling, aligned reference with arena's provenance.
            Some(unsafe { &mut *ptr })
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
