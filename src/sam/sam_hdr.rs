use libc::{c_char, c_int, c_void, size_t};
use std::{
    ffi::{CStr, CString},
    fmt::{self, Formatter},
    ops::{Deref, DerefMut},
    ptr,
    sync::{
        RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicU32, Ordering},
    },
};

use super::sam_error::SamError;
use crate::{
    cstr_len, from_c,
    hts::{
        htsfile::{HtsFile, HtsFileRaw},
        traits::*,
    },
    kstring::KString,
};

#[repr(C)]
pub struct SamHrecsRaw {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct SamHdrRaw {
    n_targets: i32,
    ignore_sam_err: i32,
    l_text: size_t,
    cigar_tab: *const i8,
    target_name: *mut *mut c_char,
    text: *mut c_char,
    sdict: *mut c_void,
    hrecs: *mut SamHrecsRaw,
    ref_count: AtomicU32,
}

pub struct SamHdrTagValue<'a> {
    tag: [char; 2],
    value: &'a str,
}

impl<'a> SamHdrTagValue<'a> {
    pub fn new_tag(s: &str, value: &'a str) -> Result<Self, SamError> {
        if s.len() != 2 {
            Err(SamError::IllegalTagLength)
        } else {
            let mut it = s.chars();
            let t1 = it.next().unwrap();
            let t2 = it.next().unwrap();
            let tag = [t1, t2];
            Ok(Self { tag, value })
        }
    }

    pub fn new(tag: [char; 2], value: &'a str) -> Self {
        Self { tag, value }
    }

    pub fn tag(&self) -> [char; 2] {
        self.tag
    }

    pub fn value(&self) -> &str {
        self.value
    }

    pub fn value_as_cstring(&self) -> Result<CString, SamError> {
        CString::new(self.value).map_err(|_| SamError::NullInTagValue)
    }
}

impl fmt::Display for SamHdrTagValue<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}:{}", self.tag[0], self.tag[1], self.value)
    }
}

fn write_tag_value_slice(v: &[SamHdrTagValue], f: &mut Formatter<'_>) -> fmt::Result {
    for t in v {
        write!(f, "\t{t}")?;
    }
    Ok(())
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SamHdrType {
    Hd,
    Sq,
    Rg,
    Pg,
}

impl SamHdrType {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::Sq => "SQ",
            Self::Rg => "RG",
            Self::Pg => "PG",
        }
    }

    pub fn to_cstr(&self) -> &'static CStr {
        match self {
            Self::Hd => c"HD",
            Self::Sq => c"SQ",
            Self::Rg => c"RG",
            Self::Pg => c"PG",
        }
    }
}

impl fmt::Display for SamHdrType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

pub enum SamHdrLine<'a> {
    Line(SamHdrType, Vec<SamHdrTagValue<'a>>),
    Comment(&'a str),
}

impl<'a> SamHdrLine<'a> {
    pub fn line(ty: SamHdrType) -> Self {
        Self::Line(ty, Vec::new())
    }

    pub fn comment(s: &'a str) -> Self {
        Self::Comment(s)
    }

    pub fn push(&mut self, tv: SamHdrTagValue<'a>) {
        match self {
            Self::Line(_, v) => v.push(tv),
            Self::Comment(_) => panic!("Cannot add tag to comment line"),
        }
    }
}

impl fmt::Display for SamHdrLine<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment(s) => write!(f, "@CO\t{s}"),
            Self::Line(t, v) => {
                write!(f, "@{t}")?;
                write_tag_value_slice(v, f)
            }
        }
    }
}

