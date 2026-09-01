use libc::{c_int, c_void};

use super::bam_pileup1::{BamPlpAuto, bam_mplp_t, bam_pileup1_t, bam_plp_t};
use crate::{hts::HtsPos, sam::bam1_t};

#[link(name = "hts")]
unsafe extern "C" {
    pub(super) unsafe fn bam_plp_init(func: BamPlpAuto, data: *mut c_void) -> *mut bam_plp_t;
    pub(super) unsafe fn bam_plp_destroy(iter: *mut bam_plp_t);
    pub(super) unsafe fn bam_plp_push(iter: *mut bam_plp_t, b: *const bam1_t);
    pub(super) unsafe fn bam_plp64_next(
        iter: *mut bam_plp_t,
        tid: *mut c_int,
        pos: *mut HtsPos,
        n_plp: *mut c_int,
    ) -> *const bam_pileup1_t;
    pub(super) unsafe fn bam_plp64_auto(
        iter: *mut bam_plp_t,
        tid: *mut c_int,
        pos: *mut HtsPos,
        n_plp: *mut c_int,
    ) -> *const bam_pileup1_t;
    pub(super) unsafe fn bam_plp_set_maxcnt(iter: *mut bam_plp_t, maxcnt: c_int);
    pub(super) unsafe fn bam_plp_reset(iter: *mut bam_plp_t);

    pub(super) fn bam_mplp_init(
        n: c_int,
        func: BamPlpAuto,
        data: *mut *mut c_void,
    ) -> *mut bam_mplp_t;
    pub(super) unsafe fn bam_mplp_init_overlaps(iter: *mut bam_mplp_t) -> c_int;
    pub(super) unsafe fn bam_mplp_destroy(iter: *mut bam_mplp_t);
    pub(super) unsafe fn bam_mplp_set_maxcnt(iter: *mut bam_mplp_t, maxcnt: c_int);
    pub(super) unsafe fn bam_mplp64_auto(
        iter: *mut bam_mplp_t,
        tid: *mut c_int,
        pos: *mut HtsPos,
        n_plp: *mut c_int,
        plp: *mut *const bam_pileup1_t,
    ) -> c_int;

    pub(super) unsafe fn bam_mplp_reset(iter: *mut bam_mplp_t);
}
