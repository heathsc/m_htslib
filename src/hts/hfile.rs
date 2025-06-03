use c2rust_bitfields::BitfieldStruct;
use libc::{EOF, c_char, c_int, c_uint, off_t, size_t, ssize_t};
use std::{
    ffi::{CStr, c_void},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::{NonNull, null},
};

use super::{Whence, hts_format::HtsFormat};
use crate::{cstr_len, error::HtsError, kstring::KString};

#[repr(C)]
struct HFileBackend {
    _unused: [u8; 0],
}

const UINT_LEN: usize = std::mem::size_of::<c_uint>();
/// Low-level input/output stream handle
/// The fields of this structure are declared here solely for the benefit
/// of the hFILE-related functions.  They may change in future releases.
/// User code should not use them directly; you should imagine that hFILE is an
/// opaque incomplete type
#[repr(C)]
#[derive(BitfieldStruct)]
pub struct HFileRaw {
    buffer: *mut c_char,
    begin: *mut c_char,
    end: *mut c_char,
    limit: *mut c_char,
    backend: *mut HFileBackend,
    offset: off_t,
    #[bitfield(name = "at_eof", ty = "c_uint", bits = "0..=0")]
    #[bitfield(name = "mobile", ty = "c_uint", bits = "1..=1")]
    #[bitfield(name = "readonly", ty = "c_uint", bits = "2..=2")]
    bfield: [u8; UINT_LEN],
    has_errno: c_int,
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hopen(fname: *const c_char, mode: *const c_char, ...) -> *mut HFileRaw;
    fn hdopen(fd: c_int, mode: *const c_char) -> *mut HFileRaw;
    fn hisremote(fname: *const c_char) -> c_int;
    fn haddextension(
        buffer: *mut KString,
        fname: *const c_char,
        replace: c_int,
        extension: *const c_char,
    ) -> *mut c_char;
    fn hclose(fp: *mut HFileRaw) -> c_int;
    fn hseek(fp: *mut HFileRaw, offset: off_t, whence: c_int) -> off_t;
    fn hgetdelim(buffer: *mut c_char, size: size_t, delim: c_int, fp: *mut HFileRaw) -> ssize_t;
    fn hpeek(fp: *mut HFileRaw, buffer: *mut c_void, nbytes: size_t) -> ssize_t;
    fn hflush(fp: *mut HFileRaw) -> c_int;
    fn hfile_set_blksize(fp: *mut HFileRaw, bufsize: size_t) -> c_int;
    fn hread2(fp: *mut HFileRaw, buffer: *mut c_void, nbytes: size_t, copied: size_t) -> ssize_t;
    fn hwrite2(fp: *mut HFileRaw, data: *const c_void, total: size_t, copied: size_t) -> ssize_t;
    fn hputs2(data: *const c_char, total: size_t, copied: size_t, fp: *mut HFileRaw) -> c_int;
    fn hgetc2(fp: *mut HFileRaw) -> c_int;
    fn hputc2(c: c_int, fp: *mut HFileRaw) -> c_int;
    fn hts_detect_format2(fp_: *mut HFileRaw, fname: *const c_char, fmt: *mut HtsFormat) -> c_int;
}

impl HFileRaw {
    /// For writing streams, flush buffered output to the underlying stream
    pub fn flush(&mut self) -> Result<(), HtsError> {
        match unsafe { hflush(self) } {
            0 => Ok(()),
            EOF => Err(HtsError::EOF),
            _ => Err(HtsError::UnknownError),
        }
    }

    #[inline]
    pub fn has_error(&self) -> bool {
        self.has_errno != 0
    }

    #[inline]
    pub fn clear_error(&mut self) {
        self.has_errno = 0
    }

    /// Reposition the read/write stream offset
    ///
    /// On success, returns the resulting new stream offset
    pub fn seek(&mut self, offset: off_t, whence: Whence) -> Result<off_t, HtsError> {
        let off = unsafe { hseek(self, offset, whence as c_int) };
        if off < 0 {
            Err(HtsError::SeekFailed)
        } else {
            Ok(off)
        }
    }

