use libc::{c_char, c_int, size_t};
use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr,
};

use super::sam_error::SamError;
use crate::{cstr_len, from_c, hts::htsfile::HtsFileRaw, kstring::KString};

#[link(name = "hts")]
extern "C" {
    fn sam_hdr_read(fp_: *mut HtsFileRaw) -> *mut SamHdrRaw;
    fn sam_hdr_write(fp_: *mut HtsFileRaw, hd_: *const SamHdrRaw) -> c_int;
    fn sam_hdr_init() -> *mut SamHdrRaw;
    fn sam_hdr_destroy(hd_: *mut SamHdrRaw);
    fn sam_hdr_dup(hd_: *const SamHdrRaw) -> *mut SamHdrRaw;
    fn sam_hdr_parse(len_: size_t, text_: *const c_char) -> *mut SamHdrRaw;
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
    fn sam_hdr_name2tid(hd_: *const SamHdrRaw, nm_: *const c_char) -> c_int;
    fn sam_hdr_str(hd_: *const SamHdrRaw) -> *const c_char;
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
    fn sam_hdr_add_pg(hd: *mut SamHdrRaw, name: *const c_char, ...) -> c_int;
}

#[repr(C)]
pub struct SamHdrRaw {
    _unused: [u8; 0],
}

impl SamHdrRaw {
    pub fn write(&self, hts_file: &mut HtsFileRaw) -> Result<(), SamError> {
        match unsafe { sam_hdr_write(hts_file as *mut HtsFileRaw, self) } {
            0 => Ok(()),
            _ => Err(SamError::FailedHeaderWrite),
        }
    }
    #[inline]
    pub fn nref(&self) -> usize {
        let l = unsafe { sam_hdr_nref(self) };
        l as usize
    }
    #[inline]
    fn check_idx(&self, i: usize) -> bool {
        i < self.nref()
    }
    pub fn tid2name(&self, i: usize) -> Option<&CStr> {
        if self.check_idx(i) {
            from_c(unsafe { sam_hdr_tid2name(self, i as c_int) })
        } else {
            None
        }
    }
    pub fn tid2len(&self, i: usize) -> Option<usize> {
        if self.check_idx(i) {
            let len = unsafe { sam_hdr_tid2len(self, i as c_int) };
            Some(len as usize)
        } else {
            None
        }
    }
    pub fn name2tid(&self, cname: &CStr) -> Option<usize> {
        let tid = unsafe { sam_hdr_name2tid(self, cname.as_ptr()) };
        if tid < 0 {
            None
        } else {
            Some(tid as usize)
        }
    }
    pub fn text(&self) -> Option<&CStr> {
        from_c(unsafe { sam_hdr_str(self) })
    }

    pub fn add_lines(&mut self, lines: &CStr) -> Result<(), SamError> {
        match unsafe { sam_hdr_add_lines(self, lines.as_ptr(), cstr_len(lines) as size_t) } {
            0 => Ok(()),
            _ => Err(SamError::FailedAddHeaderLine),
        }
    }

    pub fn remove_except(
        &mut self,
        ln_type: &CStr,
        id_key: Option<&CStr>,
        id_value: Option<&CStr>,
    ) -> Result<(), SamError> {
        match if let (Some(key), Some(value)) = (id_key, id_value) {
            unsafe { sam_hdr_remove_except(self, ln_type.as_ptr(), key.as_ptr(), value.as_ptr()) }
        } else {
            unsafe { sam_hdr_remove_except(self, ln_type.as_ptr(), ptr::null(), ptr::null()) }
        } {
            0 => Ok(()),
            _ => Err(SamError::FailedRemoveHeaderLines),
        }
    }
    pub fn remove(&mut self, ln_type: &CStr) -> Result<(), SamError> {
        self.remove_except(ln_type, None, None)
    }
    pub fn change_hd(&mut self, key: &CStr, val: Option<&CStr>) {
        let val = if let Some(v) = val {
            v.as_ptr()
        } else {
            ptr::null::<c_char>()
        };
        unsafe { sam_hdr_change_HD(self, key.as_ptr(), val) }
    }
    pub fn find_line_id(&mut self, typ: &CStr, id_key: &CStr, id_val: &CStr) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_line_id(
                self,
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
    pub fn find_line_pos(&mut self, typ: &CStr, pos: usize) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe { sam_hdr_find_line_pos(self, typ.as_ptr(), pos as c_int, &mut ks) == 0 } {
            Some(ks)
        } else {
            None
        }
    }
    pub fn find_tag_id(
        &mut self,
        typ: &CStr,
        id_key: &CStr,
        id_val: &CStr,
        key: &CStr,
    ) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_tag_id(
                self,
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
    pub fn find_tag_pos(&mut self, typ: &CStr, pos: usize, key: &CStr) -> Option<KString> {
        let mut ks = KString::new();
        if unsafe {
            sam_hdr_find_tag_pos(self, typ.as_ptr(), pos as c_int, key.as_ptr(), &mut ks) == 0
        } {
            Some(ks)
        } else {
            None
        }
    }
    pub fn count_lines(&mut self, typ: &CStr) -> Option<usize> {
        let n = unsafe { sam_hdr_count_lines(self, typ.as_ptr()) };
        if n >= 0 {
            Some(n as usize)
        } else {
            None
        }
    }
}

/// inner is always non-null, but we don't use NonNull<> here because
/// we don't want to assume Covariance.
pub struct SamHdr<'a> {
    inner: *mut SamHdrRaw,
    phantom: PhantomData<&'a SamHdrRaw>,
}

impl<'a> Deref for SamHdr<'a> {
    type Target = SamHdrRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl<'a> DerefMut for SamHdr<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl<'a> Clone for SamHdr<'a> {
    fn clone(&self) -> Self {
        Self::try_dup(self).expect("Could not duplicate SamHdr")
    }
}

unsafe impl<'a> Send for SamHdr<'a> {}
unsafe impl<'a> Sync for SamHdr<'a> {}

impl<'a> Drop for SamHdr<'a> {
    fn drop(&mut self) {
        unsafe { sam_hdr_destroy(self.inner) };
    }
}

impl<'a> Default for SamHdr<'a> {
    fn default() -> Self {
        Self::try_init().expect("Could not allocate new SamHdr")
    }
}
impl<'a> SamHdr<'a> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn try_init() -> Result<Self, SamError> {
        Self::make_sam_hdr(unsafe { sam_hdr_init() }, SamError::OutOfMemory)
    }

    pub fn try_dup(&self) -> Result<Self, SamError> {
        Self::make_sam_hdr(unsafe { sam_hdr_dup(self.inner) }, SamError::OutOfMemory)
    }

    pub fn parse(text: &CStr) -> Result<Self, SamError> {
        Self::make_sam_hdr(
            unsafe { sam_hdr_parse(cstr_len(text) as size_t, text.as_ptr()) },
            SamError::HeaderParseFailed,
        )
    }

    // pub fn read()
    fn make_sam_hdr(hdr: *mut SamHdrRaw, e: SamError) -> Result<Self, SamError> {
        if hdr.is_null() {
            Err(e)
        } else {
            Ok(Self {
                inner: hdr,
                phantom: PhantomData,
            })
        }
    }
}
