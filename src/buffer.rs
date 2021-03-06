//! Buffers for single-buffer and vectored I/O which tracking initializedness and how much has been
//! filled. Container types pointing to possibly-uninitialized memory such as
//! `Vec<MaybeUninit<Item>>`, `IoBox<Uninitialized>`, or `Box<[MaybeUninit<Item>]>` can be
//! transformed into their initialized variants via the [`Initialize`] trait, within requiring
//! unsafe code.
//!
//! This is an implementation of an API similar to what has been proposed in [this
//! RFC](https://github.com/sfackler/rfcs/blob/read-buf/text/0000-read-buf.md).

use core::fmt;
use core::mem::MaybeUninit;

use crate::initializer::BufferInitializer;
use crate::traits::{Initialize, TrustedDeref};
use crate::wrappers::AsUninit;

pub struct Buffer<T> {
    pub(crate) initializer: BufferInitializer<T>,
    pub(crate) items_filled: usize,
}

/// A reference to a [`Buffer`], which is meant be a subset of the functionality offered by the
/// fully owned buffer.
///
/// For example, it neither allows reading from the unfilled region, nor swapping out the buffer
/// pointed to, with anything else.
pub struct BufferRef<'buffer, T> {
    // NOTE: The reference here is private, and never accessed using the API, _since we don't want
    // an API user to be able to replace a `&mut Buffer` with a completely different one_.
    inner: &'buffer mut Buffer<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct BufferParts<'a, I> {
    pub filled_part: &'a [I],
    pub unfilled_init_part: &'a [I],
    pub unfilled_uninit_part: &'a [MaybeUninit<I>],
}
#[derive(Debug)]
pub struct BufferPartsMut<'a, I> {
    pub filled_part: &'a mut [I],
    pub unfilled_init_part: &'a mut [I],
    pub unfilled_uninit_part: &'a mut [MaybeUninit<I>],
}

impl<T> Buffer<T> {
    /// Create a new buffer from an initializer.
    #[inline]
    pub const fn from_initializer(initializer: BufferInitializer<T>) -> Self {
        Self {
            initializer,
            items_filled: 0,
        }
    }
    /// Create a new buffer, defaulting to not being initialized, nor filled. Prefer
    /// [`new`](Buffer::new) if the buffer is already initialized.
    pub const fn uninit(inner: T) -> Self {
        Self::from_initializer(BufferInitializer::uninit(inner))
    }
    /// Move out the buffer initializer, which contains the inner buffer and initialization cursor,
    /// and get the filledness cursor.
    #[inline]
    pub fn into_raw_parts(self) -> (BufferInitializer<T>, usize) {
        let Self {
            initializer,
            items_filled,
        } = self;

        (initializer, items_filled)
    }
    #[inline]
    pub fn into_initializer(self) -> BufferInitializer<T> {
        self.initializer
    }
    /// Move out the inner buffer, being uninitialized or initialized based on whatever it was when
    /// this buffer was constructed.
    ///
    /// Use [`try_into_init`] if the buffer is initialized.
    ///
    /// [`try_into_init`]: #method.try_into_init
    #[inline]
    pub fn into_inner(self) -> T {
        self.into_initializer().into_inner()
    }

    /// Get the number of items that are currently filled, within the buffer. Note that this is
    /// different from the number of initialized items; use [`items_initialized`] for that.
    ///
    /// [`items_initialized`]: #method.items_initialized
    #[inline]
    pub const fn items_filled(&self) -> usize {
        self.items_filled
    }

    #[inline]
    pub fn by_ref(&mut self) -> BufferRef<'_, T> {
        BufferRef { inner: self }
    }

    #[inline]
    pub const fn initializer(&self) -> &BufferInitializer<T> {
        &self.initializer
    }

    #[inline]
    pub fn initializer_mut(&mut self) -> &mut BufferInitializer<T> {
        &mut self.initializer
    }
}
impl<T, Item> Buffer<AsUninit<T>>
where
    T: core::ops::Deref<Target = [Item]> + core::ops::DerefMut + TrustedDeref,
{
    pub fn new(init: T) -> Self {
        Self::from_initializer(BufferInitializer::new(init))
    }
}