    /// Report the current stream offset
    #[inline]
    pub fn tell(&self) -> off_t {
        self.offset + (unsafe { self.begin.byte_offset_from(self.buffer) } as off_t)
    }

    /// Read one character from stream
    /// Returns None on EOF or IO error
    #[inline]
    pub fn getc(&mut self) -> Option<c_char> {
        if self.end > self.begin {
            unsafe {
                let c = *self.begin;
                self.begin = self.begin.add(1);
                Some(c)
            }
        } else {
            let c = unsafe { hgetc2(self) };
            if c < 0 { None } else { Some(c as c_char) }
        }
    }

    /// Read from the stream until the delimiter, up to a maximum length
    /// @param buffer  The buffer into which bytes will be written
    /// @param delim   The delimiter character
    /// On success returns the slice of buffer containing the read bytes (including the null)
    ///
    /// Bytes will be read into the buffer up to and including a delimiter, until
    /// EOF is reached, or _size-1_ bytes have been written, whichever comes first
    #[inline]
    pub fn get_delim<'a>(
        &mut self,
        buffer: &'a mut [c_char],
        delim: char,
    ) -> Result<&'a [c_char], HtsError> {
        let l = unsafe { hgetdelim(buffer.as_mut_ptr(), buffer.len(), delim as c_int, self) };
        Self::get_out_slice(buffer, l, true)
    }

    #[inline]
    fn get_out_slice(buffer: &[c_char], l: ssize_t, inc_zero: bool) -> Result<&[c_char], HtsError> {
        match l {
            ..=-1 => Err(HtsError::IOError),
            0 => Err(HtsError::EOF),
            _ => {
                let l1 = if inc_zero {
                    assert_eq!(buffer[l as usize], 0);
                    l + 1
                } else {
                    l
                } as usize;
                Ok(&buffer[..l1])
            }
        }
    }
    /// Specialization of get_delim() using '\n' as the delimiter (so allowing reading line by line)
    #[inline]
    pub fn get_ln<'a>(&mut self, buffer: &'a mut [c_char]) -> Result<&'a [c_char], HtsError> {
        self.get_delim(buffer, '\n')
    }

    /// Peek at characters to be read without removing them from buffers
    /// @param fp      The file stream
    /// @param buffer  The buffer to which the peeked bytes will be written
    /// On success returns slice from buffer with read bytes
    ///
    /// The characters peeked at remain in the stream's internal buffer, and will be
    /// returned by subsequent calls to hread() etc.
    #[inline]
    pub fn peek<'a>(&mut self, buffer: &'a mut [c_char]) -> Result<&'a [c_char], HtsError> {
        let l = unsafe { hpeek(self, buffer.as_mut_ptr() as *mut c_void, buffer.len()) };
        Self::get_out_slice(buffer, l, false)
    }

    /// Read block of characters from stream
    /// @param fp      The file stream
    /// @param buffer  The buffer to which the read bytes will be written
    /// On success returns slice from buffer with read bytes.  This can be smaller than
    /// buffer if EOF is reached.
    #[inline]
    pub fn read<'a>(&mut self, buffer: &'a mut [c_char]) -> Result<&'a [c_char], HtsError> {
        let nbytes = buffer.len();
        assert!(nbytes <= isize::MAX as usize, "Data size too large");
        assert!(!(self.end.is_null() || self.begin.is_null()));
        let n = unsafe { self.end.byte_offset_from(self.begin) };
        assert!(n >= 0);
        let n = (n as usize).min(nbytes);
        unsafe {
            libc::memcpy(buffer.as_ptr() as *mut c_void, self.begin as *mut c_void, n);
            self.begin = self.begin.add(n)
        };
        let l = if n == nbytes || self.mobile() == 0 {
            n as isize
        } else {
            unsafe { hread2(self, buffer.as_mut_ptr() as *mut c_void, nbytes, n) }
        };
        Self::get_out_slice(buffer, l, false)
    }

    ///  @abstract      Determine format by peeking at the start of a file
    ///
    ///   @param fp     File opened for reading, positioned at the beginning
    ///
    ///   @param fname  Optional file name.  Note that some files will not be recognized correctly without the filename
    ///
    ///   @param fmt    Format structure that will be filled out on return
    pub fn detect_format(
        &mut self,
        fname: Option<&CStr>,
        fmt: &mut HtsFormat,
    ) -> Result<(), HtsError> {
        let ptr = if let Some(f) = fname {
            f.as_ptr()
        } else {
            null()
        };
        if unsafe { hts_detect_format2(self, ptr, fmt) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    /// Write a block of characters to the file
    ///
    /// In the absence of I/O errors, the full _nbytes_ will be written.
    ///
    /// Returns the number of bytes written if successful
    pub fn write(&mut self, data: &[u8]) -> Result<size_t, HtsError> {
        let nbytes = data.len();
        assert!(nbytes <= isize::MAX as usize, "Data size too large");
        assert!(!(self.limit.is_null() || self.begin.is_null() || self.buffer.is_null()));
        let nbytes1 = nbytes as isize;
        if self.mobile() == 0 {
            let n = unsafe { self.limit.byte_offset_from(self.begin) };
            if n < nbytes1 {
                unsafe {
                    hfile_set_blksize(
                        self,
                        (self.limit.byte_offset_from(self.buffer) + nbytes1) as size_t,
                    );
                }
                self.end = self.limit;
            }
        }
        let n = unsafe { self.limit.byte_offset_from(self.begin) };
        if nbytes1 >= n && self.begin == self.buffer {
            // Go straight to hwrite2 if the buffer is empty and the request won't fit
            self.hw2(data, nbytes, 0)
        } else {
            let n = n.min(nbytes1) as size_t;
            unsafe {
                libc::memcpy(self.begin as *mut c_void, data.as_ptr() as *const c_void, n);
                self.begin = self.begin.add(n)
            }
            if n == nbytes {
                Ok(n as size_t)
            } else {
                self.hw2(data, nbytes, n)
            }
        }
    }

    fn hw2(&mut self, data: &[u8], nbytes: size_t, n: size_t) -> Result<size_t, HtsError> {
        let n = unsafe { hwrite2(self, data.as_ptr() as *const c_void, nbytes, n) };
        if n < 0 {
            Err(HtsError::IOError)
        } else {
            Ok(n as size_t)
        }
    }

    /// Write a character to the stream
    #[inline]
    #[allow(dead_code)]
    fn putc(&mut self, c: c_char) -> Result<(), HtsError> {
        if self.begin < self.limit {
            unsafe {
                *self.begin = c;
                self.begin = self.begin.add(1);
            }
            Ok(())
        } else {
            let c = unsafe { hputc2(c as c_int, self) };
            if c < 0 {
                Err(HtsError::IOError)
            } else {
                Ok(())
            }
        }
    }

    /// Write a CStr to the stream
    #[inline]
    pub fn puts(&mut self, s: &CStr) -> Result<(), HtsError> {
        let nbytes = cstr_len(s) as size_t;
        let n = unsafe { self.limit.byte_offset_from(self.begin) };
        assert!(n >= 0);
        let n = (n as size_t).min(nbytes);
        unsafe { libc::memcpy(self.begin as *mut c_void, s.as_ptr() as *const c_void, n) };
        self.begin = unsafe { self.begin.add(n) };
        if n == nbytes || unsafe { hputs2(s.as_ptr(), nbytes, n, self) } == 0 {
            Ok(())
        } else {
            Err(HtsError::EOF)
        }
    }
}

/// inner is always non-null, but we don't use NonNull<> here because
/// we don't want to assume Covariance.
pub struct HFile<'a> {
    inner: NonNull<HFileRaw>,
    phantom: PhantomData<&'a mut HFileRaw>,
}

