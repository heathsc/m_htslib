use std::{collections::HashMap, ffi::CString, num::NonZero, sync::Arc};

use super::reg::Reg;
use crate::{
    HtsError,
    hts::{
        HtsPos,
        hts_region::{HtsRegion, HtslibRegion},
    },
};

#[derive(Debug, Clone, Copy)]
pub struct RegionCoords {
    start: HtsPos,
    end: Option<NonZero<HtsPos>>,
}

impl RegionCoords {
    pub fn new(start: HtsPos, end: Option<HtsPos>) -> Result<Self, HtsError> {
        match (start, end) {
            (..0, _) => Err(HtsError::InvalidRegion),
            (x, Some(y)) if x >= y => Err(HtsError::InvalidRegion),
            (start, Some(y)) => Ok(Self {
                start,
                end: NonZero::new(y),
            }),
            (start, None) => Ok(Self { start, end: None }),
        }
    }

    /// Convert RegionCoords into a range given by two HtsPos values,
    /// (x, y) where x is 0-offset, y is 1-offset, and x < y.
    /// if the end coordinate is missing or if it is greater than tseq_len,
    /// the end coordinate is replaced by seq_len.
    /// Returns None if start is beyond the start of the contig.
    ///
    /// Note that end (if present) should be > start - if this is not the
    /// case then this indicates an internal error.
    pub fn get_range(&self, seq_len: usize) -> Option<(HtsPos, HtsPos)> {
        let l = seq_len as HtsPos;
        if self.start >= l {
            None
        } else {
            self.end
                .map(|y| {
                    let y = y.get();
                    // This should not happen
                    assert!(y > self.start, "Internal error - invalid range");
                    (self.start, y.min(l))
                })
                .or(Some((self.start, l)))
        }
    }
}

#[derive(Debug)]
pub struct Region {
    coords: RegionCoords,
    ctg_id: u32,
}

impl Region {
    fn make(reg: &Reg, rl: &mut RegionList) -> Self {
        let ctg_id = rl.add_or_lookup_ctg(reg);
        let (start, end) = match reg {
            Reg::Chrom(_) | Reg::All | Reg::UnMapped => (0, None),
            Reg::Open(_, x) => (*x as HtsPos, None),
            Reg::Closed(_, x, y) => {
                // We know that y is > 0 so this transformation is safe
                let y = unsafe { NonZero::new_unchecked(y.get() as HtsPos) };
                (*x as HtsPos, Some(y))
            }
        };
        Self {
            ctg_id,
            coords: RegionCoords { start, end },
        }
    }
}
#[derive(Debug, Hash, PartialEq, Eq)]
pub enum RegionCtg {
    Contig(CString),
    All,
    Unmapped,
}

impl RegionCtg {
    fn from_reg(reg: &Reg) -> Self {
        match reg {
            Reg::Chrom(c) | Reg::Closed(c, _, _) | Reg::Open(c, _) => {
                Self::Contig(CString::new(c.as_bytes()).expect("Bad contig name"))
            }
            Reg::All => Self::All,
            Reg::UnMapped => Self::Unmapped,
        }
    }
}

#[derive(Default)]
pub struct RegionList {
    ctg_map: HashMap<Arc<RegionCtg>, u32>,
    regions: Vec<Region>,
    ctgs: Vec<Arc<RegionCtg>>,
}

impl RegionList {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_reg(&mut self, reg: &Reg) {
        let region = Region::make(reg, self);
        self.regions.push(region);
    }

    fn add_or_lookup_ctg(&mut self, reg: &Reg) -> u32 {
        let ctg = Arc::new(RegionCtg::from_reg(reg));
        let key = ctg.clone();

        *self.ctg_map.entry(key).or_insert_with(|| {
            let i = self.ctgs.len() as u32;
            self.ctgs.push(ctg);
            i
        })
    }

    pub fn regions(&self) -> RegionIter {
        RegionIter::make(self)
    }
}

pub struct RegionIter<'a> {
    inner: &'a RegionList,
    ix: usize,
}

impl<'a> RegionIter<'a> {
    fn make(rl: &'a RegionList) -> Self {
        Self { inner: rl, ix: 0 }
    }
}

impl<'a> Iterator for RegionIter<'a> {
    type Item = HtsRegion<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.regions.get(self.ix).map(|c| {
            self.ix += 1;
            let reg_ctg = self.inner.ctgs[c.ctg_id as usize].as_ref();
            HtsRegion::new(reg_ctg, &c.coords)
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;
    
    use crate::region::reg::Reg;
    
    #[test]
    fn test_reg_list() {
        let mut rl = RegionList::new();
        let reg = Reg::from_region(b"chr5:1000-2000").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::from_region(b"chr5:1.2M-1.43M").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::from_region(b"chr7:252654").unwrap();
        rl.add_reg(&reg);
        
        for r in rl.regions() {
            eprintln!("{:?}", r)
        }
        
        panic!("OOOOK!")
    }
}