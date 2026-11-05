//! Offset representation trait for relative displacements.

/// Trait implemented by types that can represent a byte offset or displacement.
pub trait Offset: Copy + Sized {
    /// Converts this offset into an `isize` byte count.
    fn to_isize(self) -> isize;

    /// Attempts to construct this offset from an `isize` byte count.
    ///
    /// Returns `None` if `offset` is out of the representable range of `Self`.
    fn from_isize(offset: isize) -> Option<Self>;
}

macro_rules! impl_primitive_offset {
    ($($ty:ty),*) => {
        $(
            impl Offset for $ty {
                #[inline(always)]
                fn to_isize(self) -> isize {
                    self as isize
                }

                #[inline(always)]
                fn from_isize(offset: isize) -> Option<Self> {
                    <$ty>::try_from(offset).ok()
                }
            }
        )*
    };
}

impl_primitive_offset!(i8, i16, i32, i64, isize);

#[cfg(feature = "odd-int")]
impl<const BITS: usize, const BYTES: usize> Offset for odd_int::Int<BITS, BYTES> {
    #[inline(always)]
    fn to_isize(self) -> isize {
        odd_int::Int::to_isize(&self)
    }

    #[inline(always)]
    fn from_isize(offset: isize) -> Option<Self> {
        Self::from_i64(offset as i64).ok()
    }
}
