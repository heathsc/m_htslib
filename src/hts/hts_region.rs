use std::ffi::CStr;

use libc::c_int;

use crate::{
    hts::{
        HTS_IDX_NOCOOR, HTS_IDX_START, HtsError, HtsPos,
        traits::{IdMap, SeqId},
    },
    region::{
        reg::{Region, RegionContig},
        region_list::{RegionCoords, RegionCtg},
    },
};

#[derive(Debug)]
pub struct HtsCtgRegion<'a> {
    contig: &'a CStr,
    coords: RegionCoords,
}

impl<'a> HtsCtgRegion<'a> {
    pub fn make_htslib_region<T: IdMap + SeqId>(&self, h: &T) -> Result<HtslibRegion, HtsError> {
        match h.seq_id(self.contig) {
            Some(i) => {
                // We panic here because this indicates an internal error
                let len = h.seq_len(i).expect("Missing length");
                let (start, end) = self.coords.get_range(len).ok_or(HtsError::InvalidRegion)?;
                let tid = i as c_int;
                Ok(HtslibRegion { tid, start, end })
            }
            None => Err(HtsError::UnknownContig(self.contig.to_owned())),
        }
    }

    fn new(contig: &'a CStr, start: HtsPos, end: Option<HtsPos>) -> Self {
        // Shouldn't happen as this is an internal function and the parameters should have been checked
        let coords = RegionCoords::new(start, end).expect("Bad coordinates");
        Self { contig, coords }
    }
}

#[derive(Debug)]
pub enum HtsRegion<'a> {
    Contig(HtsCtgRegion<'a>),
    All,
    Unmapped,
}

impl HtsRegion<'_> {
    pub fn make_htslib_region<T: IdMap + SeqId>(&self, h: &T) -> Result<HtslibRegion, HtsError> {
        match self {
            Self::Contig(c) => c.make_htslib_region(h),
            Self::All => Ok(HtslibRegion {
                tid: HTS_IDX_START,
                start: 0,
                end: 1,
            }),
            Self::Unmapped => Ok(HtslibRegion {
                tid: HTS_IDX_NOCOOR,
                start: 0,
                end: 1,
            }),
        }
    }
}

fn mk_hts_region_contig<'a>(
    c: &'a RegionContig,
    start: usize,
    end: Option<HtsPos>,
) -> HtsRegion<'a> {
    HtsRegion::Contig(HtsCtgRegion::new(c.as_cstr(), start as HtsPos, end))
}

impl<'a> From<&'a Region> for HtsRegion<'a> {
    fn from(r: &'a Region) -> Self {
        match r {
            Region::Chrom(c) => mk_hts_region_contig(c, 0, None),
            Region::Open(c, x) => mk_hts_region_contig(c, *x, None),
            Region::Closed(c, x, y) => mk_hts_region_contig(c, *x, Some(y.get() as HtsPos)),
            Region::All => Self::All,
            Region::Unmapped => Self::Unmapped,
        }
    }
}

impl<'a> HtsRegion<'a> {
    pub fn new(ctg: &'a RegionCtg, coords: &RegionCoords) -> Self {
        match ctg {
            RegionCtg::All => Self::All,
            RegionCtg::Unmapped => Self::Unmapped,
            RegionCtg::Contig(c) => Self::Contig(HtsCtgRegion {
                contig: c.as_c_str(),
                coords: *coords,
            }),
        }
    }

    pub fn new_unmapped() -> Self {
        Self::Unmapped
    }

    pub fn new_all() -> Self {
        Self::All
    }
}

/// A region that is specific for a particular Hts file (in respect of the contig ids) and
/// that can be passed to htslib iterators etc.
#[derive(Debug)]
pub struct HtslibRegion {
    tid: c_int,
    start: HtsPos,
    end: HtsPos,
}

impl HtslibRegion {
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
        region::{reg::{Reg, Region}, region_list::RegionCoords},
        sam::SamHdr,
    };

    #[test]
    fn region_test() {
        let mut hts = HtsFile::open(c"test/realn01.sam", c"r").unwrap();
        let hdr = SamHdr::read(&mut hts).unwrap();

        let reg = HtsCtgRegion {
            contig: c"000000F",
            coords: RegionCoords::new(24, Some(200)).unwrap(),
        };
        let hreg = HtsRegion::Contig(reg);
        let hr = hreg.make_htslib_region(&hdr).unwrap();
        eprintln!("{:?}", hr);
        assert_eq!(hr.end, 200);
        assert_eq!(hr.start, 24);

        let reg = HtsCtgRegion {
            contig: c"000000F",
            coords: RegionCoords::new(24, Some(2000)).unwrap(),
        };
        let hreg = HtsRegion::Contig(reg);
        let hr = hreg.make_htslib_region(&hdr).unwrap();
        eprintln!("{:?}", hr);
        assert_eq!(hr.end, 686);

        let reg = HtsCtgRegion {
            contig: c"000000F",
            coords: RegionCoords::new(24, None).unwrap(),
        };
        let hreg = HtsRegion::Contig(reg);
        let hr = hreg.make_htslib_region(&hdr).unwrap();
        eprintln!("{:?}", hr);
        assert_eq!(hr.end, 686)
    }
    
    #[test]
    fn region_test2() {
        let mut hts = HtsFile::open(c"test/realn01.sam", c"r").unwrap();
        let hdr = SamHdr::read(&mut hts).unwrap();

        let reg = Reg::from_u8_slice(b"000000F:25-200").unwrap();
        let region = Region::from_reg(&reg);
        let hreg: HtsRegion = HtsRegion::from(&region);
        let hr = hreg.make_htslib_region(&hdr).unwrap();
        
        eprintln!("{:?}", hr);
        assert_eq!(hr.end, 200);
        assert_eq!(hr.start, 24);
    }
}
