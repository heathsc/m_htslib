use libc::{c_char, c_int};
use std::{
    ffi::CStr,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use super::{
    hts_error::HtsError,
    HtsPos,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum IdxFmt {
    Csi = 0,
    Bai,
    Tbi,
    Crai,
    Fai,
}

impl IdxFmt {
    pub fn from_int(i: c_int) -> Option<Self> {
        match i {
            0 => Some(Self::Csi),
            1 => Some(Self::Bai),
            2 => Some(Self::Tbi),
            _ => None,
        }
    }

    fn is_regular_fmt(&self) -> Result<(), HtsError> {
        if matches!(self, IdxFmt::Tbi | IdxFmt::Csi | IdxFmt::Bai) {
            Ok(())
        } else {
            Err(HtsError::InvalidIndexFormat)
        }
    }
}

pub const HTS_IDX_SAVE_REMOTE: c_int = 1;
pub const HTS_IDX_SILENT_FAIL: c_int = 2;

#[repr(C)]
pub struct HtsIdxRaw {
    _unused: [u8; 0],
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hts_idx_init(
        n: c_int,
        fmt: c_int,
        offset0: u64,
        min_shift: c_int,
        n_lvls: c_int,
    ) -> *mut HtsIdxRaw;
    fn hts_idx_destroy(idx: *mut HtsIdxRaw);
    fn hts_idx_push(
        idx: *mut HtsIdxRaw,
        tid: c_int,
        beg: HtsPos,
        end: HtsPos,
        offset: u64,
        is_mapped: c_int,
    ) -> c_int;
    fn hts_idx_finish(idx: *mut HtsIdxRaw, final_offset: u64) -> c_int;
    fn hts_idx_fmt(idx: *const HtsIdxRaw) -> c_int;
    fn hts_idx_tbi_name(idx: *mut HtsIdxRaw, tid: c_int, name: *const c_char) -> c_int;
    fn hts_idx_save(idx: *const HtsIdxRaw, fn_: *const c_char, fmt: c_int) -> c_int;
    fn hts_idx_save_as(
        idx: *const HtsIdxRaw,
        fn_: *const c_char,
        fnidx: *const c_char,
        fmt: c_int,
    ) -> c_int;
    fn hts_idx_load(fn_: *const c_char, fmt: c_int) -> *mut HtsIdxRaw;
    fn hts_idx_load2(fn_: *const c_char, fnidx: *const c_char) -> *mut HtsIdxRaw;
    fn hts_idx_load3(
        fn_: *const c_char,
        iname: *const c_char,
        fmt: c_int,
        flags: c_int,
    ) -> *mut HtsIdxRaw;
    fn hts_idx_get_meta(idx: *const HtsIdxRaw, l_meta: *mut u32) -> *const u8;
    fn hts_idx_set_meta(idx: *mut HtsIdxRaw, l_meta: u32, meta: *const u8, is_copy: c_int)
    -> c_int;
    fn hts_idx_get_stat(
        idx: *const HtsIdxRaw,
        tid: c_int,
        mapped: *mut u64,
        unmapped: *mut u64,
    ) -> c_int;
    fn hts_idx_get_n_no_coor(idx: *const HtsIdxRaw) -> u64;
    fn hts_idx_nseq(idx: *const HtsIdxRaw) -> c_int;
}

impl HtsIdxRaw {
    /// Push an index entry
    ///
    /// `tid` - Target id
    ///
    /// `beg` - Range start (zero-based)
    ///
    /// `end` - Range end (zero-based, half-open)
    ///
    /// `offset` - File offset
    ///
    /// `is_mapped` - Range corresponds to a mapped read
    #[inline]
    pub fn push(
        &mut self,
        tid: c_int,
        beg: HtsPos,
        end: HtsPos,
        offset: u64,
        is_mapped: bool,
    ) -> Result<(), HtsError> {
        if unsafe { hts_idx_push(self, tid, beg, end, offset, if is_mapped { 1 } else { 0 }) } == 0
        {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    /// Finish building index
    ///
    /// `final_offset` - Last file offset
    #[inline]
    pub fn finish(&mut self, final_offset: u64) -> Result<(), HtsError> {
        if unsafe { hts_idx_finish(self, final_offset) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OperationFailed)
        }
    }

    /// Returns format of index
    #[inline]
    pub fn fmt(&self) -> IdxFmt {
        IdxFmt::from_int(unsafe { hts_idx_fmt(self) }).expect("Unknown index format type")
    }

    /// Add name to TBI index meta-data
    ///
    /// `tid` - Target identifier
    ///
    /// `name` - Target name
    ///
    /// Returns number of names in name list
    #[inline]
    pub fn tbi_name(&mut self, tid: c_int, name: &CStr) -> Result<usize, HtsError> {
        if unsafe { hts_idx_fmt(self) } == IdxFmt::Tbi as c_int {
            match unsafe { hts_idx_tbi_name(self, tid, name.as_ptr()) } {
                ..=-1 => Err(HtsError::OperationFailed),
                l => Ok(l as usize),
            }
        } else {
            Err(HtsError::InvalidIndexFormat)
        }
    }

    /// Save index to a file
    ///
    /// `fname` - Input BAM/BCF/etc filename, to which .bai/.csi/.tbi will be added
    ///
    /// `fmt` - Only Bai | Csi | Tbi are allowed
    #[inline]
    pub fn save(&self, fname: &CStr, fmt: IdxFmt) -> Result<(), HtsError> {
        fmt.is_regular_fmt().and_then(|_| {
            if unsafe { hts_idx_save(self, fname.as_ptr(), fmt as c_int) } == 0 {
                Ok(())
            } else {
                Err(HtsError::IOError)
            }
        })
    }

    /// Save index to a file
    ///
    /// `fname` - Input BAM/BCF/etc filename
    ///
    /// `idx_name` - Name for index file.  If [None] then .bai/.csi/.tbi will be added to `fname`
    ///
    /// `fmt` - Only Bai | Csi | Tbi are allowed
    #[inline]
    pub fn save_as(
        &self,
        fname: &CStr,
        idx_name: Option<&CStr>,
        fmt: IdxFmt,
    ) -> Result<(), HtsError> {
        fmt.is_regular_fmt().and_then(|_| {
            let iname = idx_name.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
            if unsafe { hts_idx_save_as(self, fname.as_ptr(), iname, fmt as c_int) } == 0 {
                Ok(())
            } else {
                Err(HtsError::IOError)
            }
        })
    }

    /// Get extra index meta-data
    ///
    /// Indexes (both .tbi and .csi) made by tabix include extra data about the indexed file.
    /// Note that the data is stored exactly as it is in the index.  Callers need to interpret
    /// the results themselves, including knowing what sort of data to expect, byte swapping etc.
    #[inline]
    pub fn get_meta(&self) -> Option<&[u8]> {
        let mut l: u32 = 0;
        let p = unsafe { hts_idx_get_meta(self, &mut l) };
        if p.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(p, l as usize) })
        }
    }

    /// Set extra index meta-data
    #[inline]
    pub fn set_meta(&mut self, meta: &[u8]) -> Result<(), HtsError> {
        let l = meta.len();
        assert!(l < u32::MAX as usize, "Meta data too large");

        // If meta ends with a 0 character, reduce l by 1.  This is because hts_idx_set_meta adds a 0 when it
        // makes a copy of the data (to avoid problems with strlen()), so this avoids increasing the size of
        // the metadata needlessly.
        let l1 = if meta.last().map(|x| *x == 0).unwrap_or(false) {
            l - 1
        } else {
            l
        } as u32;

        // We set the is_copy flag so that `meta` is copied by [hts_idx_set_meta], otherwise free() will
        // be called on `meta` when [hts_idx_destroy] is called which would be bad.
        if unsafe { hts_idx_set_meta(self, l1, meta.as_ptr(), 1) } == 0 {
            Ok(())
        } else {
            Err(HtsError::OutOfMemory)
        }
    }

    /// Get number of mapped and unmapped reads from an index
    ///
    /// `tid` - Target ID
    ///
    /// On success, returns a tuple (u64, u64) with the mapped and unmapped read counts
    ///
    /// BAI and CSI indexes store information on the number of reads for each target
    /// that were mapped or unmapped (unmapped reads will generally hav a paired read
    /// that is mapped to the target).  This function returns this information if it
    /// is available. If this is called on an index type that is not BAI or CSI,
    /// [HtsError::StatsUnavailable] is returned
    #[inline]
    pub fn get_stat(&self, tid: c_int) -> Result<(u64, u64), HtsError> {
        if matches!(self.fmt(), IdxFmt::Bai | IdxFmt::Csi) {
            let mut mapped: u64 = 0;
            let mut unmapped: u64 = 0;
            if unsafe { hts_idx_get_stat(self, tid, &mut mapped, &mut unmapped) } == 0 {
                return Ok((mapped, unmapped));
            }
        }
        Err(HtsError::StatsUnavailable)
    }

    /// Return the number of unplaced reads from an index
    ///
    /// Unplaced reads are not linked to any reference (e.g. RNAME is '*' in SAM)
    ///
    /// Information only available for BAI and CSI indexes
    #[inline]
    pub fn get_n_no_coor(&self) -> Result<u64, HtsError> {
        if matches!(self.fmt(), IdxFmt::Bai | IdxFmt::Csi) {
            Ok(unsafe { hts_idx_get_n_no_coor(self) })
        } else {
            Err(HtsError::StatsUnavailable)
        }
    }

    /// Returns the number of targets in an index
    #[inline]
    pub fn nseq(&self) -> usize {
        let n = unsafe { hts_idx_nseq(self) };
        assert!(n >= 0);
        n as usize
    }
}

