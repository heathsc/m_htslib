use libc::c_int;

use crate::hts::HtsPos;

/// A region that is specific for a particular Hts file (in respect of the contig ids) and
/// that can be passed to htslib iterators etc.
#[derive(Debug, Copy, Clone)]
pub struct HtsRegion {
    tid: c_int,
    start: HtsPos,
    end: HtsPos,
}

impl HtsRegion {
    #[inline]
    pub(crate) fn new(tid: c_int, start: HtsPos, end: HtsPos) -> Self {
        assert!(tid >= 0);
        assert!(start <= end);
        Self { tid, start, end }
    }

    #[inline]
    pub fn tid(&self) -> c_int {
        self.tid
    }

    #[inline]
    pub fn start(&self) -> HtsPos {
        self.start
    }
    #[inline]
    pub fn end(&self) -> HtsPos {
        self.end
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;
    use crate::{
        hts::{
            HtsFile,
            traits::{IdMap, SeqId},
        },
        region::{
            reg::{Reg, Region},
            region_list::RegionCoords,
        },
        sam::SamHdr,
    };

    #[test]
    fn region_test() {
        let mut hts = HtsFile::open(c"test/realn01.sam", c"r").unwrap();
        let hdr = SamHdr::read(&mut hts).unwrap();

        let reg = Reg::from_u8_slice(b"000000F:25-200").unwrap();
        let hr = reg.make_htslib_region(&hdr).unwrap();

        eprintln!("{hr:?}");
        assert_eq!(hr.end, 200);
        assert_eq!(hr.start, 24);
    }
}
