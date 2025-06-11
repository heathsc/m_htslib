use std::{
    ffi::{CStr, CString},
    fmt,
    sync::LazyLock,
};

use regex::bytes::Regex;

use crate::{
    HtsError,
    int_utils::{parse_decimal, skip_space},
};

/// Matches when the contig is disambiguated using brackets i.e.., {chr2}
/// The Regex for the contig name comes from the VCF4.3 spec
static RE_CONTIG1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*[{]([0-9A-Za-z!#$%&+./:;?@^_|~-][0-9A-Za-z!#$%&*+./:;=?@^_|~-]*)[}](:)?\s*([^:]*)\s*$"#,
    )
    .unwrap()
});

/// Matches when the contig is present without brackets i.e., chr2
static RE_CONTIG2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*([0-9A-Za-z!#$%&+./:;?@^_|~-][0-9A-Za-z!#$%&*+./;=?@^_|~-]*)(:)?\s*([^:]*)\s*$"#,
    )
    .unwrap()
});

#[derive(Debug)]
pub struct RegContig {
    inner: CString,
}

impl RegContig {
    pub fn from_region(s: &[u8]) -> Result<(Self, &[u8], bool), HtsError> {
        if let Some(cap) = RE_CONTIG1.captures(s).or_else(|| RE_CONTIG2.captures(s)) {
            if let (Some(c), Some(r)) = (cap.get(1), cap.get(3)) {
                let ctg = Self {
                    inner: CString::new(c.as_bytes())
                        .expect("Internal error - null bytes in contig name"),
                };
                Ok((ctg, r.as_bytes(), cap.get(2).is_some()))
            } else {
                Err(HtsError::InvalidContig)
            }
        } else {
            Err(HtsError::InvalidContig)
        }
    }

    #[inline]
    pub fn contig(&self) -> &CStr {
        self.inner.as_c_str()
    }
}

impl fmt::Display for RegContig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.inner.to_string_lossy();

        if f.alternate() && s.contains(':') {
            write!(f, "{{{}}}", s)
        } else {
            write!(f, "{}", s)
        }
    }
}

#[derive(Debug)]
pub enum Reg {
    Chrom(RegContig),
    Open(RegContig, usize),
    Closed(RegContig, usize, usize),
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reg::Chrom(a) => write!(f, "{a}"),
            Reg::Open(a, x) => write!(f, "{a}:{}-", x + 1),
            Reg::Closed(a, x, y) if *x == 0 => write!(f, "{a}:-{y}"),
            Reg::Closed(a, x, y) => write!(f, "{a}:{}-{y}", x + 1),
        }
    }
}

impl Reg {
    pub fn from_region(s: &[u8]) -> Result<Self, HtsError> {
        let (ctg, s, colon) = RegContig::from_region(s)?;
        match (colon, s) {
            (_, &[]) => Ok(Self::Chrom(ctg)),
            (false, _) => Err(HtsError::TrailingGarbage),
            (true, s) => Self::parse_range(s, ctg),
        }
    }

    fn parse_range(s: &[u8], ctg: RegContig) -> Result<Self, HtsError> {
        let (x, s) = parse_decimal(s)?;
        let s = skip_space(s);
        match (x, s) {
            (x, &[]) if x < 0 => Ok(Self::Closed(ctg, 0, -x as usize)),
            (x, &[]) | (x, b"-") => Ok(Self::Open(ctg, (x - 1).max(0) as usize)),
            (x, s) if s[0] == b'-' => match parse_decimal(&s[1..])? {
                (y, &[]) if y < x => Err(HtsError::InvalidRegion),
                (y, &[]) => Ok(Self::Closed(ctg, (x - 1).max(0) as usize, y as usize)),
                (_, _) => Err(HtsError::TrailingGarbage),
            },
            (_, _) => Err(HtsError::TrailingGarbage),
        }
    }

    #[inline]
    pub fn contig(&self) -> &CStr {
        match self {
            Self::Chrom(s) | Self::Open(s, _) | Self::Closed(s, _, _) => s.contig(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    #[test]
    fn test_parse_reg_contig() {
        let (ctg, s, colon) = RegContig::from_region(b"chr5:1.2M-1.43M").unwrap();
        assert_eq!(ctg.to_string(), "chr5");
        assert_eq!(s, b"1.2M-1.43M");
        assert!(colon);

        let (ctg, s, colon) = RegContig::from_region(b"chr5").unwrap();
        assert!(s.is_empty());
        assert!(!colon);

        let (ctg, s, colon) = RegContig::from_region(b"{chr5:1}:1.2M-1.43M").unwrap();
        assert_eq!(ctg.to_string(), "chr5:1");
        assert_eq!(s, b"1.2M-1.43M");
        assert!(colon);
    }

    #[test]
    fn test_parse_reg() {
        let reg = Reg::from_region(b"chr5:1.2M-1.43M").unwrap();
        eprintln!("{reg}");
        assert_eq!(reg.contig(), c"chr5");
        assert!(matches!(reg, Reg::Closed(_, 1199999, 1430000)));

        let reg = Reg::from_region(b"chr7.1").unwrap();
        eprintln!("{reg}");
        assert_eq!(reg.contig(), c"chr7.1");
        assert!(matches!(reg, Reg::Chrom(_)));

        let reg = Reg::from_region(b"chrX:1.234m").unwrap();
        eprintln!("{reg}");
        assert!(matches!(reg, Reg::Open(_, 1233999)));

        let reg = Reg::from_region(b"chrX:1000-500");
        assert!(matches!(reg, Err(HtsError::InvalidRegion)));
        
        let reg = Reg::from_region(b"chrX:1000,");
        assert!(matches!(reg, Err(HtsError::TrailingGarbage)));       
    }
}