#[macro_export]
macro_rules! sam_hdr_line {
    ( "HD", $( $t: expr, $v:expr ),* ) => {{
        let mut tmp_line = $crate::sam::SamHdrLine::line($crate::sam::SamHdrType::Hd);
        $(
           tmp_line.push($crate::sam::SamHdrTagValue::new_tag($t, $v)?);
        )*
        let tl: Result<$crate::sam::SamHdrLine, $crate::SamError> = Ok(tmp_line);
        tl
    }};
    ( "SQ", $( $t: expr, $v:expr ),* ) => {{
        let mut tmp_line = $crate::sam::SamHdrLine::line($crate::sam::SamHdrType::Sq);
        $(
           tmp_line.push($crate::sam::SamHdrTagValue::new_tag($t, $v)?);
        )*
        let tl: Result<$crate::sam::SamHdrLine, $crate::SamError> = Ok(tmp_line);
        tl
    }};
    ( "RG", $( $t: expr, $v:expr ),* ) => {{
        let mut tmp_line = $crate::sam::SamHdrLine::line($crate::sam::SamHdrType::Rg);
        $(
           tmp_line.push($crate::sam::SamHdrTagValue::new_tag($t, $v)?);
        )*
        let tl: Result<$crate::sam::SamHdrLine, $crate::SamError> = Ok(tmp_line);
        tl
    }};
    ( "PG", $( $t: expr, $v:expr ),* ) => {{
        let mut tmp_line = $crate::sam::SamHdrLine::line($crate::sam::SamHdrType::Pg);
        $(
           tmp_line.push($crate::sam::SamHdrTagValue::new_tag($t, $v)?);
        )*
        let tl: Result<$crate::sam::SamHdrLine, $crate::SamError> = Ok(tmp_line);
        tl
    }};
    ( "CO", $s:expr ) => {
        let tl: Result<$crate::sam::SamHdrLine, $crate::SamError> = Ok(SamHdrLine::comment($s));
        tl
    };
}

#[link(name = "hts")]
unsafe extern "C" {
    fn sam_hdr_read(fp_: *mut HtsFileRaw) -> *mut SamHdrRaw;
    fn sam_hdr_write(fp_: *mut HtsFileRaw, hd_: *const SamHdrRaw) -> c_int;
    fn sam_hdr_init() -> *mut SamHdrRaw;
    fn sam_hdr_destroy(hd_: *mut SamHdrRaw);
    fn sam_hdr_dup(hd_: *const SamHdrRaw) -> *mut SamHdrRaw;
    fn sam_hdr_parse(len_: size_t, text_: *const c_char) -> *mut SamHdrRaw;
    fn sam_hdr_length(hd: *mut SamHdrRaw) -> size_t;
    fn sam_hdr_add_lines(hd_: *mut SamHdrRaw, lines_: *const c_char, len_: size_t) -> c_int;
    fn sam_hdr_remove_except(
        hd_: *mut SamHdrRaw,
        type_: *const c_char,
        id_key_: *const c_char,
        id_value_: *const c_char,
    ) -> c_int;
    fn sam_hdr_nref(hd_: *const SamHdrRaw) -> c_int;
    fn sam_hdr_tid2name(hd_: *const SamHdrRaw, i_: c_int) -> *const c_char;
    fn sam_hdr_tid2len(hd_: *const SamHdrRaw, i_: c_int) -> c_int;
    fn sam_hdr_name2tid(hd_: *mut SamHdrRaw, nm_: *const c_char) -> c_int;
    fn sam_hdr_str(hd_: *mut SamHdrRaw) -> *const c_char;
    fn sam_hdr_change_HD(hd: *mut SamHdrRaw, key: *const c_char, val: *const c_char);
    fn sam_hdr_find_line_id(
        hd: *mut SamHdrRaw,
        type_: *const c_char,
        id_key: *const c_char,
        id_val: *const c_char,
        ks: *mut KString,
    ) -> c_int;
    fn sam_hdr_find_line_pos(
        hd: *mut SamHdrRaw,
        type_: *const c_char,
        pos: c_int,
        ks: *mut KString,
    ) -> c_int;
    fn sam_hdr_remove_line_id(
        hd: *mut SamHdrRaw,
        typ: *const c_char,
        key: *const c_char,
        val: *const c_char,
    ) -> c_int;
    fn sam_hdr_remove_line_pos(hd: *mut SamHdrRaw, typ: *const c_char, pos: c_int) -> c_int;

    fn sam_hdr_find_tag_id(
        hd: *mut SamHdrRaw,
        type_: *const c_char,
        id_key: *const c_char,
        id_val: *const c_char,
        key: *const c_char,
        ks: *mut KString,
    ) -> c_int;
    fn sam_hdr_find_tag_pos(
        hd: *mut SamHdrRaw,
        type_: *const c_char,
        pos: c_int,
        key: *const c_char,
        ks: *mut KString,
    ) -> c_int;
    fn sam_hdr_count_lines(hd: *mut SamHdrRaw, type_: *const c_char) -> c_int;
    // fn sam_hdr_pg_id(hd: *mut SamHdrRaw, name: *const char) -> *const c_char;
    // fn sam_hdr_add_pg(hd: *mut SamHdrRaw, name: *const c_char, ...) -> c_int;
}

