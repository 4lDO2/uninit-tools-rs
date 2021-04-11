use core::borrow::{Borrow, BorrowMut};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

use crate::traits::{Initialize, TrustedDeref};

/// A wrapper over `T` that assumes all of `T` to be initialized.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct AssertInit<T> {
    inner: T,
}

impl<T> AssertInit<T> {
    /// Wrap a possibly-uninitialized value `inner` into, assuming that it is fully initialized.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `inner` is fully initialized before this function can be
    /// called.
    #[inline]
    pub const unsafe fn new_unchecked(inner: T) -> Self {
        Self { inner }
    }
    /// Cast `&[T]` to `&[AssertInit<T>]`.
    ///
    /// # Safety
    ///
    /// This is unsafe because the caller has to ensure that all instances of type `T`, uphold the
    /// initialization invariant.
    #[inline]
    pub unsafe fn cast_from_slices(inner_slices: &[T]) -> &[Self] {
        // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the slices have
        // the same layout. The only contract that the caller has to follow, is that the data must
        // actually be initialized.
        core::slice::from_raw_parts(inner_slices.as_ptr() as *const Self, inner_slices.len())
    }
    /// Cast `&mut [T]` to `&mut [AssertInit<T>]`.
    ///
    /// # Safety
    ///
    /// This is unsafe because the caller has to ensure that all instances of type `T`, uphold the
    /// initialization invariant.
    #[inline]
    pub unsafe fn cast_from_slices_mut(inner_slices: &mut [T]) -> &mut [Self] {
        // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the slices have
        // the same layout. The only contract that the caller has to follow, is that the data must
        // actually be initialized.
        core::slice::from_raw_parts_mut(inner_slices.as_ptr() as *mut Self, inner_slices.len())
    }
    /// Cast `&[AssertInit<T>]` to `&mut [AssertInit<T>]`.
    #[inline]
    pub fn cast_to_uninit_slices(selves: &[Self]) -> &[T] {
        unsafe {
            // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the slices have the
            // same layout.
            //
            // Since the returned slice is immutable, nothing can be deinitialized.
            core::slice::from_raw_parts(selves.as_ptr() as *const T, selves.len())
        }
    }
    /// Cast `&mut [AssertInit<T>]` to `&mut [T]`.
    ///
    /// # Safety
    ///
    /// This is unsafe because it allows for de-initialization, which is very unlikely to happen by
    /// accident in practice, but which still would be unsound. The caller must simply never
    /// overwrite an already initialized value with [`MaybeUninit::uninit()`].
    #[inline]
    pub unsafe fn cast_to_uninit_slices_mut(selves: &mut [Self]) -> &mut [T] {
        // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the slices have
        // the same layout. The only contract that the caller has to follow, is that the data must
        // never be de-initialized.
        core::slice::from_raw_parts_mut(selves.as_ptr() as *mut T, selves.len())
    }
    #[inline]
    pub fn from_ref(inner_slice: &T) -> &Self {
        unsafe {
            // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the
            // references have the same layout. The only contract that the caller has to follow, is
            // that the data must actually be initialized.
            &*(inner_slice as *const T as *const Self)
        }
    }
    #[inline]
    pub fn from_mut(inner_slice: &mut T) -> &mut Self {
        unsafe {
            // SAFETY: This is safe because AssertInit is #[repr(transparent)], making the
            // references have the same layout. The only contract that the caller has to follow, is
            // that the data must actually be initialized.
            &mut *(inner_slice as *mut T as *mut Self)
        }
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
    #[inline]
    pub const fn inner(&self) -> &T {
        &self.inner
    }
    #[inline]
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
impl<T> AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    pub fn get_init_ref(&self) -> &[T::Item] {
        unsafe { crate::cast_uninit_to_init_slice(self.inner().as_maybe_uninit_slice()) }
    }
    #[inline]
    pub fn get_init_mut(&mut self) -> &mut [T::Item] {
        unsafe {
            crate::cast_uninit_to_init_slice_mut(self.inner_mut().as_maybe_uninit_slice_mut())
        }
    }
    #[inline]
    pub fn get_uninit_ref(&self) -> &[MaybeUninit<T::Item>] {
        self.inner().as_maybe_uninit_slice()
    }
    /// Get a mutable slice to the inner uninitialized slice.
    ///
    /// # Safety
    ///
    /// Since the [`Initialize`] trait is generic over both already initialized and uninitialized
    /// types, it is unsafe to retrieve an uninitialized slice to already initialized data, because
    /// it allows for de-initialization. (This can happen when overwriting the resulting slice with
    /// [`MaybeUninit::uninit()`].
    #[inline]
    pub unsafe fn get_uninit_mut(&mut self) -> &mut [MaybeUninit<T::Item>] {
        self.inner_mut().as_maybe_uninit_slice_mut()
    }
}
impl<T> AsRef<[T::Item]> for AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    fn as_ref(&self) -> &[T::Item] {
        self.get_init_ref()
    }
}
impl<T> AsMut<[T::Item]> for AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    fn as_mut(&mut self) -> &mut [T::Item] {
        self.get_init_mut()
    }
}
impl<T> Borrow<[T::Item]> for AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    fn borrow(&self) -> &[T::Item] {
        self.get_init_ref()
    }
}
impl<T> BorrowMut<[T::Item]> for AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T::Item] {
        self.get_init_mut()
    }
}
impl<T> Deref for AssertInit<T>
where
    T: Initialize,
{
    type Target = [T::Item];

    #[inline]
    fn deref(&self) -> &[T::Item] {
        self.get_init_ref()
    }
}
impl<T> DerefMut for AssertInit<T>
where
    T: Initialize,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut [T::Item] {
        self.get_init_mut()
    }
}
impl<T> PartialEq for AssertInit<T>
where
    T: Initialize,
    T::Item: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.get_init_ref() == other.get_init_ref()
    }
}
impl<T> Eq for AssertInit<T>
where
    T: Initialize,
    T::Item: Eq,
{
}
impl<T> PartialOrd for AssertInit<T>
where
    T: Initialize,
    T::Item: Ord,
{
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}
impl<T> Ord for AssertInit<T>
where
    T: Initialize,
    T::Item: Ord,
{
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        Ord::cmp(self.get_init_ref(), other.get_init_ref())
    }
}
impl<T> core::hash::Hash for AssertInit<T>
where
    T: Initialize,
    T::Item: core::hash::Hash,
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.get_init_ref().hash(state)
    }
}

