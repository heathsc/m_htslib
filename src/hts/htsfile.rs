use c2rust_bitfields::BitfieldStruct;
use libc::{c_char, c_int, c_uchar};

use std::{
    ffi::{CStr, c_void},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use super::{
    hfile::{HFile, HFileRaw},
    hts_format::{HtsFmtOption, HtsFormat, hts_file_set_opt},
    hts_idx::HtsIdxRaw,
    hts_opt::HtsOptRaw,
    hts_thread_pool::HtsThreadPool,
};

use crate::{
    HtsError, bgzf::BgzfRaw, cram::CramFdRaw, hts::hts_opt::HtsOpt, kstring::KString,
    sam::sam_hdr::SamHdrRaw,
};

#[repr(C)]
pub(crate) struct HtsFilter {
    _unused: [u8; 0],
}

#[repr(C)]
union HtsFileType {
    bgzf: *mut BgzfRaw,
    cram_fd: *mut CramFdRaw,
    hfile: *mut HFileRaw,
}

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct HtsFileRaw {
    #[bitfield(name = "is_bin", ty = "c_uchar", bits = "0..=0")]
    #[bitfield(name = "is_write", ty = "c_uchar", bits = "1..=1")]
    #[bitfield(name = "is_be", ty = "c_uchar", bits = "2..=2")]
    #[bitfield(name = "is_cram", ty = "c_uchar", bits = "3..=3")]
    #[bitfield(name = "is_bgzf", ty = "c_uchar", bits = "4..=4")]
    #[bitfield(name = "dummy", ty = "u32", bits = "5..=31")]
    bfield: [u8; 4],
    lineno: i64,
    line: KString,
    fn_: *mut c_char,
    fn_aux: *mut c_char,
    fp: HtsFileType,
    state: *mut c_void,
    format: HtsFormat,
    idx: *mut HtsIdxRaw,
    fnidx: *const c_char,
    bam_header: *mut SamHdrRaw,
    filter: *mut HtsFilter,
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hts_open(fn_: *const c_char, mode: *const c_char) -> *mut HtsFileRaw;
    fn hts_open_format(
        fn_: *const c_char,
        mode: *const c_char,
        fmt: *const HtsFormat,
    ) -> *mut HtsFileRaw;
    fn hts_hopen(fp: *mut HFileRaw, fn_: *const c_char, mode: *const c_char) -> *mut HtsFileRaw;
    fn hts_close(fp: *mut HtsFileRaw) -> c_int;
    fn hts_get_format(fp: *const HtsFileRaw) -> *const HtsFormat;
    fn hts_set_threads(fp: *mut HtsFileRaw, t_: c_int) -> c_int;
    fn hts_set_thread_pool(fp: *mut HtsFileRaw, p: *const HtsThreadPool) -> c_int;
    fn hts_set_cache_size(fp: *mut HtsFileRaw, n: c_int);
    fn hts_flush(fp: *mut HtsFileRaw) -> c_int;
    fn hts_opt_apply(fp: *mut HtsFileRaw, opts: *mut HtsOptRaw) -> c_int;
    fn hts_getline(fp: *mut HtsFileRaw, delim: c_int, str: *mut KString) -> c_int;
    fn hts_set_fai_filename(fp: *mut HtsFileRaw, expr: *const c_char) -> c_int;
    fn hts_set_filter_expression(fp: *mut HtsFileRaw, fn_aux: *const c_char) -> c_int;
    fn hts_check_EOF(fp: *mut HtsFileRaw) -> c_int;

}

impl HtsFileRaw {
    /// For output streams, flush any buffered data
    pub fn flush(&mut self) -> Result<(), HtsError> {
        if unsafe { hts_flush(self) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    /// Returns the files format information
    pub fn get_format(&self) -> &HtsFormat {
        unsafe { &*hts_get_format(self) }
    }

    /// Read a line from file (and it's \n or \r\n terminator into `str`.
    /// The terminator is not written to `str`.
    pub fn getline(&mut self, str: &mut KString) -> Result<(), HtsError> {
        let i = unsafe { hts_getline(self, 10, str) };
        if i == 0 {
            Ok(())
        } else if i == -1 {
            Err(HtsError::EOF)
        } else {
            Err(HtsError::IOError)
        }
    }

    pub fn opt_apply(&mut self, opt: &mut HtsOpt) -> Result<(), HtsError> {
        if unsafe { hts_opt_apply(self, opt.deref_mut()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OptApplyOperationFailed)
        }
    }

    pub fn set_threads(&mut self, n: c_int) -> Result<(), HtsError> {
        if unsafe { hts_set_threads(self, n) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    pub fn set_thread_pool(&mut self, tp: &HtsThreadPool) -> Result<(), HtsError> {
        if unsafe { hts_set_thread_pool(self, tp) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    pub fn set_cache_site(&mut self, n: c_int) {
        assert!(n >= 0);
        unsafe { hts_set_cache_size(self, n) }
    }

    pub fn set_fai_filename(&mut self, fn_aux: &CStr) -> Result<(), HtsError> {
        if unsafe { hts_set_fai_filename(self, fn_aux.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    pub fn set_filter_expression(&mut self, expr: &CStr) -> Result<(), HtsError> {
        if unsafe { hts_set_filter_expression(self, expr.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    pub fn check_eof(&mut self) -> Result<(), HtsError> {
        match unsafe { hts_check_EOF(self) } {
            0 => Err(HtsError::MissingEOFMarker),
            1 => Ok(()),
            2 => Err(HtsError::NoEOFMarkerForFileType),
            3 => Err(HtsError::NoEOFMarkerCheckForFileSystem),
            _ => Err(HtsError::UnknownError),
        }
    }

    #[inline]
    pub fn set_opt(&mut self, opt: &mut HtsFmtOption) -> Result<(), HtsError> {
        hts_file_set_opt(self, opt)
    }
}

pub struct HtsFile<'a> {
    inner: NonNull<HtsFileRaw>,
    // As we can (and often do) attach a threadpool to an htsfile, then we need
    // to track the lifetime of this
    phantom: PhantomData<&'a HtsThreadPool>,
}

impl Deref for HtsFile<'_> {
    type Target = HtsFileRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for HtsFile<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

unsafe impl Send for HtsFile<'_> {}
unsafe impl Sync for HtsFile<'_> {}

impl Drop for HtsFile<'_> {
    fn drop(&mut self) {
        unsafe { hts_close(self.deref_mut()) };
    }
}

impl HtsFile<'_> {
    /// Open a sequence data (SAM/BAM/CRAM) or variant data (VCF/BCF)
    /// or possibly-compressed textual line-orientated file.
    ///
    /// `fn` is the file name or "-" for stdin/stdout. For indexed files
    /// with a non-standard naming, the file name can include the
    /// name of the index file delimited with HTS_IDX_DELIM.
    ///
    /// `mode` is the mode matching \[rwa]\[bcefFguxz0-9]*
    ///
    /// With 'r' opens for reading; any further format mode letters are ignored
    /// as the format is detected by checking the first few bytes or BGZF blocks
    /// of the file.  With 'w' or 'a' opens for writing or appending, with format
    /// specifier letters:
    ///
    /// b  binary format (BAM, BCF, etc) rather than text (SAM, VCF, etc)
    ///
    /// c  CRAM format
    ///
    /// f  FASTQ format
    ///
    /// F  FASTA format
    ///
    /// g  gzip compressed
    ///
    /// u  uncompressed
    ///
    /// z  bgzf compressed
    ///
    /// \[0-9]  zlib compression level
    ///
    /// and with non-format option letters (for any of 'r'/'w'/'a'):
    ///
    /// e  close the file on exec(2) (opens with O_CLOEXEC, where supported)
    ///
    /// x  create the file exclusively (opens with O_EXCL, where supported)
    ///
    /// Note that there is a distinction between 'u' and '0': the first yields
    /// plain uncompressed output whereas the latter outputs uncompressed data
    /// wrapped in the zlib format.
    ///
    /// *Example*
    ///
    /// \[rw]b  .. compressed BCF, BAM, FAI
    ///
    /// \[rw]bu .. uncompressed BCF
    ///
    /// \[rw]z  .. compressed VCF
    ///
    /// \[rw]   .. uncompressed VCF
    pub fn open(name: &CStr, mode: &CStr) -> Result<Self, HtsError> {
        let fp = unsafe { hts_open(name.as_ptr(), mode.as_ptr()) };
        Self::mk_hts_file(fp)
    }

    /// Open a SAM/BAM/CRAM/VCF/BCF/etc file
    ///
    /// @param fn       The file name or "-" for stdin/stdout
    ///
    /// @param mode     Open mode, as per hts_open()
    ///
    /// @param fmt      Optional format specific parameters
    ///
    /// @discussion
    ///
    ///  See hts_open() for description of fn and mode.
    ///
    ///  Opts contains a format string (sam, bam, cram, vcf, bcf) which will,
    ///  if defined, override mode.  Opts also contains a linked list of hts_opt
    ///  structures to apply to the open file handle.  These can contain things
    ///  like pointers to the reference or information on compression levels,
    ///  block sizes, etc.
    pub fn open_format(name: &CStr, mode: &CStr, format: &HtsFormat) -> Result<Self, HtsError> {
        let fp = unsafe { hts_open_format(name.as_ptr(), mode.as_ptr(), format) };
        Self::mk_hts_file(fp)
    }

    /// Open an existing stream as an HtsFile
    pub fn hopen(hfile: HFile, name: &CStr, mode: &CStr) -> Result<Self, HtsError> {
        let ptr = hfile.into_raw_ptr();
        let fp = unsafe { hts_hopen(ptr, name.as_ptr(), mode.as_ptr()) };
        Self::mk_hts_file(fp)
    }

    fn mk_hts_file(fp: *mut HtsFileRaw) -> Result<Self, HtsError> {
        match NonNull::new(fp) {
            None => Err(HtsError::FileOpenError),
            Some(p) => Ok(Self {
                inner: p,
                phantom: PhantomData,
            }),
        }
    }
}
