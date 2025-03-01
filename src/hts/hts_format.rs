use std::ffi::{CStr, CString};
use std::ptr;

use super::{
    hts_opt::{HtsOptRaw, HtsProfileOption},
    hts_thread_pool::HtsThreadPool,
    HtsFileRaw,
};

use crate::{
    cram::{CramFdRaw, CramRange, Refs},
    error::HtsError,
};
use libc::{c_char, c_int, c_short, c_void};

#[repr(C)]
#[derive(Default)]
pub enum HtsFormatCategory {
    #[default]
    UnknownCategory,
    SequenceData,
    VariantData,
    IndexFile,
    RegionList,
}

#[repr(C)]
#[derive(Default, PartialEq)]
pub enum HtsExactFormat {
    #[default]
    UnknownFormat,
    BinaryFormat,
    TextFormat,
    Sam,
    Bam,
    Bai,
    Cram,
    Crai,
    Vcf,
    Bcf,
    Csi,
    Gzi,
    Tbi,
    Bed,
    HtsGet,
    EmptyFormat,
    FastaFormat,
    FastqFormat,
    FaiFormat,
    FqiFormat,
    HtsCrypt4GH,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum HtsFmtOptionRaw {
    // CRAM specific
    CRAM_OPT_DECODE_MD,
    CRAM_OPT_PREFIX,
    CRAM_OPT_VERBOSITY,
    // obsolete, use hts_set_log_level() instead
    CRAM_OPT_SEQS_PER_SLICE,
    CRAM_OPT_SLICES_PER_CONTAINER,
    CRAM_OPT_RANGE,
    CRAM_OPT_VERSION,
    // rename to cram_version?
    CRAM_OPT_EMBED_REF,
    CRAM_OPT_IGNORE_MD5,
    CRAM_OPT_REFERENCE,
    // make general
    CRAM_OPT_MULTI_SEQ_PER_SLICE,
    CRAM_OPT_NO_REF,
    CRAM_OPT_USE_BZIP2,
    CRAM_OPT_SHARED_REF,
    CRAM_OPT_NTHREADS,
    // deprecated, use HTS_OPT_NTHREADS
    CRAM_OPT_THREAD_POOL,
    // make general
    CRAM_OPT_USE_LZMA,
    CRAM_OPT_USE_RANS,
    CRAM_OPT_REQUIRED_FIELDS,
    CRAM_OPT_LOSSY_NAMES,
    CRAM_OPT_BASES_PER_SLICE,
    CRAM_OPT_STORE_MD,
    CRAM_OPT_STORE_NM,
    CRAM_OPT_RANGE_NOSEEK,
    // CRAM_OPT_RANGE minus the seek
    CRAM_OPT_USE_TOK,
    CRAM_OPT_USE_FQZ,
    CRAM_OPT_USE_ARITH,
    CRAM_OPT_POS_DELTA, // force delta for AP, even on non-pos sorted data

    // General purpose
    HTS_OPT_COMPRESSION_LEVEL = 100,
    HTS_OPT_NTHREADS,
    HTS_OPT_THREAD_POOL,
    HTS_OPT_CACHE_SIZE,
    HTS_OPT_BLOCK_SIZE,
    HTS_OPT_FILTER,
    HTS_OPT_PROFILE,

    // Fastq

    // Boolean.
    // Read / Write CASAVA 1.8 format.
    // See https://emea.support.illumina.com/content/dam/illumina-support/documents/documentation/software_documentation/bcl2fastq/bcl2fastq_letterbooklet_15038058brpmi.pdf
    //
    // The CASAVA tag matches \d:[YN]:\d+:[ACGTN]+
    // The first \d is read 1/2 (1 or 2), [YN] is QC-PASS/FAIL flag,
    // \d+ is a control number, and the sequence at the end is
    // for barcode sequence.  Barcodes are read into the aux tag defined
    // by FASTQ_OPT_BARCODE ("BC" by default).
    FASTQ_OPT_CASAVA = 1000,

    // String.
    // Whether to read / write extra SAM format aux tags from the fastq
    // identifier line.  For reading this can simply be "1" to request
    // decoding aux tags.  For writing it is a comma separated list of aux
    // tag types to be written out.
    FASTQ_OPT_AUX,

    // Boolean.
    // Whether to add /1 and /2 to read identifiers when writing FASTQ.
    // These come from the BAM_FREAD1 or BAM_FREAD2 flags.
    // (Detecting the /1 and /2 is automatic when reading fastq.)
    FASTQ_OPT_RNUM,

    // Two character string.
    // Barcode aux tag for CASAVA; defaults to "BC".
    FASTQ_OPT_BARCODE,

    // Process SRA and ENA read names which pointlessly move the original
    // name to the second field and insert a constructed <run>.<number>
    // name in its place.
    FASTQ_OPT_NAME2,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub enum MultiSeqOpt {
    Auto = -1,
    Single = 0,
    Multi = 1,
}

pub enum HtsFmtOption<'a, 'b> {
    CramDecodeMd(c_int),
    CramPrefix(&'a CStr),
    CramVerbosity,
    CramSeqsPerSlice(c_int),
    CramBasesPerSlice(c_int),
    CramSlicesPerContainer(c_int),
    CramEmbedRef(bool),
    CramNoRef(bool),
    CramPosDelta(bool),
    CramIgnoreMd5(bool),
    CramLossyReadNames(bool),
    CramUseBzip2(bool),
    CramUseRans(bool),
    CramUseTok(bool),
    CramUseFqz(bool),
    CramUseArith(bool),
    CramUseLzma(bool),
    CramSharedRef(*mut Refs),
    CramRange(*mut CramRange),
    CramRangeNoSeek(*mut CramRange),
    CramOptReference(&'a CStr),
    CramVersion(&'a CStr),
    CramMultiSeq(MultiSeqOpt),
    CramNThreads(c_int),
    CramThreadPool(&'a mut HtsThreadPool<'b>),
    CramRequiredFields(c_int),
    CramStoreMd(bool),
    CramStoreNm(bool),

    HtsNThreads(c_int),
    HtsBlockSize(c_int),
    HtsThreadPool(&'a mut HtsThreadPool<'b>),
    HtsCacheSize(c_int),
    HtsCompressionLevel(c_int),
    HtsProfile(HtsProfileOption),
    HtsFilter(&'a CStr),

    FastQCasava,
    FastQRNum,
    FastQName2,
    FastQAux(&'a CStr),
    FastQBarcode(&'a CStr),
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum HtsCompression {
    #[default]
    NoCompression,
    Gzip,
    Bgzf,
    Custom,
    Bzip2Compression,
}

#[repr(C)]
#[derive(Default)]
pub struct HtsFormatVersion {
    major: c_short,
    minor: c_short,
}

#[repr(C)]
pub struct HtsFormat {
    category: HtsFormatCategory,
    format: HtsExactFormat,
    version: HtsFormatVersion,
    compression: HtsCompression,
    compression_level: c_short,
    specific: *mut HtsOptRaw,
}

impl Default for HtsFormat {
    fn default() -> Self {
        Self {
            category: Default::default(),
            format: Default::default(),
            version: Default::default(),
            compression: Default::default(),
            compression_level: 0,
            specific: ptr::null_mut(),
        }
    }
}

#[macro_export]
macro_rules! set_opt {
    ($f: ident, $fp:expr, $opt: expr) => {
        match $opt {
            HtsFmtOption::CramDecodeMd(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_DECODE_MD, i)
            }
            HtsFmtOption::CramPrefix(s) => do_ptr!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_DECODE_MD, s),
            HtsFmtOption::CramVerbosity => do_none!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_VERBOSITY),
            HtsFmtOption::CramSeqsPerSlice(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_SEQS_PER_SLICE, i)
            }
            HtsFmtOption::CramBasesPerSlice(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_BASES_PER_SLICE, i)
            }
            HtsFmtOption::CramSlicesPerContainer(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_SLICES_PER_CONTAINER, i)
            }
            HtsFmtOption::CramEmbedRef(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_EMBED_REF, b)
            }
            HtsFmtOption::CramNoRef(b) => do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_NO_REF, b),

            HtsFmtOption::CramPosDelta(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_POS_DELTA, b)
            }
            HtsFmtOption::CramIgnoreMd5(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_IGNORE_MD5, b)
            }
            HtsFmtOption::CramLossyReadNames(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_LOSSY_NAMES, b)
            }
            HtsFmtOption::CramUseBzip2(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_BZIP2, b)
            }
            HtsFmtOption::CramUseRans(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_RANS, b)
            }
            HtsFmtOption::CramUseTok(b) => do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_TOK, b),

            HtsFmtOption::CramUseFqz(b) => do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_FQZ, b),

            HtsFmtOption::CramUseArith(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_ARITH, b)
            }
            HtsFmtOption::CramUseLzma(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_USE_LZMA, b)
            }
            HtsFmtOption::CramRange(r) => do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_RANGE, r),
            HtsFmtOption::CramRangeNoSeek(r) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_RANGE_NOSEEK, r)
            }
            HtsFmtOption::CramOptReference(s) => {
                do_ptr!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_REFERENCE, s)
            }
            HtsFmtOption::CramVersion(s) => {
                do_ptr!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_VERSION, s)
            }
            HtsFmtOption::CramMultiSeq(x) => {
                do_enum!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_MULTI_SEQ_PER_SLICE, x)
            }
            HtsFmtOption::CramNThreads(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_NTHREADS, i)
            }
            HtsFmtOption::CramThreadPool(p) => {
                do_ptr!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_THREAD_POOL, p)
            }
            HtsFmtOption::CramRequiredFields(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_REQUIRED_FIELDS, i)
            }
            HtsFmtOption::CramStoreMd(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_STORE_MD, b)
            }
            HtsFmtOption::CramStoreNm(b) => {
                do_bool!($f, $fp, HtsFmtOptionRaw::CRAM_OPT_STORE_NM, b)
            }
            HtsFmtOption::HtsNThreads(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::HTS_OPT_NTHREADS, i)
            }
            HtsFmtOption::HtsThreadPool(p) => {
                do_ptr!($f, $fp, HtsFmtOptionRaw::HTS_OPT_THREAD_POOL, p)
            }
            HtsFmtOption::HtsCacheSize(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::HTS_OPT_CACHE_SIZE, i)
            }
            HtsFmtOption::HtsBlockSize(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::HTS_OPT_BLOCK_SIZE, i)
            }
            HtsFmtOption::HtsCompressionLevel(i) => {
                do_val!($f, $fp, HtsFmtOptionRaw::HTS_OPT_COMPRESSION_LEVEL, i)
            }
            HtsFmtOption::HtsProfile(x) => do_val!($f, $fp, HtsFmtOptionRaw::HTS_OPT_PROFILE, x),
            HtsFmtOption::HtsFilter(s) => do_ptr!($f, $fp, HtsFmtOptionRaw::HTS_OPT_FILTER, s),
            HtsFmtOption::FastQCasava => do_none!($f, $fp, HtsFmtOptionRaw::FASTQ_OPT_CASAVA),
            HtsFmtOption::FastQRNum => do_none!($f, $fp, HtsFmtOptionRaw::FASTQ_OPT_RNUM),
            HtsFmtOption::FastQName2 => do_none!($f, $fp, HtsFmtOptionRaw::FASTQ_OPT_NAME2),
            HtsFmtOption::FastQAux(s) => do_ptr!($f, $fp, HtsFmtOptionRaw::FASTQ_OPT_AUX, s),
            HtsFmtOption::FastQBarcode(s) => {
                do_ptr!($f, $fp, HtsFmtOptionRaw::FASTQ_OPT_BARCODE, s)
            }
            _ => panic!("Unknown format option"),
        }
    };
}

