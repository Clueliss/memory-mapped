#![feature(pointer_byte_offsets)]

mod open_options;
mod raw_memory_mapping;

pub use open_options::OpenOptions;
pub use raw_memory_mapping::page_size;
use raw_memory_mapping::RawMemoryMapping;

use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    path::Path,
};

/// A memory-mapped (sized) object or (unsized) slice
///
/// # Example
/// ```rust
/// use std::mem::MaybeUninit;
/// use memory_mapped::MemoryMapped;
///
/// struct S {
///     x: u32,
///     y: u32,
/// }
///
/// let mapped: MemoryMapped<MaybeUninit<S>> = MemoryMapped::open("my_S_object.bin").unwrap();
/// let mapped = unsafe { mapped.assume_init() };
///
/// println!("{} {}", mapped.x, mapped.y);
/// ```
pub struct MemoryMapped<T: ?Sized> {
    mapping: RawMemoryMapping,
    _marker: PhantomData<T>,
}

/// Iterator over a memory-mapped slice
pub struct IntoIter<T> {
    mmap: MemoryMapped<[T]>,
    cur_ix: usize,
}

unsafe impl<T: ?Sized> Sync for MemoryMapped<T> {}
unsafe impl<T: ?Sized> Send for MemoryMapped<T> {}

impl<T: ?Sized> Drop for MemoryMapped<T> {
    fn drop(&mut self) {
        self.mapping.close();
    }
}

impl<T> From<RawMemoryMapping> for MemoryMapped<MaybeUninit<T>> {
    fn from(mapping: RawMemoryMapping) -> Self {
        Self { mapping, _marker: PhantomData }
    }
}

impl<T> From<RawMemoryMapping> for MemoryMapped<[MaybeUninit<T>]> {
    fn from(mapping: RawMemoryMapping) -> Self {
        Self { mapping, _marker: PhantomData }
    }
}

impl<T: ?Sized> MemoryMapped<T> {
    /// Returns a new [`OptionOptions`] object to allow more fine grained control over the mapping
    pub fn options() -> OpenOptions<T> {
        OpenOptions::new()
    }

    pub fn segment_byte_len(&self) -> usize {
        self.mapping.segment_byte_len()
    }
}

impl<T> MemoryMapped<MaybeUninit<T>> {
    pub unsafe fn assume_init(self) -> MemoryMapped<T> {
        std::mem::transmute(self)
    }
}

impl<T> MemoryMapped<[MaybeUninit<T>]> {
    pub unsafe fn assume_init(self) -> MemoryMapped<[T]> {
        std::mem::transmute(self)
    }
}

impl<T> MemoryMapped<T> {
    /// Attempts to memory map a file with [`libc::mmap`] as a read-only, private mapping
    ///
    /// # Safety
    /// begins the object lifetime of a `T`, the caller must ensure that
    /// the created value is properly initialized
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        OpenOptions::<T>::new().read(true).open(path)
    }

    /// Attempts to memory map a file with [`libc::mmap`] as a read-write, private mapping
    /// This function will create a file if it does not exist and truncate it if it does.
    ///
    /// # Safety
    /// begins the object lifetime of a `T`, the caller must ensure that
    /// the created value is properly initialized
    pub fn create<P: AsRef<Path>>(path: P) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        OpenOptions::<T>::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
    }
}

impl<T> MemoryMapped<[T]> {
    /// Attempts to memory map a file with [`libc::mmap`] as a read-only, private mapping
    ///
    /// # Safety
    /// begins the object lifetime of a `T`, the caller must ensure that
    /// the created value is properly initialized
    pub fn open_slice<P: AsRef<Path>>(path: P) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        OpenOptions::<[T]>::new().read(true).open_slice(path)
    }

    /// Attempts to memory map a file with [`libc::mmap`] as a read-write, private mapping
    /// This function will create a file if it does not exist and truncate it if it does.
    ///
    /// # Safety
    /// begins the object lifetime of a `T`, the caller must ensure that
    /// the created value is properly initialized
    pub fn create_slice<P: AsRef<Path>>(path: P) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        OpenOptions::<[T]>::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open_slice(path)
    }
}

impl<T> MemoryMapped<[T]> {
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            &*std::ptr::slice_from_raw_parts(
                self.mapping.segment_ptr().as_ptr() as *mut T,
                self.mapping.segment_byte_len() / std::mem::size_of::<T>(),
            )
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe {
            &mut *std::ptr::slice_from_raw_parts_mut(
                self.mapping.segment_ptr().as_ptr() as *mut T,
                self.mapping.segment_byte_len() / std::mem::size_of::<T>(),
            )
        }
    }
}

impl<T> MemoryMapped<[MaybeUninit<T>]> {
    /// resizes `self` to `new_len` elements by calling [`libc::mremap`]
    ///
    /// # Safety
    /// if new_len > old_len the caller must ensure that the underlying file is large enough to support the size increase
    pub unsafe fn resize_uninit(&mut self, new_len: usize) -> std::io::Result<()> {
        let new_byte_size = new_len * std::mem::size_of::<T>();
        self.mapping.byte_resize(new_byte_size)
    }
}

