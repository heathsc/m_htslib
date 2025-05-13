pub mod bam1;

use bam1::*;

#[derive(Clone, Default, Debug)]
pub struct BamRec {
    inner: bam1_t,
}