#[macro_export]
macro_rules! do_val {
    ($f: ident, $fp:expr, $x:expr, $y:expr) => {
        $f($fp, $x, *$y)
    };
}

#[macro_export]
macro_rules! do_enum {
    ($f: ident, $fp:expr, $x:expr, $y:expr) => {
        $f($fp, $x, *$y as c_int)
    };
}

#[macro_export]
macro_rules! do_ptr {
    ($f: ident, $fp:expr, $x:expr, $y:expr) => {
        $f($fp, $x, $y.as_ptr())
    };
}

#[macro_export]
macro_rules! do_none {
    ($f: ident, $fp:expr, $x:expr) => {
        $f($fp, $x)
    };
}

#[macro_export]
macro_rules! do_bool {
    ($f: ident, $fp:expr, $x:expr, $y:expr) => {
        $f($fp, $x, if *$y { 1 } else { 0 })
    };
}

pub fn hts_file_set_opt(fp: &mut HtsFileRaw, opt: &mut HtsFmtOption) -> Result<(), HtsError> {
    if unsafe { set_opt!(hts_set_opt, fp, opt) } == 0 {
        Ok(())
    } else {
        Err(HtsError::OperationFailed)
    }
}

pub fn cram_file_set_opt(fd: &mut CramFdRaw, opt: &mut HtsFmtOption) -> Result<(), HtsError> {
    if unsafe { set_opt!(cram_set_option, fd, opt) } == 0 {
        Ok(())
    } else {
        Err(HtsError::OperationFailed)
    }
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hts_parse_format(format: *mut HtsFormat, str: *const c_char) -> c_int;
    fn hts_parse_opt_list(format: *mut HtsFormat, str: *const c_char) -> c_int;
    fn hts_format_file_extension(fmt: *const HtsFormat) -> *const c_char;
    fn hts_format_description(fmt: *const HtsFormat) -> *mut c_char;
    fn hts_set_opt(fp: *mut HtsFileRaw, opt: HtsFmtOptionRaw, ...) -> c_int;
    fn cram_set_option(fd: *mut CramFdRaw, opt: HtsFmtOptionRaw, ...) -> c_int;
}

