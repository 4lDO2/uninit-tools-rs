use core::mem::MaybeUninit;

use ioslice::IoSliceMut;
use ioslice::init_marker::*;

#[cfg(feature = "ioslice-iobox")]
use ioslice::IoBox;

use crate::traits::{Initialize, InitializeVectored, TrustedDeref};
use crate::wrappers::{AssertInit, AssertInitVectors};

impl<'a, 'b, I: InitMarker> From<AssertInitVectors<&'b mut [IoSliceMut<'a, I>]>>
    for &'b mut [IoSliceMut<'a, Init>]
{
    fn from(init_vectors: AssertInitVectors<&'b mut [IoSliceMut<'a, I>]>) -> Self {
        unsafe { IoSliceMut::cast_to_init_slices_mut(init_vectors.into_inner()) }
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
