use std::ffi::CStr;

use libc::c_int;

use crate::{
    hts::{
        HTS_IDX_NOCOOR, HTS_IDX_START, HtsError, HtsPos,
        traits::{IdMap, SeqId},
    },
    region::region_list::{RegionCtg, RegionCoords},
};

#[derive(Debug)]
pub struct HtsCtgRegion<'a> {
    contig: &'a CStr,
    coords: RegionCoords,
}

impl HtsCtgRegion<'_> {
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

impl <'a> HtsRegion<'a> {
    pub fn new(ctg: &'a RegionCtg, coords: &RegionCoords) -> Self {
        match ctg {
            RegionCtg::All => Self::All,
            RegionCtg::Unmapped => Self::Unmapped,
            RegionCtg::Contig(c) => Self::Contig(HtsCtgRegion { contig: c.as_c_str(), coords: *coords })
        }
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

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;
    use crate::{
        hts::{
            HtsFile,
            traits::{IdMap, SeqId},
        },
        region::region_list::RegionCoords,
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
}