impl HtsFormat {
    pub fn new() -> Self {
        Self::default()
    }
    /// Accepts a string file format (sam, bam, cram, vcf, bam) optionally
    /// followed by a comma separated list of key=value options and splits
    /// these up into the fields of htsFormat struct.
    pub fn parse_format(&mut self, s: &CStr) -> Result<(), HtsError> {
        if unsafe { hts_parse_format(self, s.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::ParseFormatOperationFailed)
        }
    }

    ///  Tokenise options as (key(=value)?,)*(key(=value)?)?
    ///  
    /// NB: No provision for ',' appearing in the value!
    pub fn parse_opt_list(&mut self, s: &CStr) -> Result<(), HtsError> {
        if unsafe { hts_parse_opt_list(self, s.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::ParseOptListOperationFailed)
        }
    }

    /// Returns a string containing the file format extension
    pub fn file_extension(&self) -> &CStr {
        unsafe { CStr::from_ptr(hts_format_file_extension(self)) }
    }

    ///  Get a human-readable description of the file format structure holding type, version, compression, etc.
    pub fn format_description(&self) -> CString {
        // hts_format_description returns a *mut c_char which was allocated using libc::malloc and
        // which needs to be freed after use by the caller.  If we return the pointer, therefore,
        // the memory will leak.  We therefor make a new CString by copying the returned C string, and
        // then we can free the original string, avoiding any leakage.
        let cs = unsafe { CStr::from_ptr(hts_format_description(self)) };
        let s = cs.to_owned();
        unsafe { libc::free(cs.as_ptr() as *mut c_void) }
        s
    }
    
    pub fn exact_format(&self) -> &HtsExactFormat {
        &self.format
    }
}
