use thiserror::Error;

use super::{hts, kstring, sam, bgzf};
pub use hts::hts_error::*;
pub use kstring::kstring_error::*;
pub use sam::cigar_error::*;
pub use sam::sam_error::*;
pub use bgzf::bgzf_error::*;