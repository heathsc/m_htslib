use crate::hts::HtsPos;

pub trait RegCoords {
    // Start and stop coordinates on sequence. None refers to start or end
    // of sequence respectively.
    // In keeping with htslib, the start coordinate is 0-offset and the
    // end coordinate is 1-offset, so the length of the region
    // is end - start
    fn coords(&self) -> (Option<HtsPos>, Option<HtsPos>);
}

pub trait RegCtgName {
    fn contig_name(&self) -> &str;
}