impl Deref for HFile<'_> {
    type Target = HFileRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for HFile<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

unsafe impl Send for HFile<'_> {}
unsafe impl Sync for HFile<'_> {}

impl Drop for HFile<'_> {
    fn drop(&mut self) {
        unsafe { hclose(self.deref_mut()) };
    }
}

impl HFile<'_> {
    /// Open the named file or URL as a stream
    /// The usual `fopen(3)` _mode_ letters are supported: one of
    /// `r` (read), `w` (write), `a` (append), optionally followed by any of
    /// `+` (update), `e` (close on `exec(2)`), `x` (create exclusively),
    pub fn open(name: &CStr, mode: &CStr) -> Result<Self, HtsError> {
        Self::make_hfile(unsafe { hopen(name.as_ptr(), mode.as_ptr()) })
    }

    /// Associate a stream with an existing open file descriptor
    ///
    /// Note that the file must be opened in binary mode, or else
    /// there will be problems on platforms that make a difference
    /// between text and binary mode.
    ///
    /// For socket descriptors (on Windows), _mode_ should contain `s`.
    pub fn dopen(fd: c_int, mode: &CStr) -> Result<Self, HtsError> {
        Self::make_hfile(unsafe { hdopen(fd, mode.as_ptr()) })
    }

    fn make_hfile(fp: *mut HFileRaw) -> Result<Self, HtsError> {
        match NonNull::new(fp) {
            None => Err(HtsError::FileOpenError),
            Some(p) => Ok(Self {
                inner: p,
                phantom: PhantomData,
            }),
        }
    }

    pub(crate) fn into_raw_ptr(self) -> *mut HFileRaw {
        let mut p = std::mem::ManuallyDrop::new(self);
        unsafe { p.inner.as_mut() }
    }
}

