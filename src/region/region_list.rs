use std::{
    cmp::{Ord, Ordering},
    collections::HashMap,
    ffi::CStr,
    num::NonZero,
    sync::Arc,
};

use super::{
    reg::{Reg, RegContig, RegionContig},
    traits::RegCoords,
};

use crate::{
    HtsError,
    hts::{HtsPos, hts_region::HtsRegion},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RegionCoords {
    start: HtsPos,
    end: Option<NonZero<HtsPos>>,
}

impl Ord for RegionCoords {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.start.cmp(&other.start) {
            Ordering::Equal => match (self.end.as_ref(), other.end.as_ref()) {
                (Some(x), Some(y)) => x.cmp(y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                _ => Ordering::Equal,
            },
            x => x,
        }
    }
}

impl PartialOrd for RegionCoords {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl RegCoords for RegionCoords {
    fn coords(&self) -> (Option<HtsPos>, Option<HtsPos>) {
        (Some(self.start), self.end.map(|x| x.get()))
    }
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

    pub fn from_reg_coords<T: RegCoords>(reg: &T) -> Self {
        match reg.coords() {
            (Some(x), y) => Self::new(x, y),
            (None, y) => Self::new(0, y),
        }
        .expect("Invalid region")
    }

    pub fn is_entire_contig(&self) -> bool {
        self.start == 0 && self.end.is_none()
    }

    pub fn check_overlap(&self, other: &Self) -> bool {
        if self > other {
            return other.check_overlap(self);
        }
        if let Some(x) = self.end.as_ref()
            && other.start > x.get()
        {
            false
        } else {
            true
        }
    }

    pub fn start(&self) -> HtsPos {
        self.start
    }

    pub fn end(&self) -> Option<HtsPos> {
        self.end.map(|x| x.get())
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

const CRL_UNSORTED: u8 = 1;
const CRL_UNNORMALIZED: u8 = 2;

pub struct ContigRegList {
    ctg: Arc<RegionContig>,
    regions: Option<Vec<RegionCoords>>,
    flags: u8,
}

impl ContigRegList {
    pub fn new(ctg: Arc<RegionContig>, entire_contig: bool) -> Self {
        let regions = if entire_contig {
            None
        } else {
            Some(Vec::new())
        };
        Self {
            ctg,
            regions,
            flags: 0,
        }
    }

    fn make_entire(&mut self) {
        self.regions = None;
        self.flags = 0;
    }
    
    pub fn normalize(&mut self) {
        if self.flags & CRL_UNSORTED != 0 {
            self.sort()
        }

        // Do merging
        if let Some(mut rc) = self.regions.take()
            && !rc.is_empty()
        {
            let mut new_v = Vec::new();
            let mut it = rc.drain(..);
            // We have already checked that rc is not empty, so it is safe to unwrap the first element
            let mut pending = it.next().unwrap();
            for c in it {
                if pending.check_overlap(&c) {
                    pending.end = match (pending.end, c.end) {
                        (Some(x), Some(y)) => Some(x.max(y)),
                        _ => None,
                    };
                } else {
                    new_v.push(pending);
                    pending = c;
                }
            }
            new_v.push(pending);
            self.regions = Some(new_v);
            self.flags &= !CRL_UNNORMALIZED
        }
    }

    pub fn sort(&mut self) {
        if let Some(rc) = self.regions.as_mut() {
            rc.sort_unstable();
        }
        self.flags &= !CRL_UNSORTED;
    }

    pub fn add<T: RegCoords>(&mut self, region: &T) -> bool {
        if let Some(reg_vec) = self.regions.as_mut() {
            let coords = RegionCoords::from_reg_coords(region);
            if coords.is_entire_contig() {
                self.regions = None;
                self.flags = 0;
            } else if let Some(r) = reg_vec.last() {
                if r != &coords {
                    if r > &coords {
                        self.flags |= CRL_UNSORTED | CRL_UNNORMALIZED
                    } else if r.check_overlap(&coords) {
                        self.flags |= CRL_UNNORMALIZED
                    }
                    reg_vec.push(coords);
                }
            } else {
                reg_vec.push(coords)
            }
        }
        self.flags & (CRL_UNSORTED | CRL_UNNORMALIZED) != 0
    }
}

const RL_ALL: u8 = 1;
const RL_UNMAPPED: u8 = 2;
const RL_UNNORMALIZED: u8 = 4;

#[derive(Default)]
pub struct RegionList {
    ctg_map: HashMap<Arc<RegionContig>, usize>,
    regions: Vec<ContigRegList>,
    flags: u8,
}

impl RegionList {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<'a, T>(&mut self, s: T) -> Result<(), HtsError>
    where
        T: TryInto<Reg<'a>>,
        HtsError: From<<T as TryInto<Reg<'a>>>::Error>,
    {
        let reg = s.try_into()?;
        self.add_reg(&reg);
        Ok(())
    }

    pub fn add_reg(&mut self, reg: &Reg) {
        match reg {
            Reg::All => {
                self.flags = RL_ALL;
                self.regions.clear();
                self.ctg_map.clear();
            }
            Reg::Unmapped => {
                if self.flags & RL_ALL == 0 {
                    self.flags |= RL_UNMAPPED
                }
            }
            Reg::Open(c, _) | Reg::Closed(c, _, _) => {
                let ctg = *c;
                if self.flags & RL_ALL == 0 {
                    let creg_list = self.add_or_lookup_ctg(ctg, false);
                    if creg_list.add(reg) {
                        self.flags |= RL_UNNORMALIZED
                    }
                }
            }
            Reg::Chrom(c) => {
                let ctg = *c;
                if self.flags & RL_ALL == 0 {
                    let _ = self.add_or_lookup_ctg(ctg, true);
                }
            }
        }
    }

    fn add_or_lookup_ctg(&mut self, ctg: &RegContig, entire_contig: bool) -> &mut ContigRegList {
        let ix = if let Some(x) = self.ctg_map.get(ctg) {
            if entire_contig {
                self.regions[*x].make_entire()
            }
            *x
        } else {
            let c = Arc::new(ctg.to_owned());
            let key = c.clone();
            let val = self.regions.len();
            self.ctg_map.insert(key, val);
            self.regions.push(ContigRegList::new(c, entire_contig));
            val
        };
        &mut self.regions[ix]
    }

    pub fn regions<'a>(&'a mut self) -> RegionIter<'a> {
        RegionIter::make(self)
    }

    pub fn contigs(&self) -> impl Iterator<Item = &CStr> {
        self.ctg_map.keys().map(|k| k.as_cstr())
    }

    pub fn normalize(&mut self) {
        if self.flags & RL_ALL != 0 {
            self.regions.clear();
            self.ctg_map.clear();
        }

        if self.flags & (RL_ALL | RL_UNNORMALIZED) == RL_UNNORMALIZED {
            for crl in self.regions.iter_mut() {
                crl.normalize()
            }
        }

        // Normaiize top level flags
        if self.flags & RL_ALL != 0 {
            self.flags = RL_ALL;
        } else {
            self.flags &= 3
        }
    }
}

pub struct RegionIter<'a> {
    inner: Option<&'a [ContigRegList]>,
    curr_ctg: Option<(&'a CStr, &'a [RegionCoords])>,
    flags: u8,
}

impl<'a> RegionIter<'a> {
    fn make(rl: &'a mut RegionList) -> Self {
        rl.normalize();

        let inner: Option<&[ContigRegList]> = if rl.flags & RL_ALL == 0 && !rl.regions.is_empty() {
            Some(&rl.regions)
        } else {
            None
        };
        Self {
            inner,
            curr_ctg: None,
            flags: rl.flags,
        }
    }
}

impl<'a> Iterator for RegionIter<'a> {
    type Item = HtsRegion<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.flags & RL_ALL != 0 {
            self.flags = 0;
            return Some(HtsRegion::new_all());
        }
        loop {
            if let Some((ctg, rc)) = self.curr_ctg {
                let hr = HtsRegion::new(ctg, &rc[0]);
                if rc.len() > 1 {
                    self.curr_ctg = Some((ctg, &rc[1..]));
                } else {
                    self.curr_ctg = None;
                }
                return Some(hr);
            } else if let Some(v) = self.inner {
                let crl = &v[0];
                if v.len() > 1 {
                    self.inner = Some(&v[1..]);
                } else {
                    self.inner = None;
                }
                if let Some(v1) = &crl.regions
                    && !v1.is_empty()
                {
                    let ctg = crl.ctg.as_cstr();
                    let rc: &[RegionCoords] = v1;
                    self.curr_ctg = Some((ctg, rc))
                }
            } else {
                break;
            }
        }
        if self.flags & RL_UNMAPPED != 0 {
            self.flags = 0;
            Some(HtsRegion::new_unmapped())
        } else {
            None
        }
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
        let reg = Reg::try_from("chr5:1.2M-1.43M").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::try_from(b"chr5:1000-2000").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::try_from("chr7:252654").unwrap();
        rl.add_reg(&reg);

        let r = rl.regions().nth(1).unwrap();
        let c = r.coords().expect("Not a region");
        let ctg = r.contig().expect("Not a region");
        assert_eq!(ctg, c"chr5");
        assert_eq!(c.start(), 1199999);
    }
    
    #[test]
    fn test_reg_list1() {
        let mut rl = RegionList::new();
        let reg = Reg::try_from("chr5:1.2M-1.43M").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::try_from(b"chr5:1000-2000").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::try_from(b"chr5:1500-1900").unwrap();
        rl.add_reg(&reg);
        let reg = Reg::try_from(b"chr5:1700-2500").unwrap();
        rl.add_reg(&reg);

        let r = rl.regions().next().unwrap();
        let c = r.coords().expect("Not a region");
        let ctg = r.contig().expect("Not a region");
        assert_eq!(ctg, c"chr5");
        assert_eq!(c.end(), Some(2500));
        
        let r = rl.regions().nth(1).unwrap();
        let c = r.coords().expect("Not a region");
        let ctg = r.contig().expect("Not a region");
        assert_eq!(ctg, c"chr5");
        assert_eq!(c.start(), 1199999);
    }
}