impl SamHdrRaw {
    /// Writes the header to `hts_file`
    fn write(&self, hts_file: &mut HtsFileRaw) -> Result<(), SamError> {
        match unsafe { sam_hdr_write(hts_file as *mut HtsFileRaw, self) } {
            0 => Ok(()),
            _ => Err(SamError::FailedHeaderWrite),
        }
    }

    /// Returns the number of references in hte header
    #[inline]
    fn nref(&self) -> usize {
        let l = unsafe { sam_hdr_nref(self) };
        assert!(l >= 0);
        l as usize
    }

    #[inline]
    fn check_idx(&self, i: usize) -> bool {
        i < self.nref()
    }

    /// Gets the name of the sequence corresponding to a target index
    #[inline]
    fn tid2name(&self, i: usize) -> Option<&CStr> {
        if self.check_idx(i) {
            from_c(unsafe { sam_hdr_tid2name(self, i as c_int) })
        } else {
            None
        }
    }

    /// Gets the length of the sequence corresponding to a target index
    fn tid2len(&self, i: usize) -> Option<usize> {
        if self.check_idx(i) {
            let len = unsafe { sam_hdr_tid2len(self, i as c_int) };
            assert!(len >= 0);
            Some(len as usize)
        } else {
            None
        }
    }

    /// Gets the target index corresponding to a sequence name (if it exists in the header).
    /// If self.hrecs is null then the header records are rebuilt
    #[inline]
    fn name2tid_rebuild(&mut self, cname: &CStr) -> Result<usize, SamError> {
        Self::check_tid(unsafe { sam_hdr_name2tid(self, cname.as_ptr()) })
    }

    fn check_tid(x: c_int) -> Result<usize, SamError> {
        if x < -1 {
            Err(SamError::HeaderParseFailed)
        } else if x < 0 {
            Err(SamError::UnknownReference)
        } else {
            Ok(x as usize)
        }
    }

    #[inline]
    fn name2tid(&self, cname: &CStr) -> Result<usize, SamError> {
        if self.hrecs.is_null() {
            Err(SamError::HeaderNotParsed)
        } else {
            // if self.hrecs is not null, then sam_hdr_name2tid does not mutate the header
            // but the signature requires a mut pointer so we will make one (yeuch!)
            let p = self as *const SamHdrRaw as *mut SamHdrRaw;
            Self::check_tid(unsafe { sam_hdr_name2tid(p, cname.as_ptr()) })
        }
    }

    /// Returns the current header txt.  Can be invalidated by a call to another header function
    #[inline]
    fn text(&mut self) -> Option<&CStr> {
        from_c(unsafe { sam_hdr_str(self) })
    }

    /// Returns length of header text
    #[inline]
    fn length(&mut self) -> Result<usize, SamError> {
        match unsafe { sam_hdr_length(self) } {
            size_t::MAX => Err(SamError::OperationFailed),
            l => Ok(l),
        }
    }
}

/// Header from SAM/BAM/CRAM file
///
/// This is a Wrapper around a pointer to the htslib sam_hdr_t (`SamHdrRaw`).
/// There are many methods provided by htslib for interacting with the
/// header structure, both reading and writing. There are, however, several
/// common methods which are logically read only, but in fact require a a mutable
/// pointer. This is because the header maintains hashes to enable fast access to
/// information such as sequence names etc., and these are updated when necessary
/// after modification of the header. Therefore, even to lookup the name of a
/// sequence can potential require rebuilding the hashes, and therefore a mutable pointer.
///
/// To avoid requiring a mutable reference to SamHdr for methos which are logically read only,
/// we will use interior mutability with a RwLock. For methods that require a shared pointer
/// we can get this from a read() call, for methods that are modifying the header (adding
/// lines etc.), we will still require a mutable reference, while for methods that are not
/// altering the header itself but potentially might rebuild the hashes we first check to see
/// if a rebuild is necessary (which requires obtaining a read lock). If so, then we drop
/// the read lock and obtain a write lock, rebuild the hashes and call the method. Otherwise
/// we continue with the read lock we already have. Note that ifwe have a mutable reference
/// initially, we can use get_mut() rather than write() as we don't need a lock if we already
/// have a mutable reference.
///
/// In this way the user will need a mutable reference to the header if they want to modify
/// the basic structure, but only a shared reference if they are just reading from an existing
/// file (which makes more intuitive sense - at least to me)
///
pub struct SamHdr {
    // We use interior mutability to lookup sequence names -> id
    // maps as we require a mutable reference for this in the
    // case that the required cache has not been filled, but
    // generally a shared reference is sufficient
    //
    // The pointer to SamHdrRaw will always be valid
    inner: RwLock<*mut SamHdrRaw>,
}

