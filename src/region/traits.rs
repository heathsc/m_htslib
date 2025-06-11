use std::ffi::CStr;

use crate::hts::{HtsPos, HtsTid};

pub trait RegCoords {
    // Get internal (HTS) ID for region
    fn id(&self) -> HtsTid;
    
    // Start and stop coordinates on sequence. None refers to start or end
    // of sequence respectively
    fn coords(&self) -> (Option<HtsPos>, Option<HtsPos>);
}

pub trait RegName {
    fn name(&self) -> &CStr;
}