#[repr(transparent)]
pub struct AssertInitVectors<I> {
    inner: I,
}
impl<I> AssertInitVectors<I> {
    pub const unsafe fn new_unchecked(inner: I) -> Self {
        Self { inner }
    }
    #[inline]
    pub fn into_inner(self) -> I {
        self.inner
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SingleVector<T>(pub T);

impl<T> AsRef<[T]> for SingleVector<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        core::slice::from_ref(&self.0)
    }
}
impl<T> AsMut<[T]> for SingleVector<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        core::slice::from_mut(&mut self.0)
    }
}
impl<T> AsRef<T> for SingleVector<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.0
    }
}
impl<T> AsMut<T> for SingleVector<T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T> Borrow<T> for SingleVector<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.0
    }
}
impl<T> BorrowMut<T> for SingleVector<T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
impl<T> Borrow<[T]> for SingleVector<T> {
    #[inline]
    fn borrow(&self) -> &[T] {
        core::slice::from_ref(&self.0)
    }
}
impl<T> BorrowMut<[T]> for SingleVector<T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        core::slice::from_mut(&mut self.0)
    }
}

impl<T> Deref for SingleVector<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for SingleVector<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
// TODO: Add test.
unsafe impl<T: Initialize> Initialize for SingleVector<T> {
    type Item = <T as Initialize>::Item;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<Self::Item>] {
        <T as Initialize>::as_maybe_uninit_slice(&self.0)
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<Self::Item>] {
        <T as Initialize>::as_maybe_uninit_slice_mut(&mut self.0)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct AsUninit<T>(pub T);

impl<T> Deref for AsUninit<T>
where
    T: Deref,
{
    type Target = <T as Deref>::Target;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
impl<T> DerefMut for AsUninit<T>
where
    T: DerefMut,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

unsafe impl<T, Item> Initialize for AsUninit<T>
where
    T: Deref<Target = [Item]> + DerefMut + TrustedDeref,
{
    type Item = Item;

    #[inline]
    fn as_maybe_uninit_slice(&self) -> &[MaybeUninit<Item>] {
        let slice: &[Item] = &*self;
        crate::cast_init_to_uninit_slice(slice)
    }
    #[inline]
    unsafe fn as_maybe_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<Item>] {
        let slice_mut: &mut [Item] = &mut *self;
        crate::cast_init_to_uninit_slice_mut(slice_mut)
    }
}
