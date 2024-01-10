use c2rust_bitfields::BitfieldStruct;
use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    hts::{
        hfile::{HFile, HFileRaw},
        HtsTPool,
    },
    kstring::KString,
    HtsError,
};

use libc::{c_char, c_int, c_uchar, c_uint, c_void, off_t, size_t, ssize_t};

pub mod bgzf_error;
use bgzf_error::BgzfError;

#[repr(C)]
struct BgzfCache {
    _unused: [u8; 0],
}

#[repr(C)]
struct BgzfMtAux {
    _unused: [u8; 0],
}

#[repr(C)]
struct BgzIdx {
    _unused: [u8; 0],
}

#[repr(C)]
struct ZStream {
    _unused: [u8; 0],
}

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct BgzfRaw {
    #[bitfield(name = "errcode", ty = "c_uint", bits = "0..=15")]
    #[bitfield(name = "reserved", ty = "c_uchar", bits = "16..=16")]
    #[bitfield(name = "is_write", ty = "c_uchar", bits = "17..=17")]
    #[bitfield(name = "no_eof_block", ty = "c_uchar", bits = "18..=18")]
    #[bitfield(name = "is_be", ty = "c_uchar", bits = "19..=19")]
    #[bitfield(name = "compress_level", ty = "c_int", bits = "20..=28")]
    #[bitfield(name = "last_block_eof", ty = "c_uchar", bits = "29..=29")]
    #[bitfield(name = "is_compressed", ty = "c_uchar", bits = "30..=30")]
    #[bitfield(name = "is_gzip", ty = "c_uchar", bits = "31..=31")]
    bfield: [u8; 4],
    cache_size: c_int,
    block_length: c_int,
    block_clength: c_int,
    block_offset: c_int,
    block_address: i64,
    uncompressed_address: i64,
    uncompressed_block: *mut c_void,
    compressed_block: *mut c_void,
    cache: *mut BgzfCache,
    fp: *mut HFileRaw,       // actual file handle
    mt: *mut BgzfMtAux,      // only used for multi-threading
    idx: *mut BgzIdx,        // BGZF index
    idx_build_otf: c_int,    // build index on the fly, set by bgzf_index_build_init()
    gz_stream: *mut ZStream, // for gzip-compressed files
    seeked: i64,             // virtual offset of last seek
}

