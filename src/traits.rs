use core::mem::MaybeUninit;

use crate::wrappers::AssertInit;

#[cfg(feature = "alloc")]
use {alloc::boxed::Box, alloc::vec::Vec};

/// A trait for mutable initializable slices, that provide access to all the data required for
/// initialization, before the data can be assumed to be fully initialized.
///
/// # Safety
///
/// This trait is unsafe to implement since whatever slices are returned from the casts here,
/// __must have the same length and point to the same memory as before__. This is to allow safer
/// abstractions to assume that there are has not unexpectedly appeared additional items that must
/// be initialized.
pub unsafe trait Initialize {
    type Item;

    /// Retrieve an immutable slice pointing to possibly uninitialized memory. __This must be
    /// exactly the same slice as the one from [`as_maybe_uninit_slice_mut`], or the trait
    /// implementation as a whole, gets incorrect.__
    ///
    /// [`as_maybe_uninit_slice_mut`]: Self::as_maybe_uninit_slice_mut
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<Self::Item>];

    /// Retrieve a mutable slice pointing to possibly uninitialized memory. __This must always
    /// point to the same slice as with previous invocations__.
    ///
    /// # Safety
    ///
    /// The caller must not use the resulting slice to de-initialize the data.
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<Self::Item>];
}

/// A trait for slices (or owned memory) that contain possibly uninitialized slices themselves.
/// That is, the [`Initialize`] trait but for singly-indirect slices.
///
/// # Safety
///
/// For this trait to be implemented correctly, [`as_maybe_uninit_vectors`] and
/// [`as_maybe_uninit_vectors_mut`] must always return the same slices (albeit with different
/// aliasing rules as they take `&self` and `&mut self` respectively).
///
/// [`as_maybe_uninit_vectors`]: InitializeVectored::as_maybe_uninit_vectors
/// [`as_maybe_uninit_vectors_mut`]: InitializeVectored::as_maybe_uninit_vectors_mut
pub unsafe trait InitializeVectored {
    /// The possibly uninitialized vector type, which must implement [`Initialize`]. Note that this
    /// does not necessarily need to deref into [`MaybeUninit<Item>`], but can be anything that is
    /// convertible to it.
    type UninitVector: Initialize;

    /// Get the uninitialized version of all vectors. This slice must always be exactly equal to
    /// the slice returned by
    /// [`as_maybe_uninit_vectors_mut`](InitializeVectored::as_maybe_uninit_vectors_mut), except
    /// being borrowed differently, or the trait is unsoundly implemented.
    ///
    /// [`as_maybe_uninit_slice_mut`]: InitializeVectored::as_maybe_uninit_slice_mut
    fn as_maybe_uninit_vectors(&self) -> &[Self::UninitVector];

    /// Get the uninitialized version of all vectors, mutably. This slice must always be exactly
    /// equal to the slice returned by [`as_maybe_uninit_vectors`](Self::as_maybe_uninit_vectors),
    /// or the trait is unsoundly implemented.
    ///
    /// # Safety
    ///
    /// For the user of this trait, the resulting slice returned from this method _must not_ be
    /// used to de-initialize the vectors by overwriting their contents with
    /// [`MaybeUninit::uninit`] if they were already initialized.
    ///
    /// [`as_maybe_uninit_slice`]: InitializeVectored::as_maybe_uninit_slice
    unsafe fn as_maybe_uninit_vectors_mut(&mut self) -> &mut [Self::UninitVector];
}
pub trait InitializeExt: private2::Sealed + Initialize + Sized {
    /// Assume that the type is already initialized. This is equivalent of calling
    /// [`AssertInit::new_unchecked`].
    ///
    /// # Safety
    ///
    /// The initialization invariant must be upheld for this to be safe.
    unsafe fn assume_init(self) -> AssertInit<Self> {
        AssertInit::new_unchecked(self)
    }
}
mod private2 {
    pub trait Sealed {}
}
mod private3 {
    pub trait Sealed {}
}
mod private4 {
    pub trait Sealed {}
}
mod private5 {
    pub trait Sealed {}
}

impl<T> private2::Sealed for T where T: Initialize {}
impl<T> InitializeExt for T where T: Initialize {}

