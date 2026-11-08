//! mmap-backed shared memory for local Unix display servers; no external Rust crate.
use std::{ffi::c_void, fs::{remove_file,File,OpenOptions}, io,
    os::{fd::{AsRawFd},unix::fs::OpenOptionsExt}, path::PathBuf, ptr::NonNull,
    sync::atomic::{AtomicU64,Ordering}};

unsafe extern "C" {
    fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> i32;
}
const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const MAP_SHARED: i32 = 1;
static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) struct MappedMemory {
    pub(super) file: File,
    ptr: NonNull<u8>,
    len: usize,
}
impl MappedMemory {
    pub(super) fn new(len: usize) -> io::Result<Self> {
        if len == 0 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty mapping")); }
        let file = create_file(len)?;
        let ptr = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ | PROT_WRITE,
            MAP_SHARED, file.as_raw_fd(), 0) };
        if ptr as isize == -1 { return Err(io::Error::last_os_error()); }
        let ptr = NonNull::new(ptr.cast::<u8>()).ok_or_else(io::Error::last_os_error)?;
        Ok(Self { file, ptr, len })
    }
    /// Caller must ensure no compositor/other process reads a shared buffer during mutation.
    pub(super) fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
    pub(super) fn as_mut_ptr(&mut self) -> *mut u8 { self.ptr.as_ptr() }
}
impl Drop for MappedMemory {
    fn drop(&mut self) { unsafe { munmap(self.ptr.as_ptr().cast::<c_void>(), self.len); } }
}

fn create_file(len: usize) -> io::Result<File> {
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    for _ in 0..128 {
        let n = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(".mata_win-shm-{}-{n}", std::process::id()));
        match OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(&path) {
            Ok(file) => {
                let result = file.set_len(len as u64);
                let _ = remove_file(&path);
                result?;
                return Ok(file);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "shared-memory filename exhausted"))
}
