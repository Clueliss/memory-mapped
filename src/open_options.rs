use std::{fs, marker::PhantomData, path::Path};

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

    pub fn open<P: AsRef<Path>>(&self, path: P) -> std::io::Result<super::MemoryMapped<T>> {
        super::MemoryMapped::open_(path, self)
    }

    /// # Safety
    /// caller must ensure that the underlying file is not mapped as shared elsewhere
    pub unsafe fn open_shared<P: AsRef<Path>>(&self, path: P) -> std::io::Result<super::MemoryMapped<T>> {
        super::MemoryMapped::open_(path, &Self { shared: true, ..*self })
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
