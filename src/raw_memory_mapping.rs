use crate::OpenOptions;
use std::{os::unix::io::RawFd, ptr::NonNull};

/// Return the currently configured page size
/// by calling [`libc::sysconf`]
pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) as usize }
}

pub struct RawMemoryMapping {
    ptr: NonNull<()>,
    byte_size: usize,
    byte_offset: usize,
}

impl RawMemoryMapping {
    pub fn open<T: ?Sized>(fd: RawFd, open_options: &OpenOptions<T>) -> std::io::Result<RawMemoryMapping> {
        let offset_delta = open_options.byte_offset % page_size();
        let mapping_size = open_options.byte_len + offset_delta;

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                mapping_size,
                open_options.get_mmap_protection(),
                open_options.get_mmap_flags(),
                fd,
                (open_options.byte_offset - offset_delta) as libc::off_t,
            )
        };

        if ptr == libc::MAP_FAILED {
            let errno = unsafe { *libc::__errno_location() };
            return Err(std::io::Error::from_raw_os_error(errno));
        }

        Ok(RawMemoryMapping {
            ptr: unsafe { NonNull::new_unchecked(ptr as *mut ()) },
            byte_size: mapping_size,
            byte_offset: offset_delta,
        })
    }

    pub fn close(&self) {
        let res = unsafe { libc::munmap(self.ptr.as_ptr() as *mut libc::c_void, self.byte_size) };

        assert_eq!(
            res,
            0,
            "munmap failed: {:?}",
            std::io::Error::from_raw_os_error(unsafe { *libc::__errno_location() })
        );
    }

    pub fn segment_ptr(&self) -> NonNull<()> {
        unsafe { NonNull::new_unchecked(self.ptr.as_ptr().byte_add(self.byte_offset)) }
    }

    pub fn segment_byte_len(&self) -> usize {
        self.byte_size - self.byte_offset
    }

    pub unsafe fn byte_resize(&mut self, new_byte_size: usize) -> std::io::Result<()> {
        let new_ptr = libc::mremap(
            self.ptr.as_ptr() as *mut libc::c_void,
            self.byte_size,
            new_byte_size,
            libc::MREMAP_MAYMOVE,
        );

        if new_ptr == libc::MAP_FAILED {
            return Err(std::io::Error::from_raw_os_error(*libc::__errno_location()));
        }

        self.ptr = NonNull::new_unchecked(new_ptr as *mut ());
        self.byte_size = new_byte_size;

        Ok(())
    }
}