/// Report whether the file name or URL denotes remote storage
///  "Remote" means involving e.g. explicit network access, with the implication
/// that callers may wish to cache such files' contents locally.
#[inline]
pub fn is_remote(name: &CStr) -> bool {
    unsafe { hisremote(name.as_ptr()) != 0 }
}

/// Append an extension or replace an existing extension
/// @param buffer     The kstring to be used to store the modified filename
/// @param filename   The filename to be (copied and) adjusted
/// @param replace    If true, one extension (if any) is removed first
/// @param extension  The extension to be added (e.g. ".csi")
///
/// If _filename_ is an URL, alters extensions at the end of the `hier-part`,
/// leaving any trailing `?query` or `#fragment` unchanged.
pub fn add_extension(
    buffer: &mut KString,
    fname: &CStr,
    replace: bool,
    extension: &CStr,
) -> Result<(), HtsError> {
    if unsafe {
        haddextension(
            buffer,
            fname.as_ptr(),
            if replace { 1 } else { 0 },
            extension.as_ptr(),
        )
    }
    .is_null()
    {
        Err(HtsError::OperationFailed)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TstHFile<'a> {
        hfile: HFile<'a>,
        name: *mut c_char,
    }

    impl TstHFile<'_> {
        fn new() -> Self {
            let name = unsafe { libc::strdup(c"test/htslib_test_XXXXXX".as_ptr()) };
            let fd = unsafe { libc::mkstemp(name) };
            assert!(fd >= 0);
            // Open HFile using file descriptor
            let hfile = HFile::dopen(fd, c"rw").unwrap();
            Self { hfile, name }
        }
    }

    impl Drop for TstHFile<'_> {
        fn drop(&mut self) {
            unsafe {
                libc::unlink(self.name);
                libc::free(self.name as *mut c_void);
            }
        }
    }

    #[test]
    fn get_ln_test() {
        let mut fp = HFile::open(c"test/list.txt", c"r").unwrap();
        let mut buf: [c_char; 256] = [0; 256];
        let mut n = 0;
        while let Ok(s) = fp.get_ln(&mut buf) {
            // Each line should terminate in a '\n' and a null
            assert!(s.len() > 1);
            assert_eq!(&s[s.len() - 2..], &[10, 0]);
            n += 1;
        }
        assert_eq!(n, 3);
    }

    #[test]
    fn read_test() {
        let mut fp = HFile::open(c"test/list.txt", c"r").unwrap();
        let mut buf: [c_char; 8] = [0; 8];

        // Check content after reading 8 bytes
        let v = fp.read(&mut buf).unwrap();
        assert_eq!(v.len(), 8);
        assert_eq!(v[7], b'e' as c_char);

        // Try using getc()
        let c = fp.getc().unwrap();
        assert_eq!(c, b'e' as c_char);
        let c = fp.getc().unwrap();
        assert_eq!(c, 10 as c_char);

        // Try seek()
        let off = fp.seek(-2, Whence::Cur).unwrap();
        assert_eq!(off, 8);

        // Check using a slice of buf
        let v = fp.read(&mut buf[0..4]).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[3], b'a' as c_char);

        // Try using peek() rather than read()
        let v = fp.peek(&mut buf).unwrap();
        assert_eq!(v.len(), 5);
        assert_eq!(v[4], 10);

        // Now repeat with read(), and we should see the same content
        let v = fp.read(&mut buf).unwrap();
        assert_eq!(v.len(), 5);
        assert_eq!(v[4], 10);

        // Check that we are at EOF
        assert!(matches!(fp.read(&mut buf), Err(HtsError::EOF)));
    }

    #[test]
    fn write_test() {
        let mut tf = TstHFile::new();

        // Open HFile using file descriptor
        let fp = &mut tf.hfile;

        let l = fp
            .write("This is a test\nSecond line\n".as_bytes())
            .unwrap();
        assert_eq!(l, 27);

        fp.puts(c"1,2,3,4,5,").unwrap();
        fp.putc(b'6' as c_char).unwrap();
        fp.putc(10).unwrap();

        fp.flush().unwrap();

        fp.seek(17, Whence::Set).unwrap();
        let c = fp.getc().unwrap();
        assert_eq!(c, b'c' as c_char);
        assert_eq!(fp.tell(), 18);
        assert_eq!(fp.seek(19, Whence::Cur).unwrap(), 37);
        let c = fp.getc().unwrap();
        assert_eq!(c, b'6' as c_char);
    }

    #[test]
    fn add_extension_to_file() {
        let mut ks = KString::new();
        add_extension(&mut ks, c"testfile.txt.gz", true, c".bz2").unwrap();
        let s = ks.as_cstr().to_string_lossy();
        assert_eq!(s, "testfile.txt.bz2");
        add_extension(&mut ks, c"testfile.txt", false, c".gz").unwrap();
        let s = ks.as_cstr().to_string_lossy();
        assert_eq!(s, "testfile.txt.gz");
    }

    #[test]
    fn detect_format() {
        // SAM file test
        let mut fp = HFile::open(c"test/realn01.sam", c"r").unwrap();
        let mut fmt = HtsFormat::new();
        fp.detect_format(None, &mut fmt).unwrap();
        let s = fmt.format_description();
        assert_eq!(s.to_string_lossy(), "SAM version 1.4 sequence text");

        // FASTA test
        let mut fp = HFile::open(c"test/realn01.fa", c"r").unwrap();
        fp.detect_format(None, &mut fmt).unwrap();
        assert_eq!(
            fmt.format_description().to_string_lossy(),
            "FASTA sequence text"
        );

        // FASTA Index test
        // Note that without the filename, this is seen as a BED file
        let mut fp = HFile::open(c"test/realn01.fa.fai", c"r").unwrap();
        fp.detect_format(None, &mut fmt).unwrap();
        assert_eq!(
            fmt.format_description().to_string_lossy(),
            "BED genomic region text"
        );

        // Adding the filename allows correct detection of the format
        fp.detect_format(Some(c"test/realn01.fa.fai"), &mut fmt)
            .unwrap();
        assert_eq!(
            fmt.format_description().to_string_lossy(),
            "FASTA-IDX index text"
        );

        // Detection of BGZIP compressed data
        let mut fp = HFile::open(c"test/bgziptest.txt.gz", c"r").unwrap();
        fp.detect_format(None, &mut fmt).unwrap();
        assert_eq!(
            fmt.format_description().to_string_lossy(),
            "unknown BGZF-compressed data"
        );
    }
}
