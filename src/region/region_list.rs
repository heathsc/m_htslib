use std::num::NonZero;
use std::{collections::HashMap, ffi::CString};

use super::reg::Reg;
use crate::hts::HtsPos;

#[derive(Debug)]
pub struct Region {
    start: HtsPos,
    end: Option<NonZero<HtsPos>>,
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
            },
        };
        Self { ctg_id, start, end}
    }
}
#[derive(Debug, Hash, PartialEq, Eq)]
pub enum RegionCtg {
    Contig(CString),
    All,
    UnMapped,
}

impl RegionCtg {
    fn from_reg(reg: &Reg) -> Self {
        match reg {
            Reg::Chrom(c) | Reg::Closed(c, _, _) | Reg::Open(c, _) => {
                Self::Contig(CString::new(c.as_bytes()).expect("Bad contig name"))
            }
            Reg::All => Self::All,
            Reg::UnMapped => Self::UnMapped,
        }
    }
}

#[derive(Default)]
pub struct RegionList {
    ctg_map: HashMap<RegionCtg, u32>,
    regions: Vec<Region>,
    n_ctgs: u32,
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
        let ctg = RegionCtg::from_reg(reg);
        *self.ctg_map.entry(ctg).or_insert_with(|| {
            let i = self.n_ctgs;
            self.n_ctgs += 1;
            i
        })
    }
}
