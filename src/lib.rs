#![feature(pointer_byte_offsets)]

mod open_options;

pub use open_options::OpenOptions;

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    os::unix::io::AsRawFd,
    path::Path,
    ptr::NonNull,
};

pub struct MemoryMapped<T: ?Sized> {
    ptr: NonNull<()>,
    size: usize,
    offset: usize,
    _marker: PhantomData<T>,
}

pub struct IntoIter<T> {
    mmap: MemoryMapped<[T]>,
    cur_ix: usize,
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

impl<T: ?Sized> Drop for MemoryMapped<T> {
    fn drop(&mut self) {
        let base = unsafe { self.ptr.as_ptr().byte_sub(self.offset) };
        let mapping_size = self.size + self.offset;

        let res = unsafe { libc::munmap(base as *mut libc::c_void, mapping_size) };

        assert_eq!(
            res,
            0,
            "munmap failed: {:?}",
            std::io::Error::from_raw_os_error(unsafe { *libc::__errno_location() })
        );
    }
}

impl<T: ?Sized> MemoryMapped<T> {
    fn open_<P: AsRef<Path>>(path: P, open_options: &OpenOptions<T>) -> std::io::Result<Self> {
        let f = open_options.get_fs_open_options().open(path)?;

        let size = if open_options.byte_len == 0 {
            f.metadata()?.len() as usize - open_options.byte_offset
        } else {
            open_options.byte_len
        };

        let offset_delta = open_options.byte_offset % unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) } as usize;
        let mapping_size = size + offset_delta;

        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                mapping_size,
                open_options.get_mmap_protection(),
                open_options.get_mmap_flags(),
                f.as_raw_fd(),
                (open_options.byte_offset - offset_delta) as libc::off_t,
            )
        };

        if base == libc::MAP_FAILED {
            let errno = unsafe { *libc::__errno_location() };
            return Err(std::io::Error::from_raw_os_error(errno));
        }

        let ptr = unsafe { (base as *mut ()).byte_add(offset_delta) };

        Ok(Self {
            ptr: NonNull::new(ptr).expect("mmap ptr to be non null"),
            offset: offset_delta,
            size,
            _marker: PhantomData,
        })
    }
}

impl<T: ?Sized> MemoryMapped<T> {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        OpenOptions::new().read(true).open(path)
    }

    pub fn create<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        OpenOptions::new().read(true).write(true).create_new(true).open(path)
    }

    pub fn options() -> OpenOptions<T> {
        OpenOptions::new()
    }

    pub fn segment_len(&self) -> usize {
        self.size
    }
}

impl<T> MemoryMapped<[T]> {
    pub fn as_slice(&self) -> &[T] {
        unsafe { &*std::ptr::slice_from_raw_parts(self.ptr.as_ptr() as *const T, self.size / std::mem::size_of::<T>()) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe {
            &mut *std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr() as *mut T, self.size / std::mem::size_of::<T>())
        }
    }
}

impl<T> Deref for MemoryMapped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.cast().as_ref() }
    }
}

impl<T> DerefMut for MemoryMapped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.cast().as_mut() }
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