pub struct HtsIdx {
    inner: NonNull<HtsIdxRaw>,
}

impl Deref for HtsIdx {
    type Target = HtsIdxRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for HtsIdx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

unsafe impl Send for HtsIdx {}
unsafe impl Sync for HtsIdx {}

impl Drop for HtsIdx {
    fn drop(&mut self) {
        unsafe { hts_idx_destroy(self.deref_mut()) };
    }
}

impl HtsIdx {
    /// Create a BAI/CSI/TBI type index structure
    ///
    /// `n` - Initial number of targets
    ///
    /// `fmt` - Desired format.  Note that only Bai | Csi | Tbi are valid
    ///
    /// `offset0` - Initial file offset
    ///
    /// `min_shift` - Number of bits for the minimal interval
    ///
    /// `n_levels` - Number of levels in the binning index
    pub fn init(
        n: c_int,
        fmt: IdxFmt,
        offset0: u64,
        min_shift: c_int,
        n_levels: c_int,
    ) -> Result<Self, HtsError> {
        fmt.is_regular_fmt().and_then(|_| {
            Self::mk_hts_idx(
                unsafe { hts_idx_init(n, fmt as c_int, offset0, min_shift, n_levels) },
                HtsError::IndexInitFailed,
            )
        })
    }

    /// Load an index file
    ///
    /// `fname` - BAM/BCF/etc filename, to which .bai/.csi/.tbi will be added or the extension
    /// substituted, to search for an existing index file. In case of a non-standard naming, the file
    /// name can include the name of the index file delimited with [crate::hts::HTS_IDX_DELIM].
    ///
    /// `fmt`- Desired format. Note that only Bai | Csi | Tbi are valid
    ///
    /// If `fname` contains the string "##idx##" ([crate::hts::HTS_IDX_DELIM]), the part before the delimiter will be
    /// used as the name of the data file and the part after it will be used as the name of the index.
    /// Otherwise, this function tries to work out the index name as follows:
    ///
    /// It will try appending the appropriate suffix for fmt to `fname`.  If the index is not found it
    /// will then try substituting the format appropriate suffix for the existing suffix (e.g., .bam)
    ///
    /// If the index file is remote (served over a protocol like https), first a check is made to see
    /// if a locally cached copy is available.  This is done for all of the possible names listed
    /// above.  If a cached copy is not available then the index will be downloaded and stored in the
    /// current working directory, with the same name as the remote index.
    ///
    /// Equivalent to HtsIdx::load3(fn, [None], fmt, [HTS_IDX_SAVE_REMOTE]);
    pub fn load(fname: &CStr, fmt: IdxFmt) -> Result<Self, HtsError> {
        fmt.is_regular_fmt().and_then(|_| {
            Self::mk_hts_idx(
                unsafe { hts_idx_load(fname.as_ptr(), fmt as c_int) },
                HtsError::IOError,
            )
        })
    }