impl Clone for SamHdr {
    fn clone(&self) -> Self {
        Self::try_dup(self).expect("Could not duplicate SamHdr")
    }
}

unsafe impl Send for SamHdr {}
unsafe impl Sync for SamHdr {}

impl Drop for SamHdr {
    fn drop(&mut self) {
        let hdr = match self.inner.get_mut() {
            Ok(p) => *p,
            Err(e) => *e.into_inner(),
        };
        let h = unsafe { &mut *hdr };
        if h.ref_count.fetch_sub(1, Ordering::Relaxed) == u32::MAX {
            unsafe { sam_hdr_destroy(hdr) };
        }
    }
}

impl Default for SamHdr {
    fn default() -> Self {
        Self::try_init().expect("Could not allocate new SamHdr")
    }
}
impl SamHdr {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn try_init() -> Result<Self, SamError> {
        Self::make_sam_hdr(unsafe { sam_hdr_init() }, SamError::OutOfMemory)
    }

    pub fn try_dup(&self) -> Result<Self, SamError> {
        let hd = self.inner.read().unwrap();
        Self::make_sam_hdr(unsafe { sam_hdr_dup(*hd.deref()) }, SamError::OutOfMemory)
    }

    pub fn parse(text: &CStr) -> Result<Self, SamError> {
        Self::make_sam_hdr(
            unsafe { sam_hdr_parse(cstr_len(text) as size_t, text.as_ptr()) },
            SamError::HeaderParseFailed,
        )
    }

    pub fn read(hts_file: &mut HtsFile<'_>) -> Result<Self, SamError> {
        Self::make_sam_hdr(
            unsafe { sam_hdr_read(hts_file.deref_mut() as *mut HtsFileRaw) },
            SamError::FailedHeaderRead,
        )
    }

    fn make_sam_hdr(hdr: *mut SamHdrRaw, e: SamError) -> Result<Self, SamError> {
        if hdr.is_null() {
            Err(e)
        } else {
            Ok(Self {
                inner: RwLock::new(hdr),
            })
        }
    }

    /// Get a shared reference from inner. We return a read guard along with the
    /// reference so that the guard remains in scope and the reference stays valid
    #[inline]
    fn get_ref(&self) -> (RwLockReadGuard<*mut SamHdrRaw>, &SamHdrRaw) {
        let g = self.inner.read().unwrap();
        let hdr = unsafe { &*(*g.deref()) };
        (g, hdr)
    }

    /// Release a read lock and obtain a write lock, allowing the generation of a
    /// mutable reference from inner. We return a write guard along with the
    /// reference so that the guard remains in scope and the reference stays valid
    #[inline]
    fn drop_and_get_mut(
        &self,
        g: RwLockReadGuard<*mut SamHdrRaw>,
    ) -> (RwLockWriteGuard<*mut SamHdrRaw>, &mut SamHdrRaw) {
        drop(g);
        self.get_mut()
    }

    /// Get a mutable reference from inner. We return a writeguard along with the
    /// reference so that the guard remains in scope and the reference stays valid
    #[inline]
    pub (in crate::sam) fn get_mut(&self) -> (RwLockWriteGuard<*mut SamHdrRaw>, &mut SamHdrRaw) {
        let g = self.inner.write().unwrap();        
        let hdr = unsafe { &mut *(*g.deref()) };
        (g, hdr)
    }