impl<T> Buffer<T>
where
    T: Initialize,
{
    #[inline]
    pub fn capacity(&self) -> usize {
        self.initializer.capacity()
    }

    pub(crate) fn debug_assert_validity(&self) {
        self.initializer.debug_assert_validity();
        debug_assert!(self.items_filled() <= self.capacity());
        debug_assert!(self.items_filled() <= self.initializer.items_initialized());
    }
    /// Get the number of items that may be filled before the buffer is full.
    #[inline]
    pub fn remaining(&self) -> usize {
        debug_assert!(self.capacity() >= self.items_filled);
        self.capacity().wrapping_sub(self.items_filled)
    }
    /// Check whether the buffer is completely filled, and thus also initialized.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.items_filled() == self.capacity()
    }
    /// Check whether the buffer is empty. It can be partially or fully initialized however.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items_filled() == 0
    }
    /// Retrieve a shared slice to the filled part of the buffer.
    #[inline]
    pub fn filled_part(&self) -> &[T::Item] {
        unsafe {
            self.debug_assert_validity();

            let ptr = self.initializer.all_uninit().as_ptr();
            let len = self.items_filled;

            core::slice::from_raw_parts(ptr as *const T::Item, len)
        }
    }
    /// Retrieve a mutable slice to the filled part of the buffer.
    #[inline]
    pub fn filled_part_mut(&mut self) -> &mut [T::Item] {
        let orig_ptr = unsafe { self.initializer.all_uninit_mut().as_mut_ptr() };

        unsafe {
            self.debug_assert_validity();

            let ptr = orig_ptr;
            let len = self.items_filled;

            core::slice::from_raw_parts_mut(ptr as *mut T::Item, len)
        }
    }
    /// Get a shared slice to the unfilled part, which may be uninitialized.
    #[inline]
    pub fn unfilled_part(&self) -> &[MaybeUninit<T::Item>] {
        let (orig_ptr, orig_len) = {
            let orig = self.initializer.all_uninit();

            (orig.as_ptr(), orig.len())
        };

        unsafe {
            self.debug_assert_validity();

            let ptr = orig_ptr.add(self.items_filled);
            let len = orig_len.wrapping_sub(self.items_filled);

            core::slice::from_raw_parts(ptr, len)
        }
    }
    /// Get a mutable reference to the unfilled part of the buffer, which may overlap with the
    /// initialized-but-nonfilled region.
    ///
    /// # Safety
    ///
    /// Due to the possibility of an overlap between the part that is initialized and the part that
    /// is unfilled, the caller must ensure that the resulting slice is never used to deinitialize
    /// the buffer.
    ///
    /// It is thus recommended to use [`append`](Self::append) or
    /// [`fill_by_repeating`](Self::fill_by_repeating) instead, since those are the by far most
    /// common operations to do when initializing. However, code that requires interfacing with
    /// other APIs such as system calls, need to use this function.
    ///
    /// If mutable access really is needed for the unfilled region in safe code, consider using
    /// [`all_parts_mut`](Self::all_parts_mut).
    #[inline]
    pub unsafe fn unfilled_part_mut(&mut self) -> &mut [MaybeUninit<T::Item>] {
        let (orig_ptr, orig_len) = {
            let orig = self.initializer.all_uninit_mut();
            (orig.as_mut_ptr(), orig.len())
        };

        self.debug_assert_validity();

        let ptr = orig_ptr.add(self.items_filled);
        let len = orig_len.wrapping_sub(self.items_filled);

        core::slice::from_raw_parts_mut(ptr, len)
    }
    /// Borrow both the filled and unfilled parts immutably.
    #[inline]
    pub fn filled_unfilled_parts(&self) -> (&[T::Item], &[MaybeUninit<T::Item>]) {
        (self.filled_part(), self.unfilled_part())
    }
    /// Borrow the filled part, the unfilled but initialized part, and the unfilled and
    /// uninitialized part.
    #[inline]
    pub fn all_parts(&self) -> BufferParts<'_, T::Item> {
        BufferParts {
            filled_part: self.filled_part(),
            unfilled_init_part: self.unfilled_init_part(),
            unfilled_uninit_part: self.unfilled_uninit_part(),
        }
    }

    #[inline]
    pub fn all_parts_mut(&mut self) -> BufferPartsMut<'_, T::Item> {
        let (all_ptr, all_len) = unsafe {
            let all = self.initializer.all_uninit_mut();

            (all.as_mut_ptr(), all.len())
        };

        unsafe {
            self.debug_assert_validity();

            let filled_base_ptr = all_ptr as *mut T::Item;
            let filled_len = self.items_filled;

            let unfilled_init_base_ptr = all_ptr.add(self.items_filled) as *mut T::Item;
            let unfilled_init_len = self
                .initializer
                .items_initialized()
                .wrapping_sub(self.items_filled);

            let unfilled_uninit_base_ptr = all_ptr.add(self.initializer.items_initialized());
            let unfilled_uninit_len = all_len.wrapping_sub(self.initializer.items_initialized());

            let filled_part = core::slice::from_raw_parts_mut(filled_base_ptr, filled_len);
            let unfilled_init_part =
                core::slice::from_raw_parts_mut(unfilled_init_base_ptr, unfilled_init_len);
            let unfilled_uninit_part =
                core::slice::from_raw_parts_mut(unfilled_uninit_base_ptr, unfilled_uninit_len);

            BufferPartsMut {
                filled_part,
                unfilled_init_part,
                unfilled_uninit_part,
            }
        }
    }

    /// Borrow both the filled and the unfilled parts, mutably.
    ///
    /// # Safety
    ///
    /// This is unsafe as the uninit part may have items in it that are tracked to be initialized.
    /// It is hence the responsibility of the caller to ensure that the buffer is not deinitialized
    /// by writing [`MaybeUninit::uninit()`] to it.
    #[inline]
    pub unsafe fn filled_unfilled_parts_mut(
        &mut self,
    ) -> (&mut [T::Item], &mut [MaybeUninit<T::Item>]) {
        let (all_ptr, all_len) = {
            let all = self.initializer.all_uninit_mut();

            (all.as_mut_ptr(), all.len())
        };

        {
            self.debug_assert_validity();

            let filled_base_ptr = all_ptr as *mut T::Item;
            let filled_len = self.items_filled;

            let unfilled_base_ptr = all_ptr.add(self.items_filled);
            let unfilled_len = all_len.wrapping_sub(self.items_filled);

            let filled = core::slice::from_raw_parts_mut(filled_base_ptr, filled_len);
            let unfilled = core::slice::from_raw_parts_mut(unfilled_base_ptr, unfilled_len);

            (filled, unfilled)
        }
    }

    #[inline]
    pub fn unfilled_init_part(&self) -> &[T::Item] {
        unsafe {
            self.debug_assert_validity();

            let all = self.initializer.all_uninit();
            let all_ptr = all.as_ptr();

            let unfilled_init_base_ptr = all_ptr.add(self.items_filled) as *const T::Item;
            let unfilled_init_len = self
                .initializer
                .items_initialized()
                .wrapping_sub(self.items_filled);

            core::slice::from_raw_parts(unfilled_init_base_ptr, unfilled_init_len)
        }
    }

    /// Get the initialized part of the unfilled part, if there is any.
    #[inline]
    pub fn unfilled_init_part_mut(&mut self) -> &mut [T::Item] {
        let BufferPartsMut {
            unfilled_init_part, ..
        } = self.all_parts_mut();

        unfilled_init_part
    }
    #[inline]
    pub fn unfilled_uninit_part(&self) -> &[MaybeUninit<T::Item>] {
        self.initializer.uninit_part()
    }
    /// Get the uninitialized part of the unfilled part, if there is any.
    #[inline]
    pub fn unfilled_uninit_part_mut(&mut self) -> &mut [MaybeUninit<T::Item>] {
        self.initializer.uninit_part_mut()
    }

    #[inline]
    pub fn unfilled_parts(&mut self) -> (&[T::Item], &[MaybeUninit<T::Item>]) {
        let BufferParts {
            unfilled_init_part,
            unfilled_uninit_part,
            ..
        } = self.all_parts();

        (unfilled_init_part, unfilled_uninit_part)
    }
    #[inline]
    pub fn unfilled_parts_mut(&mut self) -> (&mut [T::Item], &mut [MaybeUninit<T::Item>]) {
        let BufferPartsMut {
            unfilled_init_part,
            unfilled_uninit_part,
            ..
        } = self.all_parts_mut();

        (unfilled_init_part, unfilled_uninit_part)
    }

    /// Revert the internal cursor to 0, forgetting about the initialized items.
    #[inline]
    pub fn revert_to_start(&mut self) {
        self.by_ref().revert_to_start()
    }
    #[inline]
    pub fn append(&mut self, slice: &[T::Item])
    where
        T::Item: Copy,
    {
        unsafe {
            // TODO: If this would ever turn out to be worth it, this could be optimized as bounds
            // checking gets redundant here.
            let unfilled_part = self.unfilled_part_mut();
            assert!(slice.len() <= unfilled_part.len());
            unfilled_part[..slice.len()].copy_from_slice(crate::cast_init_to_uninit_slice(slice));

            self.assume_init(slice.len())
        }
    }
    #[inline]
    pub fn advance(&mut self, count: usize) {
        assert!(
            self.initializer
                .items_initialized()
                .wrapping_sub(self.items_filled)
                >= count,
            "advancing filledness cursor beyond the initialized region ({} + {} = {} filled > {} init)",
            self.items_filled,
            count,
            self.items_filled + count,
            self.initializer.items_initialized,
        );
        self.items_filled = self.items_filled.wrapping_add(count);
    }
    #[inline]
    pub fn advance_to_init_part(&mut self) {
        self.items_filled = self.initializer.items_initialized;
    }
    // TODO: Method for increasing the items filled, but not the items initialized?
    /// Increment the counter that marks the progress of filling, as well as the initialization
    /// progress, `count` items.
    ///
    /// # Safety
    ///
    /// This does not initialize nor fill anything, and it is hence up to the user to ensure that
    /// no uninitialized items are marked initialized.
    pub unsafe fn assume_init(&mut self, count: usize) {
        self.items_filled += count;
        self.initializer.items_initialized =
            core::cmp::max(self.items_filled, self.initializer.items_initialized);

        self.debug_assert_validity();
    }
    /// Mark the buffer as fully filled and initialized, without actually filling the buffer.
    ///
    /// # Safety
    ///
    /// For this to be safe, the caller must ensure that every single item in the slice that the
    /// buffer wraps, is initialized.
    #[inline]
    pub unsafe fn assume_init_all(&mut self) {
        self.items_filled = self.capacity();
        self.initializer.items_initialized = self.capacity();
    }
    #[inline]
    pub fn fill_by_repeating(&mut self, item: T::Item)
    where
        T::Item: Copy,
    {
        unsafe {
            crate::fill_uninit_slice(self.unfilled_part_mut(), item);
            self.assume_init_all();
        }
    }
}
impl<T> Buffer<T>
where
    T: Initialize<Item = u8>,
{
    #[inline]
    pub fn fill_by_zeroing(&mut self) {
        self.fill_by_repeating(0_u8);
    }
}
impl<'a> Buffer<AsUninit<&'a mut [u8]>> {
    // TODO: Use a trait that makes the dynamic counter statically set to full.
    #[inline]
    pub fn from_slice_mut(slice: &'a mut [u8]) -> Self {
        let mut initializer = BufferInitializer::new(slice);
        unsafe {
            initializer.advance_to_end();
        }
        Self::from_initializer(initializer)
    }
}
impl<'a> Buffer<&'a mut [MaybeUninit<u8>]> {
    #[inline]
    pub fn from_uninit_slice_mut(slice: &'a mut [MaybeUninit<u8>]) -> Self {
        Self::uninit(slice)
    }
}