    /// Load a specific index file
    ///
    /// `fname` - Input BAM/BCF/etc filename
    ///
    /// `idx_name` -  The input index filename
    ///
    /// Equivalent to HtsIdx::load3(fn, Some(fname), [IdxFmt::Csi], 0);
    ///
    /// This function will not attempt to save index files locally.
    pub fn load2(fname: &CStr, idx_name: &CStr) -> Result<Self, HtsError> {
        Self::mk_hts_idx(
            unsafe { hts_idx_load2(fname.as_ptr(), idx_name.as_ptr()) },
            HtsError::IOError,
        )
    }

    /// Load a specific index file
    ///
    /// 'fname' - Input BAM/BCF/etc filename
    ///
    /// `idx_name` - The input index filename
    ///
    /// `fmt` - Desired format. Note that only Bai | Csi | Tbi are valid, and that if `idx_name` is
    /// not [None] then `fmt` is ignored.
    ///
    /// `flags` - Flags to alter behaviour (see description)
    ///
    /// If `idx_name` is [None], the index name will be derived from `fname` in the same way
    /// as [HtsIdx::load()].
    ///
    /// The `flags` parameter can be set to a combination of the following values:
    ///
    ///   HTS_IDX_SAVE_REMOTE - Save a local copy of any remote indexes
    ///
    ///   HTS_IDX_SILENT_FAIL - Fail silently if the index is not present
    pub fn load3(
        fname: &CStr,
        idx_name: Option<&CStr>,
        fmt: IdxFmt,
        flags: c_int,
    ) -> Result<Self, HtsError> {
        // If idx_name.is_some() them fmt is ignored, so set it to a valid value
        let fmt = if idx_name.is_some() { IdxFmt::Bai } else { fmt };

        fmt.is_regular_fmt().and_then(|_| {
            let iname = idx_name.map(|s| s.as_ptr()).unwrap_or_else(std::ptr::null);
            Self::mk_hts_idx(
                unsafe { hts_idx_load3(fname.as_ptr(), iname, fmt as c_int, flags) },
                HtsError::IOError,
            )
        })
    }

