use std::{fmt, sync::LazyLock};

use regex::Regex;

/// Matches when the contig is disambiguated using brackets i.e.., {chr2}:20000-50000
/// The Regex for the contig name comes from the VCF4.3 spec
static RE_REGION1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^[{]([0-9A-Za-z!#$%&+./:;?@^_|~-][0-9A-Za-z!#$%&*+./:;=?@^_|~-]*)[}]:?\s*([0-9,]+)?-?\s*([0-9,]+)?"#).unwrap()
});

/// Matches when the contig is present without brackets i.e., chr2:20000-30000
static RE_REGION2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^([0-9A-Za-z!#$%&+./:;?@^_|~-][0-9A-Za-z!#$%&*+./;=?@^_|~-]*):?\s*([0-9,]+)?-?\s*([0-9,]+)?"#,
    )
    .unwrap()
});

#[derive(Debug)]
pub enum Reg<'a> {
    Chrom(&'a str),
    Open(&'a str, usize),
    Closed(&'a str, usize, usize),
}

fn write_ctg(f: &mut fmt::Formatter, s: &str) -> fmt::Result {
    if f.alternate() && s.contains(':') {
        write!(f, "{{{}}}", s)
    } else {
        write!(f, "{}", s)
    }
}

impl fmt::Display for Reg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reg::Chrom(a) => write_ctg(f, a),
            Reg::Open(a, x) => {
                write_ctg(f, a)?;
                write!(f, ":{}", *x)
            }
            Reg::Closed(a, x, y) if *x == 0 => {
                write_ctg(f, a)?;
                write!(f, ":-{}", *y)
            }
            Reg::Closed(a, x, y) => {
                write_ctg(f, a)?;
                write!(f, ":{}-{}", *x, *y)
            }
        }
    }
}