impl<'buffer, T> BufferRef<'buffer, T> {
    #[inline]
    pub fn items_filled(&self) -> usize {
        self.inner.items_filled()
    }

    /// Reborrow the inner buffer, getting a buffer reference with a shorter lifetime.
    #[inline]
    pub fn by_ref(&mut self) -> BufferRef<'_, T> {
        BufferRef { inner: self.inner }
    }
}

impl<'buffer, T> BufferRef<'buffer, T>
where
    T: Initialize,
{
    #[inline]
    pub fn remaining(&self) -> usize {
        self.inner.remaining()
    }
    #[inline]
    pub fn unfilled_parts(&mut self) -> (&mut [T::Item], &mut [MaybeUninit<T::Item>]) {
        self.inner.unfilled_parts_mut()
    }
    /// Get a mutable and possibly-uninitialized reference to all of the buffer.
    ///
    /// # Safety
    ///
    /// The caller must not allow safe code to de-initialize the resulting slice.
    #[inline]
    pub unsafe fn unfilled_mut(&mut self) -> &mut [MaybeUninit<T::Item>] {
        self.inner.unfilled_part_mut()
    }
    /// Advance the counter of the number of items filled.
    ///
    /// The number of items that are initialized is also updated accordingly, so that the number of
    /// items initialized is always greater than or equal to the number of items filled.
    ///
    /// # Safety
    ///
    /// The caller must uphold the initialization invariant.
    #[inline]
    pub unsafe fn advance(&mut self, count: usize) {
        self.inner.assume_init(count)
    }
    /// Advance the counter of the number of items filled, and the number of items initialized, to
    /// the end of the buffer.
    ///
    /// # Safety
    ///
    /// The caller must uphold the initialization invariant.
    #[inline]
    pub unsafe fn advance_all(&mut self) {
        self.inner.assume_init_all();
    }
    #[inline]
    pub fn revert_to_start(&mut self) {
        self.inner.revert_to_start()
    }
    #[inline]
    pub fn fill_by_repeating(&mut self, item: T::Item)
    where
        T::Item: Copy,
    {
        self.inner.fill_by_repeating(item)
    }
    #[inline]
    pub fn append(&mut self, slice: &[T::Item])
    where
        T::Item: Copy,
    {
        self.inner.append(slice)
    }
}
impl<T> BufferRef<'_, T>
where
    T: Initialize<Item = u8>,
{
    #[inline]
    pub fn fill_by_zeroing(&mut self) {
        self.inner.fill_by_zeroing()
    }
}

