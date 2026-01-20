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

    pub fn overlaps(&self, other: &Self) -> bool {
        if self > other {
            other._overlaps(self)
        } else {
            self._overlaps(other)
        }
    }

    /// Note: self <= other
    fn _overlaps(&self, other: &Self) -> bool {
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

#[derive(Clone)]
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
                if pending.overlaps(&c) {
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
                    } else if r.overlaps(&coords) {
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

    pub fn intersect(&mut self, other: &Self) -> Result<(), HtsError> {
        if other.flags & CRL_UNNORMALIZED == 0 {
            if self.flags & CRL_UNNORMALIZED != 0 {
                self.normalize();
            }
            if self.regions.is_none() {
                // self has entire contig, so we just clone from other
                *self = other.clone()
            } else if let Some(reg2) = other.regions.as_deref() {
                // This is safe as we already checked above that self.regions is not none
                let mut reg1 = self.regions.take().unwrap();
                let mlen = reg1.len().min(reg2.len());
                let mut reg1_itr = reg1.drain(..);
                let mut reg2_itr = reg2.iter();

                //  Get intersect between reg1 and reg2

                // Output vector for intersect
                let mut out_vec = Vec::with_capacity(mlen);

                let mut curr_reg1 = reg1_itr.next();
                let mut curr_reg2 = None;
                while let Some(r1) = curr_reg1.as_ref() {
                    if let Some(r2) = curr_reg2.take().or_else(|| reg2_itr.next()) {
                        if r1.overlaps(r2) {
                            let start = r1.start.max(r2.start);
                            if match (r1.end(), r2.end()) {
                                (Some(x), Some(y)) => x <= y,
                                (None, Some(_)) => false,
                                _ => true,
                            } {
                                // r1 ends at or before r2
                                out_vec.push(RegionCoords { start, end: r1.end });
                                // As both reg1 and reg2 are normalized we know that the next rgion in reg2 will not
                                // overlap r1, so we get the next region from reg1, but we keep r2 in case the next
                                // from reg1 also overlaps with it
                                curr_reg1 = reg1_itr.next();
                                curr_reg2 = Some(r2);
                            } else {
                                // r1 ends after r2
                                out_vec.push(RegionCoords { start, end: r2.end });
                                // We keep r1 for the next comparison, and get the next region from reg2
                            }
                        } else if r1 < r2 {
                            // No overlap and r1 is before r2, so get next entry from reg1
                            curr_reg1 = reg1_itr.next();
                            curr_reg2 = Some(r2)
                        }
                        // if r1 > r2 (and doesn't overlap with r1) then we keep r1 and get the next rgion from r2
                    } else {
                        break;
                    }
                }
                self.regions = Some(out_vec);
            }

            Ok(())
        } else {
            Err(HtsError::RegionListArgumentNotNormalized)
        }
    }
}

const RL_ALL: u8 = 1;
const RL_UNMAPPED: u8 = 2;
const RL_UNNORMALIZED: u8 = 4;

#[derive(Clone, Default)]
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

    pub fn intersect(&mut self, other: &Self) -> Result<(), HtsError> {
        if other.flags & RL_UNNORMALIZED == 0 {
            self.normalize();
            if self.flags | other.flags & RL_ALL == 0 {
                self.flags |= other.flags & RL_UNMAPPED;
                let mut new_regions = Vec::new();
                let mut new_map = HashMap::new();
                for mut reg in self.regions.drain(..) {
                    if let Some(ix) = other.ctg_map.get(reg.ctg.as_ref()) {
                        reg.intersect(&other.regions[*ix])?;
                        let key = reg.ctg.clone();
                        let val = new_regions.len();
                        new_map.insert(key, val);
                        new_regions.push(reg);
                    }
                }
                self.ctg_map = new_map;
                self.regions = new_regions;
            } else if self.flags & RL_ALL != 0 {
                *self = other.clone()
            }
            // The remaining possibility is other.flags contains RL_ALL. In this case we do nothing

            Ok(())
        } else {
            Err(HtsError::RegionListArgumentNotNormalized)
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

    #[test]
    fn test_reg_list_intersect() {
        let mut r1 = RegionList::new();
        let reg = Reg::try_from("chr5:3000-5000").unwrap();
        r1.add_reg(&reg);
        let reg = Reg::try_from("chr5:1000-2000").unwrap();
        r1.add_reg(&reg);
        let reg = Reg::try_from("chr3:1000-20000").unwrap();
        r1.add_reg(&reg);
        let reg = Reg::try_from("chr4:1M-1.5M").unwrap();
        r1.add_reg(&reg);
        let reg = Reg::try_from("chr8:15.1k-").unwrap();
        r1.add_reg(&reg);

        let mut r2 = RegionList::new();
        let reg = Reg::try_from("chr3").unwrap();
        r2.add_reg(&reg);
        let reg = Reg::try_from("chr5:4000-6000").unwrap();
        r2.add_reg(&reg);
        let reg = Reg::try_from("chr5:500-1200").unwrap();
        r2.add_reg(&reg);
        let reg = Reg::try_from("chr8:15k-17k").unwrap();
        r2.add_reg(&reg);
        let reg = Reg::try_from("chr5:3100-3200").unwrap();
        r2.add_reg(&reg);

        r2.normalize();
        r1.intersect(&r2).expect("Error in intersect");

        let r = r1.regions().nth(4).expect("Not enough regions in intersect");
        let c = r.coords().expect("Not a region");
        let ctg = r.contig().expect("Not a region");
        assert_eq!(ctg, c"chr8");
        assert_eq!(c.start(), 15099);
    }
}