#[link(name = "hts")]
extern "C" {
    fn bgzf_open(path: *const c_char, mode: *const c_char) -> *mut BgzfRaw;
    fn bgzf_dopen(fd: c_int, mode: *const c_char) -> *mut BgzfRaw;
    fn bgzf_hopen(fp: *mut HFileRaw, mode: *const c_char) -> *mut BgzfRaw;
    fn bgzf_close(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_read(fp: *mut BgzfRaw, data: *mut c_void, length: size_t) -> ssize_t;
    fn bgzf_write(fp: *mut BgzfRaw, data: *const c_void, length: size_t) -> ssize_t;
    fn bgzf_block_write(fp: *mut BgzfRaw, data: *const c_void, length: size_t) -> ssize_t;
    fn bgzf_peek(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_raw_read(fp: *mut BgzfRaw, data: *mut c_void, length: size_t) -> ssize_t;
    fn bgzf_raw_write(fp: *mut BgzfRaw, data: *const c_void, length: size_t) -> ssize_t;
    fn bgzf_flush(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_seek(fp: *mut BgzfRaw, pos: i64, whence: c_int) -> i64;
    fn bgzf_check_EOF(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_compression(fp: *const BgzfRaw) -> c_int;
    fn bgzf_set_cache_size(fp: *mut BgzfRaw, size: c_int);
    fn bgzf_flush_try(fp: *mut BgzfRaw, size: ssize_t) -> c_int;
    fn bgzf_getc(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_getline(fp: *mut BgzfRaw, delim: c_int, str: *mut KString) -> c_int;
    fn bgzf_read_block(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_thread_pool(fp: *mut BgzfRaw, pool: *mut HtsTPool, qsize: c_int) -> c_int;
    fn bgzf_mt(fp: *mut BgzfRaw, n_threads: c_int, _unuszed: c_int) -> c_int;
    fn bgzf_compress(
        dst: *mut c_void,
        dlen: *mut size_t,
        src: *const c_void,
        slen: size_t,
        level: c_int,
    ) -> c_int;
    fn bgzf_useek(fp: *mut BgzfRaw, uoffset: off_t, whence: c_int) -> c_int;
    fn bgzf_utell(fp: *mut BgzfRaw) -> off_t;
    fn bgzf_index_build_init(fp: *mut BgzfRaw) -> c_int;
    fn bgzf_index_load(fp: *mut BgzfRaw, bname: *const c_char, suffix: *const c_char) -> c_int;
    fn bgzf_index_load_hfile(fp: *mut BgzfRaw, idx: *mut HFileRaw, name: *const c_char) -> c_int;
    fn bgzf_index_dump(fp: *mut BgzfRaw, bname: *const c_char, suffix: *const c_char) -> c_int;
    fn bgzf_index_dump_hfile(fp: *mut BgzfRaw, idx: *mut HFileRaw, name: *const c_char) -> c_int;
}

impl BgzfRaw {
    /// Read block of characters from stream
    ///
    /// `fp` is the file stream
    ///
    /// `buffer` is where the read bytes will be written
    ///
    /// On success returns slice from buffer with read bytes.  This can be smaller than
    /// buffer if EOF is reached.
    pub fn read<'a>(&mut self, buffer: &'a mut [u8]) -> Result<&'a [u8], BgzfError> {
        let dlen = buffer.len();
        assert!(dlen > 0);
        match unsafe { bgzf_read(self, buffer.as_mut_ptr() as *mut c_void, dlen) } {
            ..=-1 => Err(BgzfError::IoError),
            0 => Err(BgzfError::EOF),
            l => Ok(&buffer[..l as usize]),
        }
    }

    /// Write byte slice to stream. If no IO errors occur, the complete slice
    /// will be written (or queued for writing).
    ///
    /// `fp` is the file stream
    ///
    /// `buffer` contains the slice to be written
    ///
    /// On success returns then number of bytes written.
    pub fn write(&mut self, buffer: &[u8]) -> Result<size_t, BgzfError> {
        let dlen = buffer.len();
        match unsafe { bgzf_write(self, buffer.as_ptr() as *const c_void, dlen) } {
            ..=-1 => Err(BgzfError::IoError),
            l => Ok(l as size_t),
        }
    }

    /// Write byte slice to stream.  The index will be used to decide the amount of
    /// uncompressed data to be written to each bgzip block. If no IO errors occur, the
    /// complete slice will be written (or queued for writing).
    ///
    /// `fp` is the file stream
    ///
    /// `buffer` contains the slice to be written
    ///
    /// On success returns then number of bytes written.
    pub fn block_write(&mut self, buffer: &[u8]) -> Result<size_t, BgzfError> {
        let dlen = buffer.len();
        match unsafe { bgzf_block_write(self, buffer.as_ptr() as *const c_void, dlen) } {
            ..=-1 => Err(BgzfError::IoError),
            l => Ok(l as size_t),
        }
    }

    /// Returns next byte from stream without consuming it
    #[inline]
    pub fn peek(&mut self) -> Result<u8, BgzfError> {
        Self::check_byte_read(unsafe { bgzf_peek(self) })
    }

    #[inline]
    fn check_byte_read(c: c_int) -> Result<u8, BgzfError> {
        match c {
            ..=-2 => Err(BgzfError::IoError),
            -1 => Err(BgzfError::EOF),
            c => {
                assert!(c < 256);
                Ok(c as u8)
            }
        }
    }
    /// Read block of characters from the underlying stream without decompression.
    /// Bypasses BGZF blocking so should only be used with care in specialized circumstances.
    ///
    /// `fp` is the file stream
    ///
    /// `buffer` is where the read bytes will be written
    ///
    /// On success returns slice from buffer with read bytes.  This can be smaller than
    /// buffer if EOF is reached.
    pub fn raw_read<'a>(&mut self, buffer: &'a mut [u8]) -> Result<&'a [u8], BgzfError> {
        let dlen = buffer.len();
        assert!(dlen > 0);
        match unsafe { bgzf_raw_read(self, buffer.as_mut_ptr() as *mut c_void, dlen) } {
            ..=-1 => Err(BgzfError::IoError),
            0 => Err(BgzfError::EOF),
            l => Ok(&buffer[..l as usize]),
        }
    }

    /// Write byte slice to stream to the underlying stream without compression.
    /// Bypasses BGZF blocking so should only be used with care in specialized circumstances.
    ///
    /// `fp` is the file stream
    ///
    /// `buffer` contains the slice to be written
    ///
    /// On success returns then number of bytes written.
    pub fn raw_write(&mut self, buffer: &[u8]) -> Result<size_t, BgzfError> {
        let dlen = buffer.len();
        match unsafe { bgzf_raw_write(self, buffer.as_ptr() as *const c_void, dlen) } {
            ..=-1 => Err(BgzfError::IoError),
            l => Ok(l as size_t),
        }
    }

    /// Write data in buffers to file.
    #[inline]
    pub fn flush(&mut self) -> Result<(), BgzfError> {
        if unsafe { bgzf_flush(self) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Return a virtual file pointer to the current location in the file.
    /// No interpretation of the value should be made, other than a subsequent
    /// call to [BgzfRaw::seek()] can be used to position the file at the same point.
    #[inline]
    pub fn tell(&self) -> Result<u64, BgzfError> {
        match (self.block_address << 16) | ((self.block_offset & 0xffff) as i64) {
            ..=-1 => Err(BgzfError::OperationFailed),
            l => Ok(l as u64),
        }
    }

    /// Set the file to read from the location specified by _pos_.
    ///
    /// `pos` is a virtual file offset returned by [BgzfRaw::tell()]
    ///
    /// It is not permitted to seek on files open for writing,
    /// or files compressed with gzip (as opposed to bgzip).
    #[inline]
    pub fn seek(&mut self, i: u64) -> Result<(), BgzfError> {
        assert!(i <= i64::MAX as u64);
        if unsafe { bgzf_seek(self, i as i64, libc::SEEK_SET) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::IoError)
        }
    }

    /// Check if the BGZF end-of-file (EOF) marker is present
    #[inline]
    pub fn check_eof(&mut self) -> Result<(), BgzfError> {
        match unsafe { bgzf_check_EOF(self) } {
            0 => Err(BgzfError::MissingEOFMarker),
            1 => Ok(()),
            2 => Err(BgzfError::CannotCheckEOF),
            _ => Err(BgzfError::UnknownError),
        }
    }

    /// Get the compression format for the file
    #[inline]
    pub fn compression(&self) -> BgzfCompression {
        match unsafe { bgzf_compression(self) } {
            0 => BgzfCompression::None,
            1 => BgzfCompression::Gzip,
            2 => BgzfCompression::Bgzip,
            _ => panic!("Unknown compression type"),
        }
    }

    /// Set the cache size. Only effective when libhts was compiled with -DBGZF_CACHE.
    #[inline]
    pub fn set_cache_size(&mut self, size: c_int) {
        unsafe { bgzf_set_cache_size(self, size) }
    }

    /// Flush the file if the remaining buffer size is smaller than `size`
    #[inline]
    pub fn flush_try(&mut self, size: ssize_t) -> Result<(), BgzfError> {
        if unsafe { bgzf_flush_try(self, size) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::IoError)
        }
    }

    /// Read one byte from file. It is faster than [BgzfRaw::read()].
    #[inline]
    pub fn getc(&mut self) -> Result<u8, BgzfError> {
        Self::check_byte_read(unsafe { bgzf_getc(self) })
    }

    /// Read one line from file. It is faster than [BgzfRaw::getc()]
    #[inline]
    pub fn get_line(&mut self, delim: c_char, s: &mut KString) -> Result<usize, BgzfError> {
        match unsafe { bgzf_getline(self, delim as c_int, s) } {
            ..=-2 => Err(BgzfError::IoError),
            -1 => Err(BgzfError::EOF),
            l => Ok(l as usize),
        }
    }

    /// Read the next BGZF block
    #[inline]
    pub fn read_block(&mut self) -> Result<(), BgzfError> {
        if unsafe { bgzf_read_block(self) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::IoError)
        }
    }

    /// Enable multi-threading via a shared thread pool.  This means
    /// both encoder and decoder can balance usage across a single pool
    /// of worker jobs.
    #[inline]
    pub fn set_thread_pool(&mut self, tpool: &mut HtsTPool, qsize: c_int) -> Result<(), BgzfError> {
        if unsafe { bgzf_thread_pool(self, tpool, qsize) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    #[inline]
    pub fn set_multi_threading(&mut self, n_threads: c_int) -> Result<(), BgzfError> {
        if unsafe { bgzf_mt(self, n_threads, 0) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Position stream at the uncompressed offset.  Requires index
    #[inline]
    pub fn useek(&mut self, x: off_t) -> Result<(), BgzfError> {
        eprintln!("{} {}", self.is_write(), self.is_gzip());
        if unsafe { bgzf_useek(self, x, libc::SEEK_SET) } == 0 {
            Ok(())
        } else {
            eprintln!("errcode = {}", self.errcode());
            Err(BgzfError::OperationFailed)
        }
    }

    /// Position in uncompressed stream
    #[inline]
    pub fn utell(&mut self) -> Result<off_t, BgzfError> {
        match unsafe { bgzf_utell(self) } {
            ..=-1 => Err(BgzfError::OperationFailed),
            l => Ok(l),
        }
    }

    /// Build index while compressing. Must be called before any data
    /// has been written or read.
    #[inline]
    pub fn index_build_init(&mut self) -> Result<(), BgzfError> {
        if unsafe { bgzf_index_build_init(self) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Load BGZF index.  If `suffix` is not [None], it is added to `bname`.
    #[inline]
    pub fn index_load(&mut self, bname: &CStr, suffix: Option<&CStr>) -> Result<(), BgzfError> {
        let sp = suffix.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
        if unsafe { bgzf_index_load(self, bname.as_ptr(), sp) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Load BGZF index from [HFile].  A filename can optionally be supplied. This is only used
    /// for error messages. If `name` is [None] then "index" will be used for error messages.
    #[inline]
    pub fn index_load_hfile(
        &mut self,
        fp: &mut HFile,
        name: Option<&CStr>,
    ) -> Result<(), BgzfError> {
        let np = name.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
        if unsafe { bgzf_index_load_hfile(self, fp.deref_mut(), np) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Write BGZF index.  If `suffix` is not [None], it is added to `bname`.
    #[inline]
    pub fn index_dump(&mut self, bname: &CStr, suffix: Option<&CStr>) -> Result<(), BgzfError> {
        let sp = suffix.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
        if unsafe { bgzf_index_dump(self, bname.as_ptr(), sp) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }

    /// Write BGZF index to [HFile].  A filename can optionally be supplied. This is only used
    /// for error messages. If `name` is [None] then "index" will be used for error messages.
    #[inline]
    pub fn index_dump_hfile(
        &mut self,
        fp: &mut HFile,
        name: Option<&CStr>,
    ) -> Result<(), BgzfError> {
        let np = name.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
        if unsafe { bgzf_index_dump_hfile(self, fp.deref_mut(), np) } == 0 {
            Ok(())
        } else {
            Err(BgzfError::OperationFailed)
        }
    }
}

/// Compress a single BGZF block in src into dst.  Returns the subslice of dst containing the compressed data.
pub fn compress<'a>(dst: &'a mut [u8], src: &[u8], level: c_int) -> Result<&'a [u8], BgzfError> {
    let mut dlen = dst.len() as size_t;
    match unsafe {
        bgzf_compress(
            dst.as_mut_ptr() as *mut c_void,
            &mut dlen,
            src.as_ptr() as *const c_void,
            src.len() as size_t,
            level,
        )
    } {
        ..=-1 => Err(BgzfError::OperationFailed),
        l => Ok(&dst[..l as usize]),
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BgzfCompression {
    None,
    Bgzip,
    Gzip,
}

pub struct Bgzf<'a> {
    inner: *mut BgzfRaw,
    phantom: PhantomData<&'a BgzfRaw>,
}

impl<'a> Deref for Bgzf<'a> {
    type Target = BgzfRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl<'a> DerefMut for Bgzf<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

unsafe impl<'a> Send for Bgzf<'a> {}
unsafe impl<'a> Sync for Bgzf<'a> {}

impl<'a> Drop for Bgzf<'a> {
    fn drop(&mut self) {
        unsafe {
            bgzf_close(self.inner);
        };
    }
}

impl<'a> Bgzf<'a> {
    /// Open specified file for reading or writing.
    ///
    /// `mode` matching \[rwag]\[u0-9]\: 'r' for reading, 'w' for
    ///  writing, 'a' for appending, 'g' for gzip rather than BGZF
    ///  compression (with 'w' only), and digit specifies the zlib
    ///  compression level.
    ///
    ///  Note that there is a distinction between 'u' and '0': the
    ///  first yields plain uncompressed output whereas the latter
    ///  outputs uncompressed data wrapped in the zlib format.
    #[inline]
    pub fn open(name: &CStr, mode: &CStr) -> Result<Self, BgzfError> {
        Self::make_bgzf_file(unsafe { bgzf_open(name.as_ptr(), mode.as_ptr()) })
    }

    /// Open existing file descriptor for reading or writing.  File should have been
    /// opened in binary mode to avoid problems on some platforms
    ///
    /// `mode` as described for [Bgzf::open()]
    #[inline]
    pub fn dopen(fd: c_int, mode: &CStr) -> Result<Self, BgzfError> {
        Self::make_bgzf_file(unsafe { bgzf_dopen(fd, mode.as_ptr()) })
    }

    /// Open existing HFile stream for reading or writing.  Note that `fp` is moved
    /// into the new Bgzp struct, so `fp` is no longer available (this is to prevent
    /// problems with double frees etc.)
    ///
    /// `mode` is as described for [Bgzf::open()]
    #[inline]
    pub fn hopen(fp: HFile, mode: &CStr) -> Result<Self, BgzfError> {
        let ptr = fp.into_raw_ptr();
        Self::make_bgzf_file(unsafe { bgzf_hopen(ptr, mode.as_ptr()) })
    }

    #[inline]
    fn make_bgzf_file(fp: *mut BgzfRaw) -> Result<Self, BgzfError> {
        if fp.is_null() {
            Err(BgzfError::OpenError)
        } else {
            Ok(Self {
                inner: fp,
                phantom: PhantomData,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TstBgzf<'a> {
        bgzf: Bgzf<'a>,
        name: *mut c_char,
        index: bool,
    }
    impl<'a> TstBgzf<'a> {
        fn new() -> Self {
            let name = unsafe { libc::strdup(c"test/htslib_test_XXXXXX".as_ptr()) };
            let mut fd = unsafe { libc::mkstemp(name) };
            assert!(fd >= 0);
            // Open Bgzf using file descriptor
            let mut bgzf = Bgzf::dopen(fd, c"w").unwrap();
            Self {
                bgzf,
                name,
                index: false,
            }
        }

        fn write_index(&mut self) {
            assert!(!self.name.is_null());
            unsafe {
                let p = self.name.add(10) as *mut u8;
                let c = *p;
                *p = b'x';
                self.bgzf.index_dump(CStr::from_ptr(self.name), None);
                *p = c;
            }
            self.index = true;
        }

        fn reopen(mut self) -> Self {
            let name = self.name;
            let index = self.index;
            self.name = std::ptr::null_mut();
            drop(self);
            let mut bgzf = Bgzf::open(unsafe { CStr::from_ptr(name) }, c"r").unwrap();
            if index {
                unsafe {
                    let p = name.add(10) as *mut u8;
                    let c = *p;
                    *p = b'x';
                    bgzf.index_load(CStr::from_ptr(name), None).unwrap();
                    *p = c;
                }
            }
            Self { bgzf, name, index }
        }
    }

    impl<'a> Drop for TstBgzf<'a> {
        fn drop(&mut self) {
            if !self.name.is_null() {
                unsafe {
                    libc::unlink(self.name);
                    if self.index {
                        *self.name.add(10) = b'x' as c_char;
                        libc::unlink(self.name);
                    }
                    libc::free(self.name as *mut c_void);
                }
            }
        }
    }
    #[test]
    fn read_tests() {
        let mut b = Bgzf::open(c"test/bgziptest.txt.gz", c"r").unwrap();
        assert_eq!(b.compression(), BgzfCompression::Bgzip);
        assert_eq!(b.peek().unwrap(), b'1');
        assert_eq!(b.getc().unwrap(), b'1');
        let mut ks = KString::new();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"22333444455555");

        let mut b = Bgzf::open(c"test/gzip.test.gz", c"r").unwrap();
        assert_eq!(b.compression(), BgzfCompression::Gzip);
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"122333444455555");

        let mut b = Bgzf::open(c"test/bgziptest.txt", c"r").unwrap();
        assert_eq!(b.compression(), BgzfCompression::None);
        let mut buf: [u8; 10] = [0; 10];
        let b1 = b.read(&mut buf).unwrap();
        assert_eq!(b1.len(), 10);
        assert_eq!(b1[9], b'4');
        let b1 = b.read(&mut buf).unwrap();
        assert_eq!(b1.len(), 5);
        assert_eq!(b1[4], b'5');

        let mut fp = HFile::open(c"test/bgziptest.txt.gz", c"r").unwrap();
        let mut b = Bgzf::hopen(fp, c"r").unwrap();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"122333444455555");

        let fd = unsafe { libc::open(c"test/bgziptest.txt.gz".as_ptr(), libc::O_RDONLY) };
        assert!(fd > 0);
        let mut b = Bgzf::dopen(fd, c"r").unwrap();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"122333444455555");
    }

    #[test]
    fn write_tests() {
        let mut tb = TstBgzf::new();
        let b = &mut tb.bgzf;
        assert_eq!(b.compression(), BgzfCompression::Bgzip);
        let buf = "This is a test\nSecond line".as_bytes();
        let l = b.write(buf).unwrap();
        assert_eq!(l, 26);
        let mut tb = tb.reopen();
        let b = &mut tb.bgzf;
        assert_eq!(b.compression(), BgzfCompression::Bgzip);
        let mut ks = KString::new();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"This is a test");
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"Second line");
        assert_eq!(b.get_line(10, &mut ks), Err(BgzfError::EOF));
    }

    #[test]
    fn read_index() {
        let fname = c"test/bgziptest.txt.gz";
        let mut b = Bgzf::open(fname, c"r").unwrap();
        b.index_load(fname, Some(c".gzi")).unwrap();
        b.useek(5).unwrap();
        let mut ks = KString::new();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"3444455555");
    }

    #[test]
    fn write_index() {
        let mut tb = TstBgzf::new();
        let b = &mut tb.bgzf;
        b.index_build_init().unwrap();
        let buf = "This is a test\nSecond line".as_bytes();
        b.write(buf).unwrap();
        b.flush().unwrap();
        tb.write_index();
        let mut tb = tb.reopen();
        let b = &mut tb.bgzf;
        b.check_eof().unwrap();
        b.useek(15).unwrap();
        let mut ks = KString::new();
        b.get_line(10, &mut ks).unwrap();
        assert_eq!(ks.to_cstr().unwrap(), c"Second line");
    }
}
