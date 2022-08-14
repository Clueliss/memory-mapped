use super::MemoryMapped;

use crate::RawMemoryMapping;
use std::{fs, fs::File, marker::PhantomData, mem::MaybeUninit, os::unix::io::AsRawFd, path::Path};

/// # Example
///
/// ```rust
/// use std::mem::MaybeUninit;
/// use memory_mapped::MemoryMapped;
///
/// let mapped: MemoryMapped<[MaybeUninit<u32>]> = MemoryMapped::options()
///     .read(true)
///     .write(true)
///     .byte_offset(512)
///     .byte_len(64)
///     .open_slice("some_slice.bin")
///     .unwrap();
///
/// let mapped = unsafe { mapped.assume_init() };
///
/// for x in mapped {
///     println!("{x}");
/// }
/// ```
pub struct OpenOptions<T: ?Sized> {
    read: bool,
    write: bool,
    create: bool,
    create_new: bool,

    shared: bool,

    pub(super) byte_offset: usize,
    pub(super) byte_len: usize,

    _marker: PhantomData<*const T>,
}

impl<T: ?Sized> OpenOptions<T> {
    pub(super) fn get_mmap_protection(&self) -> libc::c_int {
        use libc::{PROT_NONE, PROT_READ, PROT_WRITE};

        let mut protection = PROT_NONE;

        if self.read {
            protection |= PROT_READ;
        }

        if self.write {
            protection |= PROT_WRITE;
        }

        protection
    }

    pub(super) fn get_mmap_flags(&self) -> libc::c_int {
        use libc::{MAP_PRIVATE, MAP_SHARED};

        if self.shared {
            MAP_SHARED
        } else {
            MAP_PRIVATE
        }
    }

    pub(super) fn get_fs_open_options(&self) -> fs::OpenOptions {
        let mut opts = fs::OpenOptions::new();
        opts.read(self.read)
            .write(self.write)
            .create(self.create)
            .create_new(self.create_new);

        opts
    }

    fn with_shared(&self, shared: bool) -> Self {
        Self { shared, ..*self }
    }
}

impl<T: ?Sized> OpenOptions<T> {
    pub fn new() -> Self {
        OpenOptions {
            read: false,
            write: false,
            create: false,
            create_new: false,
            shared: false,
            byte_offset: 0,
            byte_len: 0,
            _marker: PhantomData,
        }
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    pub fn byte_offset(&mut self, byte_offset: usize) -> &mut Self {
        self.byte_offset = byte_offset;
        self
    }

    pub fn byte_len(&mut self, byte_len: usize) -> &mut Self {
        self.byte_len = byte_len;
        self
    }
}

impl<T> OpenOptions<[T]> {
    pub fn offset(&mut self, element_offset: usize) -> &mut Self {
        self.byte_offset = element_offset * std::mem::size_of::<T>();
        self
    }

    pub fn len(&mut self, len: usize) -> &mut Self {
        self.byte_len = len * std::mem::size_of::<T>();
        self
    }
}

impl<T> OpenOptions<T> {
    pub fn open<P: AsRef<Path>>(&self, path: P) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        let f = self.get_fs_open_options().open(path)?;
        self.open_from_file(&f)
    }

    pub fn open_from_file(&self, f: &File) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        let opts = Self {
            byte_len: if self.byte_len == 0 {
                f.metadata()?.len() as usize - self.byte_offset
            } else {
                self.byte_len
            },
            ..*self
        };

        Ok(RawMemoryMapping::open(f.as_raw_fd(), &opts)?.into())
    }

    pub fn open_from_fd<F: AsRawFd>(&self, f: &F) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        Ok(RawMemoryMapping::open(f.as_raw_fd(), self)?.into())
    }

    /// # Safety
    /// - caller must ensure that the the segment resulting from this call does not overlap with any other segment mapped as shared
    /// - called must ensure that the mapped memory contains a properly initialized object of type `T`
    pub unsafe fn open_shared<P: AsRef<Path>>(&self, path: P) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        self.with_shared(true).open(path)
    }

    /// # Safety
    /// see [`memory_mapped::OptionOptions::open_shared`]
    pub unsafe fn open_shared_from_file(&self, f: &File) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        self.with_shared(true).open_from_file(f)
    }

    /// # Safety
    /// see [`memory_mapped::OptionOptions::open_shared`]
    pub unsafe fn open_shared_from_fd<F: AsRawFd>(&self, fd: &F) -> std::io::Result<MemoryMapped<MaybeUninit<T>>> {
        self.with_shared(true).open_from_fd(fd)
    }
}

impl<T> OpenOptions<[T]> {
    pub fn open_slice<P: AsRef<Path>>(&self, path: P) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        let f = self.get_fs_open_options().open(path)?;
        self.open_slice_from_file(&f)
    }

    pub fn open_slice_from_file(&self, f: &File) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        let opts = Self {
            byte_len: if self.byte_len == 0 {
                f.metadata()?.len() as usize - self.byte_offset
            } else {
                self.byte_len
            },
            ..*self
        };

        Ok(RawMemoryMapping::open(f.as_raw_fd(), &opts)?.into())
    }

    pub fn open_slice_from_fd<F: AsRawFd>(&self, f: &F) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        Ok(RawMemoryMapping::open(f.as_raw_fd(), self)?.into())
    }

    /// # Safety
    /// caller must ensure that the the segment resulting from this call does not overlap with any other segment mapped as shared
    pub unsafe fn open_shared_slice<P: AsRef<Path>>(&self, path: P) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        self.with_shared(true).open_slice(path)
    }

    /// # Safety
    /// see [`memory_mapped::OptionOptions::open_shared_slice`]
    pub unsafe fn open_shared_slice_from_file(&self, f: &File) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        self.with_shared(true).open_slice_from_file(f)
    }

    /// # Safety
    /// see [`memory_mapped::OptionOptions::open_shared_slice`]
    pub unsafe fn open_shared_slice_from_fd<F: AsRawFd>(&self, fd: &F) -> std::io::Result<MemoryMapped<[MaybeUninit<T>]>> {
        self.with_shared(true).open_slice_from_fd(fd)
    }
}