unsafe impl<'a, T> Initialize for &'a mut [MaybeUninit<T>] {
    type Item = T;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<T>] {
        self
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        self
    }
}
impl<'a, T> From<AssertInit<&'a mut [MaybeUninit<T>]>> for &'a mut [T] {
    #[inline]
    fn from(init_slice: AssertInit<&'a mut [MaybeUninit<T>]>) -> &'a mut [T] {
        unsafe { crate::cast_uninit_to_init_slice_mut(init_slice.into_inner()) }
    }
}
unsafe impl<T> InitializeVectored for T
where
    T: Initialize,
{
    type UninitVector = Self;

    #[inline]
    fn as_maybe_uninit_vectors(&self) -> &[Self::UninitVector] {
        core::slice::from_ref(self)
    }
    #[inline]
    unsafe fn as_maybe_uninit_vectors_mut(&mut self) -> &mut [Self::UninitVector] {
        core::slice::from_mut(self)
    }
}
unsafe impl<'a, 'b, T> InitializeVectored for &'a mut [&'b mut [MaybeUninit<T>]] {
    type UninitVector = &'b mut [MaybeUninit<T>];

    fn as_maybe_uninit_vectors(&self) -> &[Self::UninitVector] {
        self
    }
    unsafe fn as_maybe_uninit_vectors_mut(&mut self) -> &mut [Self::UninitVector] {
        self
    }
}
#[cfg(feature = "alloc")]
unsafe impl<T> Initialize for Box<[MaybeUninit<T>]> {
    type Item = T;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<T>] {
        self
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        self
    }
}
#[cfg(feature = "alloc")]
impl<T> From<AssertInit<Box<[MaybeUninit<T>]>>> for Box<[T]> {
    #[inline]
    fn from(init_box: AssertInit<Box<[MaybeUninit<T>]>>) -> Box<[T]> {
        #[cfg(feature = "nightly")]
        unsafe {
            #[forbid(unconditional_recursion)]
            Box::<[MaybeUninit<T>]>::assume_init(init_box.into_inner())
        }
        #[cfg(not(feature = "nightly"))]
        unsafe {
            let slice_ptr = Box::into_raw(init_box.into_inner());
            Box::from_raw(crate::cast_uninit_to_init_slice_mut(&mut *slice_ptr))
        }
    }
}
/*
#[cfg(feature = "alloc")]
unsafe impl Initialize for Vec<Item> {
    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        crate::cast_init_to_uninit_slice(&*self)
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // TODO: Give the whole allocation, and not just the length set? With MaybeUninit, calling
        // set_len is safe.
        crate::cast_init_to_uninit_slice_mut(&mut *self)
    }
}
#[cfg(feature = "alloc")]
unsafe impl Initialize for Vec<MaybeUninit<u8>> {
    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<u8>] {
        &*self
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut *self
    }
}*/
#[cfg(feature = "alloc")]
impl<T> From<AssertInit<Vec<MaybeUninit<T>>>> for Vec<T> {
    #[inline]
    fn from(init_vec: AssertInit<Vec<MaybeUninit<T>>>) -> Vec<T> {
        unsafe {
            let mut vec = init_vec.into_inner();
            //let (ptr, cap, len) = Vec::into_raw_parts(self);

            let (ptr, cap, len) = {
                let ptr = vec.as_mut_ptr();
                let cap = vec.capacity();
                let len = vec.len();

                core::mem::forget(vec);

                (ptr, cap, len)
            };

            Vec::from_raw_parts(ptr as *mut T, cap, len)
        }
    }
}
unsafe impl<T, const N: usize> Initialize for [MaybeUninit<T>; N] {
    type Item = T;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<T>] {
        self
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        self
    }
}
impl<T, const N: usize> From<AssertInit<[MaybeUninit<T>; N]>> for [T; N] {
    #[inline]
    fn from(init: AssertInit<[MaybeUninit<T>; N]>) -> [T; N] {
        #[cfg(feature = "nightly")]
        unsafe {
            MaybeUninit::array_assume_init(init.into_inner())
        }
        #[cfg(not(feature = "nightly"))]
        unsafe {
            let inner = init.into_inner();
            let init: [T; N] = core::mem::transmute_copy(&inner);
            init
        }
    }
}
