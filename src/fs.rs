//! This module reimplements the `std::fs::read_dir` function, but using the
//! `getdents64` Linux syscall directly, without the libc wrappers.
//!
//! The iterator yields instances of [`DirEntry`], which is the equivalent for
//! `std::fs::DirEntry`, though it only provides the `path` method. The rest of
//! the methods can be implemented with the data contained in [`DirEntry`], but
//! are not necessary for this proof of concept.

use std::ffi::{CString, OsStr};
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io, slice};

use libc::{c_char, c_int, c_long, c_void, dirent64, size_t};

/// Reimplementation of the `std::fs::read_dir` function without the libc
/// wrappers.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    ReadDir::new(path.as_ref().to_path_buf())
}

/// Iterator over the entries in a directory.
pub struct ReadDir {
    dirfd: c_int,
    root: Arc<PathBuf>,
    buf: Vec<u8>,
    buf_size: usize,
    buf_offset: usize,
}

macro_rules! try_posix_fn {
    ($call:expr) => {
        loop {
            let res = $call;

            if res != -1 {
                break res;
            }

            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    };
}

impl ReadDir {
    fn new(path: PathBuf) -> io::Result<Self> {
        let dirfd = unsafe {
            let path = CString::new(path.as_os_str().as_bytes())?;
            try_posix_fn!(libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_DIRECTORY,
            ))
        };

        // Try to estimate the buffer size for `getdents64`.
        //
        // It seems that there is no way to get the exact size to make only one
        // call to `getdents64`.
        //
        // GNU libc uses the `st_blksize` field from the directory inode, and
        // clamps it between 32K and 1M. This value is too low for directories
        // with *a lot* of files.
        //
        // Here, we are using a more-complicated-yet-not-necessarily-better
        // approach:
        //
        // 1. Use the size of the inode as reference.
        // 2. Round it to the next multiple of `BUFSIZ`.
        // 3. Clamps the result between `BUFSIZ` and 1M.
        //
        // This method assumes that `BUFSIZ` is a power of 2.
        let buf_capacity = unsafe {
            let mut stat = MaybeUninit::uninit();
            try_posix_fn!(libc::fstat64(dirfd, stat.as_mut_ptr()));

            const BUFSIZ: usize = libc::BUFSIZ as usize;

            ((stat.assume_init().st_size as usize + (BUFSIZ - 1)) & !(BUFSIZ - 1))
                .clamp(BUFSIZ, 1024 * 1024)
        };

        Ok(ReadDir {
            dirfd,
            root: Arc::new(path),
            buf: vec![0; buf_capacity],
            buf_size: 0,
            buf_offset: 0,
        })
    }
}

/// Wrapper for the `getdents64` syscall, since some versions of libc does not
/// provide it.
#[inline(always)]
unsafe fn getdents64(fd: c_int, buf: *mut c_void, bytes: size_t) -> c_long {
    libc::syscall(libc::SYS_getdents64, fd, buf, bytes)
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.buf_offset >= self.buf_size {
                // Get more entries from the kernel.
                //
                // `getdents64` returns the number of bytes written to the
                // buffer, or `0` when all entries have been read.
                let buf_size = loop {
                    let res = unsafe {
                        getdents64(self.dirfd, self.buf.as_mut_ptr().cast(), self.buf.len())
                    };

                    if res == 0 {
                        return None;
                    }

                    if res > 0 {
                        break res as usize;
                    }

                    // `res` contains an error. Retry if it is `EINTR`.
                    let error = -res as i32;
                    if error != libc::EINTR {
                        return Some(Err(io::Error::from_raw_os_error(error)));
                    }
                };

                self.buf_size = buf_size;
                self.buf_offset = 0;
            }

            unsafe {
                let dirent: *const dirent64 = self.buf.as_ptr().add(self.buf_offset).cast();

                let d_reclen = (*dirent).d_reclen as usize;
                let d_ino = (*dirent).d_ino;

                self.buf_offset += d_reclen;

                // Copy the bytes of the `dirent64` record from the buffer to a
                // memory owned by `DirEntry`.
                let entry = slice::from_raw_parts(dirent.cast(), d_reclen)
                    .to_owned()
                    .into_boxed_slice();

                let dir_entry = DirEntry {
                    entry,
                    namelen: libc::strlen((*dirent).d_name.as_ptr()),
                    root: Arc::clone(&self.root),
                };

                // Skip `.`, `..`, and deleted files.
                //
                // It is unclear if `d_ino == 0` should be skipped. In 2010, an
                // [1]issue was reported in the GNU libc Bugzilla to delete this
                // condition in `readdir(3)`. The issue was discarded, but the
                // reporter mentions a case where `0` is a valid inode number.
                //
                // To keep compatibility with existing programs, we replicate
                // the GNU libc behaviour.
                //
                // [1]: https://sourceware.org/bugzilla/show_bug.cgi?id=12165

                if d_ino == 0 || matches!(dir_entry.name_bytes(), b"." | b"..") {
                    continue;
                }

                return Some(Ok(dir_entry));
            }
        }
    }
}

impl Drop for ReadDir {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.dirfd);
        }
    }
}

/// Entries returned by the [`ReadDir`] iterator.
pub struct DirEntry {
    /// The `dirent64` record.
    entry: Box<[u8]>,

    /// Size, in bytes, of the file name.
    namelen: usize,

    /// Shared reference to argument of `fs::read_dir`.
    root: Arc<PathBuf>,
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DirEntry").field(&self.path()).finish()
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.root.join(OsStr::from_bytes(self.name_bytes()))
    }

    fn name_bytes(&self) -> &[u8] {
        let dirent: *const dirent64 = Box::as_ref(&self.entry).as_ptr().cast();
        let d_name: *const c_char = unsafe { (*dirent).d_name.as_ptr() };
        unsafe { slice::from_raw_parts(d_name.cast(), self.namelen) }
    }
}