    /// Get a *mut pointer from inner. As we have a mut reference for self,
    /// we can directly obtain a mut pointer without requiring a lock as
    /// exclusive access is statically enforced
    #[inline]
    fn as_mut(&mut self) -> &mut SamHdrRaw {
        unsafe { &mut *(*self.inner.get_mut().unwrap()) }
    }

    /// Writes the header to `hts_file`
    #[inline]
    pub fn write(&self, hts_file: &mut HtsFileRaw) -> Result<(), SamError> {
        self.get_ref().1.write(hts_file)
    }

    /// Returns the number of references in sam header
    #[inline]
    pub fn nref(&self) -> usize {
        self.get_ref().1.nref()
    }

    /// Gets the name of the sequence corresponding to a target index
    #[inline]
    pub fn tid2name(&self, i: usize) -> Option<&CStr> {
        self.get_ref().1.tid2name(i)
    }

    /// Gets the length of the sequence corresponding to a target index
    #[inline]
    pub fn tid2len(&self, i: usize) -> Option<usize> {
        self.get_ref().1.tid2len(i)
    }

    /// Get internal ID corresponding to a sequence name
    #[inline]
    pub fn name2tid(&self, cname: &CStr) -> Result<usize, SamError> {
        let (g, hdr) = self.get_ref();
        if hdr.hrecs.is_null() {
            self.drop_and_get_mut(g).1.name2tid_rebuild(cname)
        } else {
            hdr.name2tid(cname)
        }
    }

    /// Returns the current header txt.
    #[inline]
    pub fn text(&self) -> Option<&CStr> {
        // The htslib function *always* rebuilds the header so we will
        // unconditionally obtain a mutable pointer
        // 
    
        self.get_mut().1.text()
    }

    /// Returns length of header text
    #[inline]
    pub fn length(&self) -> Result<usize, SamError> {
        // The htslib function *always* rebuilds the header so we will
        // unconditionally obtain a mutable pointer
        self.get_mut().1.length()
    }

    /// Add SAM header record(s) with optional new line.  If multiple lines are present (separated by newlines)
    /// then they will be added in order
    #[inline]
    pub fn add_lines(&mut self, lines: &CStr) -> Result<(), SamError> {
        match unsafe { sam_hdr_add_lines(self.as_mut(), lines.as_ptr(), cstr_len(lines) as size_t) }
        {
            0 => Ok(()),
            _ => Err(SamError::FailedAddHeaderLine),
        }
    }

    pub fn add_line(&mut self, line: &SamHdrLine) -> Result<(), SamError> {
        let nl = format!("{line}");
        let cs = CString::new(nl.as_str()).map_err(|_| SamError::IllegalHeaderChars)?;
        self.add_lines(&cs)
    }

    pub fn remove_except(
        &mut self,
        ln_type: &SamHdrType,
        id: Option<SamHdrTagValue>,
    ) -> Result<(), SamError> {
        match if let Some(tv) = id {
            unsafe {
                sam_hdr_remove_except(
                    self.as_mut(),
                    ln_type.to_cstr().as_ptr(),
                    tv.tag().as_ptr() as *const c_char,
                    tv.value().as_ptr() as *const c_char,
                )
            }
        } else {
            unsafe {
                sam_hdr_remove_except(
                    self.as_mut(),
                    ln_type.to_cstr().as_ptr(),
                    ptr::null(),
                    ptr::null(),
                )
            }
        } {
            0 => Ok(()),
            _ => Err(SamError::FailedRemoveHeaderLines),
        }
    }

    pub fn remove(&mut self, ln_type: &SamHdrType) -> Result<(), SamError> {
        self.remove_except(ln_type, None)
    }

    pub fn change_hd(&mut self, key: &CStr, val: Option<&CStr>) {
        let val = if let Some(v) = val {
            v.as_ptr()
        } else {
            ptr::null::<c_char>()
        };
        unsafe { sam_hdr_change_HD(self.as_mut(), key.as_ptr(), val) }
    }

