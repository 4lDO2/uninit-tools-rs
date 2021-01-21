use core::mem::MaybeUninit;

pub mod buffer;
pub mod buffers;
pub mod initializer;
pub mod traits;
pub mod wrappers;

#[cfg(feature = "alloc")]
extern crate alloc;

#[inline]
unsafe fn cast_slice_same_layout<A, B>(a: &[A]) -> &[B] {
    core::slice::from_raw_parts(a.as_ptr() as *const B, a.len())
}
#[inline]
unsafe fn cast_slice_same_layout_mut<A, B>(a: &mut [A]) -> &mut [B] {
    core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut B, a.len())
}

/// Cast a slice of bytes into a slice of uninitialized bytes, pretending that it is uninitialized.
/// This is completely safe, since `MaybeUninit` must have the exact same (direct) layout, like
/// `u8` has. The downside with this is that the information about initializedness is lost; unless
/// relying on unsafe code, the resulting slice can only be used to prove validity of the memory
/// range.
#[inline]
pub fn cast_init_to_uninit_slice<U>(init: &[U]) -> &[MaybeUninit<U>] {
    unsafe { cast_slice_same_layout(init) }
}
/// Cast a possibly uninitialized slice of bytes, into an initializied slice, assuming that it is
/// initialized.
///
/// # Safety
///
/// The initialization variant must be upheld; that is, the caller must ensure that the buffer
/// cannot contain any uninitialized data.
#[inline]
pub unsafe fn cast_uninit_to_init_slice<U>(uninit: &[MaybeUninit<U>]) -> &[U] {
    cast_slice_same_layout(uninit)
}

/// Cast a mutable slice of bytes into a slice of uninitialized bytes, pretending that it is
/// uninitialized. This is completely safe since they always have the same memory layout; however,
/// the layout of the slices themselves must not be relied upon. The initializedness information is
/// lost as part of this cast, but can be recovered when initializing again or by using unsafe
/// code.
///
/// # Safety
///
/// This is unsafe, since it allows a slice which is borrowed for a lifetime possibly shorter than
/// `'static`, to be reused after the `MaybeUninit` slice has had `MaybeUninit::uninit()` values
/// written to it. For this to be safe, the caller must only write initialized bytes to the
/// returned slice.
///
/// This function is only meant to be used in generic contexts, unlike
/// [`cast_init_to_uninit_slice`], which is used more often when copying initialized bytes to
/// uninitialized bytes.
#[inline]
pub unsafe fn cast_init_to_uninit_slice_mut<U>(init: &mut [U]) -> &mut [MaybeUninit<U>] {
    cast_slice_same_layout_mut(init)
}
/// Cast a mutable slice of possibly initialized bytes into a slice of initialized bytes, assuming
/// it is initialized.
///
/// # Safety
///
/// For this to be safe, the initialization invariant must be upheld, exactly like when reading.
///
/// __NOTE: This must not be used for initializing the buffer__. For that, there are are other safe
/// methods like [`InitializeExt::init_by_filling`] and [`InitializeExt::init_by_copying`]. If
/// unsafe code is still somehow, always initialize this by copying from _another_ MaybeUninit
/// slice, or using [`std::ptr::copy`] or [`std::ptr::copy_nonoverlapping`].
#[inline]
pub unsafe fn cast_uninit_to_init_slice_mut<U>(uninit: &mut [MaybeUninit<U>]) -> &mut [U] {
    cast_slice_same_layout_mut(uninit)
}

/// Fill a possibly uninitialized mutable slice of bytes, with the same `byte`, returning the
/// initialized slice.
#[inline]
pub fn fill_uninit_slice<U: Copy>(slice: &mut [MaybeUninit<U>], byte: U) -> &mut [U] {
    unsafe {
        // NOTE: This is solely to allow for any improved optimizations nightly may offer; we all
        // know that memset most likely is faster (and cleaner) than a loop.
        #[cfg(feature = "nightly")]
        {
            slice.fill(MaybeUninit::new(byte));
        }

        #[cfg(not(feature = "nightly"))]
        for slice_byte in slice.iter_mut() {
            *slice_byte = MaybeUninit::new(byte);
        }

        cast_uninit_to_init_slice_mut(slice)
    }
}
