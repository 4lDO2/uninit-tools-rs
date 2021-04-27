use core::mem::MaybeUninit;

use ioslice::init_marker::*;
use ioslice::IoSliceMut;

#[cfg(feature = "ioslice-iobox")]
use {crate::traits::Equivalent, ioslice::IoBox};

use crate::traits::{Initialize, InitializeVectored, TrustedDeref};
use crate::wrappers::{AssertInit, AssertInitVectors};

impl<'a, 'b, I: InitMarker> From<AssertInitVectors<&'b mut [IoSliceMut<'a, I>]>>
    for &'b mut [IoSliceMut<'a, Init>]
{
    fn from(init_vectors: AssertInitVectors<&'b mut [IoSliceMut<'a, I>]>) -> Self {
        unsafe { IoSliceMut::cast_to_init_slices_mut(init_vectors.into_inner()) }
    }
}
impl<'a, I: InitMarker, const N: usize> From<AssertInitVectors<[IoSliceMut<'a, I>; N]>>
    for [IoSliceMut<'a, Init>; N]
{
    fn from(init_vectors: AssertInitVectors<[IoSliceMut<'a, I>; N]>) -> Self {
        // NOTE: Because of https://github.com/rust-lang/rust/issues/61956, it is impossible to
        // transmute [T; N] to [U; N] if N is not known directly in the type signature. Another
        // complication is that we cannot use transmute_copy directly either, as that would create
        // two equivalent array containing two copies of the same mutable references! Thus, we have
        // to wrap the old array in MaybeUninit, temporarily removing the aliasing invariant. We
        // then perform the bitwise copy into the new type, and assume initializedness.
        unsafe {
            let general: MaybeUninit<[IoSliceMut<'a, I>; N]> =
                MaybeUninit::new(init_vectors.into_inner());
            // SAFETY: This is safe, because having wrapped the mutable references in MaybeUninit
            // allows us to alias the MaybeUninit value as much as we want to.
            let init: MaybeUninit<[IoSliceMut<'a, Init>; N]> = core::mem::transmute_copy(&general);

            // SAFETY: Since the value was already wrapped in MaybeUninit::new, it has been
            // initialized for type [IoSliceMut<'a, I>; N]. But, since that type and
            // [IoSliceMut<'a, Init>; N] both have the same layout as iovec, it will also be
            // initialized for the target type.
            init.assume_init()

            // SAFETY: And finally, IoSliceMut neither has a Drop implementation, nor will that
            // ever be possible for the old value to run a destructor when wrapped in MaybeUninit.
        }
    }
}
unsafe impl<'a, I: InitMarker> Initialize for IoSliceMut<'a, I> {
    type Item = u8;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        #[forbid(unconditional_recursion)]
        IoSliceMut::as_maybe_uninit_slice(self)
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        #[forbid(unconditional_recursion)]
        IoSliceMut::as_maybe_uninit_slice_mut(self)
    }
}
impl<'a, I: InitMarker> From<AssertInit<IoSliceMut<'a, I>>> for IoSliceMut<'a, Init> {
    #[inline]
    fn from(init_ioslice: AssertInit<IoSliceMut<'a, I>>) -> IoSliceMut<'a, Init> {
        #[forbid(unconditional_recursion)]
        unsafe {
            IoSliceMut::assume_init(init_ioslice.into_inner())
        }
    }
}
unsafe impl<'a, 'b, I: InitMarker> InitializeVectored for &'b mut [IoSliceMut<'a, I>] {
    type UninitVector = IoSliceMut<'a, Uninit>;

    #[inline]
    fn as_maybe_uninit_vectors(&self) -> &[Self::UninitVector] {
        IoSliceMut::cast_to_uninit_slices(self)
    }
    #[inline]
    unsafe fn as_maybe_uninit_vectors_mut(&mut self) -> &mut [Self::UninitVector] {
        IoSliceMut::cast_to_uninit_slices_mut(self)
    }
}
unsafe impl<'a, I: InitMarker, const N: usize> InitializeVectored for [IoSliceMut<'a, I>; N] {
    type UninitVector = IoSliceMut<'a, Uninit>;

    #[inline]
    fn as_maybe_uninit_vectors(&self) -> &[Self::UninitVector] {
        IoSliceMut::cast_to_uninit_slices(self)
    }
    #[inline]
    unsafe fn as_maybe_uninit_vectors_mut(&mut self) -> &mut [Self::UninitVector] {
        IoSliceMut::cast_to_uninit_slices_mut(self)
    }
}

#[cfg(feature = "ioslice-iobox")]
unsafe impl<I: InitMarker> Initialize for IoBox<I> {
    type Item = u8;

    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        #[forbid(unconditional_recursion)]
        IoBox::as_maybe_uninit_slice(self)
    }
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        #[forbid(unconditional_recursion)]
        IoBox::as_maybe_uninit_slice_mut(self)
    }
}
#[cfg(feature = "ioslice-iobox")]
impl<I: InitMarker> From<AssertInit<IoBox<I>>> for IoBox<Init> {
    #[inline]
    fn from(init_iobox: AssertInit<IoBox<I>>) -> IoBox<Init> {
        let (ptr, len) = init_iobox.into_inner().into_raw_parts();
        let ptr = ptr as *mut u8;
        unsafe { IoBox::from_raw_parts(ptr, len) }
    }
}

// TODO: Document safety.
unsafe impl<'a, I: InitMarker> TrustedDeref for ioslice::IoSlice<'a, I> {}
unsafe impl<'a, I: InitMarker> TrustedDeref for ioslice::IoSliceMut<'a, I> {}

#[cfg(feature = "ioslice_iobox")]
unsafe impl<'a, I: InitMarker> TrustedDeref for ioslice::IoBox<'a, I> {}

impl<'a, T> ioslice::CastSlice<'a, T> for crate::wrappers::AsUninit<T> {
    fn cast_slice(selves: &[Self]) -> &[T] {
        // SAFETY: This is safe, because AsUninit is marked #[repr(transparent)], and is thus
        // guaranteed to follow the exact same layout and ABI as the type T.
        unsafe { crate::cast_slice_same_layout::<Self, T>(selves) }
    }
}
impl<'a, T> ioslice::CastSliceMut<'a, T> for crate::wrappers::AsUninit<T> {
    fn cast_slice_mut(selves: &mut [Self]) -> &mut [T] {
        // SAFETY: This is safe due to AsUninit being #[repr(transparent)]. Additionally, the
        // AsUninit comes with the fundamental property of wrapping an already initialized type, so
        // de-initialization is impossible here.
        unsafe { crate::cast_slice_same_layout_mut::<Self, T>(selves) }
    }
}
// TODO: Document safety further.
// NOTE: The initialization markers have no effect on the Initialize impl, whatsoever.
#[cfg(feature = "ioslice-iobox")]
unsafe impl<'a, I: InitMarker, J: InitMarker> Equivalent<IoSliceMut<'a, I>> for IoBox<J> {}
#[cfg(feature = "ioslice-iobox")]
unsafe impl<'a, I: InitMarker, J: InitMarker> Equivalent<IoBox<I>> for IoSliceMut<'a, J> {}

// TODO: Find a better abstraction for this. I am not sure though, whether the trait system is even
// capable of this without HKT.