impl<T> MemoryMapped<[T]> {
    /// resizes `self` to `new_len` elements by calling [`libc::mremap`] without initializing the new elements
    /// in case of a length increase
    ///
    /// # Safety
    /// - if new_len > old_len the caller must ensure that the underlying file is large enough to support the size increase
    /// - the additional memory will not be initialized by this function but is assumed to be correctly initialized
    pub unsafe fn resize_assume_init(&mut self, new_len: usize) -> std::io::Result<()> {
        let uninit_self: &mut MemoryMapped<[MaybeUninit<T>]> = std::mem::transmute(self);
        uninit_self.resize_uninit(new_len)
    }

    /// resizes `self` to `new_len` by calling [`libc::mremap`] overwriting the new elements by repeatedly calling `f`
    /// in case of a length increase
    ///
    /// # Safety
    /// if new_len > old_len the caller must ensure that the underlying file is large enough to support the size increase
    pub unsafe fn resize_with<F>(&mut self, new_len: usize, mut f: F) -> std::io::Result<()>
    where
        F: FnMut() -> T,
    {
        let old_len = self.len();

        let uninit_self: &mut MemoryMapped<[MaybeUninit<T>]> = std::mem::transmute(self);
        uninit_self.resize_uninit(new_len)?;

        if new_len > old_len {
            let uninit_elems = &mut uninit_self.as_slice_mut()[old_len..];
            uninit_elems.fill_with(move || MaybeUninit::new(f()));
        }

        Ok(())
    }

    /// Shrinks the capacity of this mapping to `new_len` by calling [`libc::mremap`]
    pub fn shrink_to(&mut self, new_len: usize) -> std::io::Result<()> {
        assert!(self.len() > new_len);

        // SAFETY: size will shrink so no new possbily uninitialized memory can be produced by this call
        // additionally the file already is known to have a size at least as large as this mapping
        unsafe { self.resize_assume_init(new_len) }
    }
}

impl<T: Copy> MemoryMapped<[T]> {
    /// resizes `self` to `new_len` by calling [`libc::mremap`] overwriting the new elements with `fill`
    /// in case of a length increase
    ///
    /// # Safety
    /// if new_len > old_len the caller must ensure that the underlying file is large enough to support the size increase
    pub unsafe fn resize(&mut self, new_len: usize, fill: T) -> std::io::Result<()> {
        self.resize_with(new_len, move || fill)
    }
}

impl<T> Deref for MemoryMapped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.mapping.segment_ptr().cast().as_ref() }
    }
}

impl<T> DerefMut for MemoryMapped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.mapping.segment_ptr().cast().as_mut() }
    }
}

impl<T> Deref for MemoryMapped<[T]> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for MemoryMapped<[T]> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl<T: Copy> IntoIterator for MemoryMapped<[T]> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { mmap: self, cur_ix: 0 }
    }
}

impl<T: Copy> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_ix < self.mmap.len() {
            let old_ix = self.cur_ix;
            self.cur_ix += 1;

            Some(self.mmap[old_ix])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{page_size, MemoryMapped};
    use std::fs::File;

    #[test]
    fn test_shared_resize() {
        let page_size = page_size();
        let ints_per_page = page_size / std::mem::size_of::<u32>();

        let f = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open("test.bin")
            .unwrap();

        f.set_len((page_size * 2) as u64).unwrap();

        let mut m1: MemoryMapped<[u32]> = unsafe {
            MemoryMapped::options()
                .read(true)
                .write(true)
                .len(ints_per_page)
                .open_shared_slice_from_file(&f)
                .unwrap()
                .assume_init()
        };

        let mut m2: MemoryMapped<[u32]> = unsafe {
            MemoryMapped::options()
                .read(true)
                .write(true)
                .offset(m1.len())
                .open_shared_slice_from_file(&f)
                .unwrap()
                .assume_init()
        };

        *m1.first_mut().unwrap() = 0x11111111;
        *m1.last_mut().unwrap() = 0x22222222;

        *m2.first_mut().unwrap() = 0x33333333;
        *m2.last_mut().unwrap() = 0x44444444;

        f.set_len((page_size * 3) as u64).unwrap();

        let move_off = m2.len();

        unsafe {
            m2.resize_assume_init(m2.len() * 2).unwrap();
        }

        m2.copy_within(..move_off, move_off);

        m2 = unsafe {
            MemoryMapped::options()
                .read(true)
                .write(true)
                .offset(m1.len() * 2)
                .open_slice_from_file(&f)
                .unwrap()
                .assume_init()
        };

        unsafe {
            m1.resize_assume_init(m1.len() * 2).unwrap();
        }

        *m1.last_mut().unwrap() = 0x99999999;
    }
}
