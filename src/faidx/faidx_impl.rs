use libc::{c_char, c_int, c_void, free};

use crate::hts::HtsPos;

#[repr(C)]
pub(super) struct FaidxRaw {
    _unused: [u8; 0],
}

unsafe extern "C" {
    fn fai_load(fn_: *const c_char) -> *mut FaidxRaw;
    fn fai_load3(fn_: *const c_char, fnai: *const c_char, fngzi: *const c_char, flags: c_int) -> *mut FaidxRaw;
    fn faidx_nseq(fai: *const FaidxRaw) -> c_int;
    fn faidx_iseq(fai: *const FaidxRaw, n: c_int) -> *const c_char;
    fn faidx_seq_len(fai: *const FaidxRaw, seq: *const c_char) -> c_int;
    fn faidx_fetch_seq64(
        fai: *const FaidxRaw,
        cname: *const c_char,
        x: HtsPos,
        y: HtsPos,
        len: *mut HtsPos,
    ) -> *mut c_char;
}