    pub fn find_line_id(&self, typ: &CStr, id_key: &CStr, id_val: &CStr) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_line_id(
                self.get_mut().1,
                typ.as_ptr(),
                id_key.as_ptr(),
                id_val.as_ptr(),
                &mut ks,
            ) == 0
        } {
            Some(ks)
        } else {
            None
        }
    }

    pub fn find_line_pos(&self, typ: &CStr, pos: usize) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_line_pos(self.get_mut().1, typ.as_ptr(), pos as c_int, &mut ks) == 0
        } {
            Some(ks)
        } else {
            None
        }
    }

    pub fn remove_line_pos(&mut self, typ: &CStr, pos: usize) -> Result<(), SamError> {
        if unsafe { sam_hdr_remove_line_pos(self.as_mut(), typ.as_ptr(), pos as c_int) == 0 } {
            Ok(())
        } else {
            Err(SamError::OperationFailed)
        }
    }

    pub fn remove_line_id(
        &mut self,
        typ: &CStr,
        id_key: &CStr,
        id_val: &CStr,
    ) -> Result<(), SamError> {
        if unsafe {
            sam_hdr_remove_line_id(
                self.as_mut(),
                typ.as_ptr(),
                id_key.as_ptr(),
                id_val.as_ptr(),
            ) == 0
        } {
            Ok(())
        } else {
            Err(SamError::OperationFailed)
        }
    }

    pub fn find_tag_id(
        &self,
        typ: &CStr,
        id_key: &CStr,
        id_val: &CStr,
        key: &CStr,
    ) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_tag_id(
                self.get_mut().1,
                typ.as_ptr(),
                id_key.as_ptr(),
                id_val.as_ptr(),
                key.as_ptr(),
                &mut ks,
            ) == 0
        } {
            Some(ks)
        } else {
            None
        }
    }

    pub fn find_tag_pos(&self, typ: &CStr, pos: usize, key: &CStr) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_tag_pos(
                self.get_mut().1,
                typ.as_ptr(),
                pos as c_int,
                key.as_ptr(),
                &mut ks,
            ) == 0
        } {
            Some(ks)
        } else {
            None
        }
    }

    pub fn count_lines(&self, typ: &CStr) -> Option<usize> {
        let n = unsafe { sam_hdr_count_lines(self.get_mut().1, typ.as_ptr()) };
        if n >= 0 { Some(n as usize) } else { None }
    }
}

impl HdrType for SamHdr {
    fn hdr_type(&self) -> HtsHdrType {
        HtsHdrType::Sam
    }
}

impl SeqId for SamHdr {
    #[inline]
    fn seq_id(&self, s: &CStr) -> Option<usize> {
        self.name2tid(s).ok()
    }
}

impl IdMap for SamHdr {
    #[inline]
    fn seq_name(&self, i: usize) -> Option<&CStr> {
        self.tid2name(i)
    }

    #[inline]
    fn seq_len(&self, i: usize) -> Option<usize> {
        self.tid2len(i)
    }

    #[inline]
    fn num_seqs(&self) -> usize {
        self.nref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HtsError,
        hts::{
            HtsFile,
            traits::{IdMap, SeqId},
        },
    };

    #[test]
    fn construct() -> Result<(), SamError> {
        // Make empty header structure and add a line to it
        let mut hdr = SamHdr::new();
        hdr.add_lines(c"@HD\tVN:1.6\tSO:coordinate")?;
        assert_eq!(hdr.length().unwrap(), 25);
        let nl = sam_hdr_line!(
            "SQ",
            "SN",
            "CHROMOSOME_I",
            "LN",
            "1009800",
            "M5",
            "8ede36131e0dbf3417807e48f77f3ebd"
        )?;
        hdr.add_line(&nl)?;
        let cs = hdr.text().unwrap();
        let l = cstr_len(cs);
        assert_eq!(hdr.length().unwrap(), l);
        assert_eq!(l, 92);
        Ok(())
    }

    #[test]
    fn read_hdr() -> Result<(), HtsError> {
        let mut samfile = HtsFile::open(c"test/realn01.sam", c"r")?;
        let hdr = SamHdr::read(&mut samfile)?;
        let ctg0 = c"000000F";

        assert_eq!(hdr.tid2name(0), Some(ctg0));
        assert_eq!(hdr.tid2name(1), None);
        assert_eq!(hdr.tid2len(0), Some(686));
        assert_eq!(hdr.seq_id(ctg0), Some(0));
        assert_eq!(hdr.num_seqs(), 1);
        assert_eq!(hdr.seq_name(0), Some(ctg0));
        assert_eq!(hdr.seq_len(0), Some(686));
        Ok(())
    }
}