impl<T> fmt::Debug for Buffer<T>
where
    T: Initialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.initializer().all_uninit().as_ptr();
        let items_init = self.initializer().items_initialized();
        let items_filled = self.items_filled();
        let total = self.capacity();

        if f.alternate() {
            let init_percentage = items_init as f64 / total as f64 * 100.0;
            let filled_percentage = items_filled as f64 / total as f64 * 100.0;
            write!(
                f,
                "[buffer at {:?}, {} B filled ({:.1}%), {} B init ({:.1}%), {} B total]",
                ptr, items_filled, filled_percentage, items_init, init_percentage, total
            )
        } else {
            write!(
                f,
                "[buffer at {:?}, {}/{}/{}]",
                ptr, items_filled, items_init, total
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_buffer_ops() {
        let mut buffer = Buffer::uninit([MaybeUninit::<u8>::uninit(); 32]);
        assert_eq!(buffer.capacity(), 32);
        assert_eq!(buffer.remaining(), 32);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        assert!(buffer.initializer().is_completely_uninit());
        assert!(!buffer.initializer().is_completely_init());
        assert_eq!(buffer.initializer().items_initialized(), 0);
        assert_eq!(buffer.initializer().remaining(), 32);
        assert_eq!(buffer.filled_part(), &[]);
        assert_eq!(buffer.unfilled_part().len(), 32);
        assert_eq!(buffer.filled_part_mut(), &mut []);
        assert_eq!(unsafe { buffer.unfilled_part_mut().len() }, 32);
        // TODO: more

        let BufferParts {
            filled_part,
            unfilled_init_part,
            unfilled_uninit_part,
        } = buffer.all_parts();
        assert_eq!(filled_part, &[]);
        assert_eq!(unfilled_init_part, &[]);
        assert_eq!(unfilled_uninit_part.len(), 32);

        let BufferPartsMut {
            filled_part,
            unfilled_init_part,
            unfilled_uninit_part,
        } = buffer.all_parts_mut();
        assert_eq!(filled_part, &mut []);
        assert_eq!(unfilled_init_part, &mut []);
        assert_eq!(unfilled_uninit_part.len(), 32);

        let src = b"I am a really nice slice!";
        let modified = b"I am a really wise slice!";

        buffer.append(src);

        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());
        assert_eq!(buffer.filled_part(), src);
        assert_eq!(buffer.filled_part_mut(), src);
        assert!(!buffer.initializer().is_completely_init());
        assert!(!buffer.initializer().is_completely_uninit());
        assert_eq!(buffer.initializer().init_part(), src);
        assert_eq!(buffer.initializer_mut().init_part_mut(), src);
        assert_eq!(buffer.items_filled(), src.len());
        assert_eq!(buffer.remaining(), 32 - src.len());

        {
            let slice_mut = buffer.filled_part_mut();
            slice_mut[14] = b'w';
            slice_mut[16] = b's';
        }

        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());
        assert_eq!(buffer.filled_part(), modified);
        assert_eq!(buffer.filled_part_mut(), modified);
        assert_eq!(buffer.initializer().init_part(), modified);
        assert_eq!(buffer.initializer_mut().init_part_mut(), modified);
        assert_eq!(buffer.items_filled(), src.len());
        assert_eq!(buffer.remaining(), 32 - src.len());
        assert_eq!(buffer.unfilled_part().len(), 32 - src.len());
        assert_eq!(unsafe { buffer.unfilled_part_mut().len() }, 32 - src.len());

        let mut initializer = buffer.into_initializer();

        assert_eq!(initializer.items_initialized(), modified.len());
        assert_eq!(initializer.remaining(), 7);
        initializer.partially_fill_uninit_part(3_usize, 0xFF_u8);
        initializer.partially_zero_uninit_part(1_usize);
        assert_eq!(initializer.remaining(), 3);

        let modified_and_garbage_items = b"I am a really wise slice!\xFF\xFF\xFF\x00";
        let (init, uninit) = initializer.init_uninit_parts();
        assert_eq!(init, modified_and_garbage_items);
        assert_eq!(uninit.len(), 3);

        let (init, uninit) = initializer.init_uninit_parts_mut();
        assert_eq!(init, modified_and_garbage_items);
        init[2] = b'e';
        assert_eq!(uninit.len(), 3);

        let mut buffer = Buffer::from_initializer(initializer);
        buffer.advance(modified.len());

        let BufferParts {
            filled_part,
            unfilled_init_part,
            unfilled_uninit_part,
        } = buffer.all_parts();
        let modified_again = b"I em a really wise slice!";
        assert_eq!(filled_part, modified_again);
        assert_eq!(unfilled_init_part, b"\xFF\xFF\xFF\x00");
        assert_eq!(unfilled_uninit_part.len(), 3);

        let BufferPartsMut {
            filled_part,
            unfilled_init_part,
            unfilled_uninit_part,
        } = buffer.all_parts_mut();
        assert_eq!(filled_part, modified_again);
        assert_eq!(unfilled_init_part, b"\xFF\xFF\xFF\x00");
        unfilled_init_part[2] = b'\x13';
        unfilled_init_part[3] = b'\x37';
        assert_eq!(unfilled_init_part, b"\xFF\xFF\x13\x37");
        assert_eq!(unfilled_uninit_part.len(), 3);

        let rest = b" Right?";
        buffer.append(rest);

        assert_eq!(buffer.items_filled(), 32);
        assert!(!buffer.is_empty());
        assert!(buffer.is_full());
        assert_eq!(buffer.remaining(), 0);

        let total = b"I em a really wise slice! Right?";
        assert_eq!(buffer.filled_part(), total);
        assert_eq!(buffer.filled_part_mut(), total);
        assert_eq!(buffer.initializer().remaining(), 0);
        assert_eq!(buffer.initializer().items_initialized(), 32);
        assert!(buffer.initializer().is_completely_init());
        assert!(!buffer.initializer().is_completely_uninit());

        buffer.advance_to_init_part();

        // TODO: Shorthand?
        let initialized: [u8; 32] = buffer.into_initializer().try_into_init().unwrap().into();
        assert_eq!(&initialized, total);
    }
    #[test]
    fn debug_impl() {
        let array = [MaybeUninit::<u8>::uninit(); 32];
        let mut buffer = Buffer::uninit(array);
        buffer.append(b"Hello, world!");
        buffer.initializer_mut().partially_zero_uninit_part(13);

        assert_eq!(
            format!("{:?}", buffer),
            format!(
                "[buffer at {:p}, 13/26/32]",
                buffer.initializer().all_uninit().as_ptr()
            )
        );
        assert_eq!(
            format!("{:#?}", buffer),
            format!(
                "[buffer at {:p}, 13 B filled (40.6%), 26 B init (81.2%), 32 B total]",
                buffer.initializer().all_uninit().as_ptr()
            )
        );
    }
}