    pub(crate) fn mk_hts_idx(p: *mut HtsIdxRaw, err: HtsError) -> Result<Self, HtsError> {
        match NonNull::new(p) {
            None => Err(err),
            Some(p) => Ok(Self {
                inner: p,
            }),
        }
    }
    
    pub fn mk_iterator<H>(&self, hdr: H) {
        
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read() {
        let idx = HtsIdx::load(c"test/index.bam", IdxFmt::Bai).unwrap();
        assert_eq!(idx.nseq(), 7);
        assert_eq!(idx.get_n_no_coor().unwrap(), 50);
        let (map, unmap) = idx.get_stat(1).unwrap();
        assert!(map == 28 && unmap == 0);
    }

    #[test]
    fn read2() {
        let idx = HtsIdx::load2(c"test/index.sam.gz", c"test/index.sam.gz.csi").unwrap();
        assert_eq!(idx.nseq(), 7);
        assert_eq!(idx.get_n_no_coor().unwrap(), 50);
        let (map, unmap) = idx.get_stat(1).unwrap();
        assert!(map == 28 && unmap == 0);
    }

    #[test]
    fn vcf_read() {
        let idx = HtsIdx::load(c"test/index.vcf.gz", IdxFmt::Tbi).unwrap();
        assert_eq!(idx.nseq(), 3);
        assert_eq!(idx.get_n_no_coor().unwrap(), 0);
        let (map, unmap) = idx.get_stat(1).unwrap();
        eprintln!("{map} {unmap}");
        assert!(map == 219 && unmap == 0);
    }
}
