use std::{
    borrow::Borrow,
    ffi::{CStr, CString},
    fmt,
    hash::Hash,
    num::NonZero,
    ops::Deref,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use libc::c_int;
use regex::bytes::Regex;

use crate::{
    HtsError,
    hts::{
        HTS_IDX_NOCOOR, HTS_IDX_START, HtsPos,
        hts_region::HtsRegion,
        traits::{IdMap, SeqId},
    },
    int_utils::{parse_decimal, skip_space},
};

use super::{region_list::RegionCoords, traits::*};

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

/// Owned version of RegContig, with the difference that RegionContig will always be
/// nul terminated (and contain no nul characters, which is the same as RegContig).
/// In this way we can translate directly into a CStr for interacting with C functions.
///
/// Note that this is similar to Box<CStr> except we box a u8 slice rather than a c_char slice.
/// This is just to make it more transparent moving between RegionContig and RegContig, while
/// converting to CStr is cheap as u8 and c_char have the same binary representation.
#[derive(Debug, Eq)]
pub struct RegionContig {
    inner: Box<[u8]>,
}

impl Ord for RegionContig {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_bytes().cmp(other.to_bytes())
    }
}

impl PartialOrd for RegionContig {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RegionContig {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl Hash for RegionContig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state)
    }
}

impl Deref for RegionContig {
    type Target = RegContig;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.to_bytes_with_nul() as *const [u8] as *const Self::Target) }
    }
}

impl AsRef<RegContig> for RegionContig {
    fn as_ref(&self) -> &RegContig {
        self
    }
}

impl Borrow<RegContig> for RegionContig {
    fn borrow(&self) -> &RegContig {
        self.deref()
    }
}

impl Borrow<RegContig> for Box<RegionContig> {
    fn borrow(&self) -> &RegContig {
        self.deref()
    }
}

impl Borrow<RegContig> for Rc<RegionContig> {
    fn borrow(&self) -> &RegContig {
        self.deref()
    }
}

impl Borrow<RegContig> for Arc<RegionContig> {
    fn borrow(&self) -> &RegContig {
        self.deref()
    }
}

impl RegionContig {
    pub fn to_bytes(&self) -> &[u8] {
        let b = self.inner.as_ref();
        &b[..b.len() - 1]
    }

    pub fn to_bytes_with_nul(&self) -> &[u8] {
        self.inner.as_ref()
    }

    pub fn from_u8_slice(s: &[u8]) -> Result<(Self, &[u8], bool), HtsError> {
        RegContig::parse_from_u8_slice(s).map(|(ctg, r, colon)| (ctg.to_owned(), r, colon))
    }

    #[inline]
    pub fn as_cstr(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(self.to_bytes_with_nul()) }
    }
}

impl fmt::Display for RegionContig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// A borrowed version of RegionContig whcih is a wrapper around a [u8] slice where all
/// members of the slice must be ascii and valid components of a contig name. In particular
/// there can be no non-terminal nul characters in the slice.
///
/// Important: the [u8] slice may or may not be null terminated. This can be checked using the
/// is_null_terminated function.
///
/// An instance can be created using `RegContig::from_u8_slice`. Note that this
/// is the only way to directly create a RegContig instance, and because we are using a regexp to
/// parse the input, we can be assured that the invariants above hold.
#[derive(Debug, Eq)]
#[repr(transparent)]
pub struct RegContig {
    inner: [u8],
}

impl ToOwned for RegContig {
    type Owned = RegionContig;

    // We need to clone the slice to a Box<[u8]>, giving enough room for the terminating nul
    fn to_owned(&self) -> Self::Owned {
        let bytes = &self.inner;
        let capacity = if self.is_null_terminated() {
            bytes.len()
        } else {
            bytes.len().checked_add(1).unwrap()
        };
        let mut buffer = Vec::with_capacity(capacity);
        buffer.extend(bytes);
        if !self.is_null_terminated() {
            buffer.reserve_exact(1);
            buffer.push(0);
        }
        Self::Owned {
            inner: buffer.into_boxed_slice(),
        }
    }
}

impl Deref for RegContig {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.to_bytes() as *const Self::Target) }
    }
}

impl AsRef<[u8]> for RegContig {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl PartialEq for RegContig {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl Ord for RegContig {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_bytes().cmp(other.to_bytes())
    }
}

impl PartialOrd for RegContig {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for RegContig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state)
    }
}

impl RegContig {
    #[inline]
    pub fn is_null_terminated(&self) -> bool {
        self.inner.last().map(|c| *c == 0).unwrap_or(false)
    }

    pub fn parse_from_u8_slice(s: &[u8]) -> Result<(&Self, &[u8], bool), HtsError> {
        if let Some(cap) = RE_CONTIG1.captures(s).or_else(|| RE_CONTIG2.captures(s)) {
            if let (Some(c), Some(r)) = (cap.get(1), cap.get(3)) {
                let bytes = c.as_bytes();
                let ctg = unsafe { &*(bytes as *const [u8] as *const Self) };
                Ok((ctg, r.as_bytes(), cap.get(2).is_some()))
            } else {
                Err(HtsError::InvalidContig)
            }
        } else {
            Err(HtsError::InvalidContig)
        }
    }

    pub fn from_u8_slice(s: &[u8]) -> Result<&Self, HtsError> {
        match s.iter().position(|&b| b == 0) {
            Some(pos) if pos + 1 == s.len() => Ok(unsafe { &*(s as *const [u8] as *const Self) }),
            Some(_) => Err(HtsError::InvalidContig),
            None => Ok(unsafe { &*(s as *const [u8] as *const Self) }),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // This is safe because every contig name has to match [RE_CONTIG1] or [RE_CONTIG2],
        // so we know that they must only contain valid 7 bit ascii which is also valid utf8
        unsafe { str::from_utf8_unchecked(self.to_bytes()) }
    }

    #[inline]
    pub fn to_cstr(&self) -> Option<&CStr> {
        self.to_bytes_eith_null()
            .map(|s| unsafe { CStr::from_bytes_with_nul_unchecked(s) })
    }

    #[inline]
    pub fn to_bytes(&self) -> &[u8] {
        let l = self.inner.len();
        if l > 0 && self.inner[l - 1] == 0 {
            &self.inner[..l - 1]
        } else {
            &self.inner
        }
    }

    #[inline]
    pub fn to_bytes_eith_null(&self) -> Option<&[u8]> {
        if self.is_null_terminated() {
            Some(&self.inner)
        } else {
            None
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.to_bytes().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    fn make_open_region<T: IdMap + SeqId>(
        &self,
        h: &T,
        x: HtsPos,
    ) -> Result<HtsRegion, HtsError> {
        self.make_region(h, x, None)
    }

    fn make_closed_region<T: IdMap + SeqId>(
        &self,
        h: &T,
        x: HtsPos,
        y: HtsPos,
    ) -> Result<HtsRegion, HtsError> {
        self.make_region(h, x, Some(y))
    }

    fn make_full_region<T: IdMap + SeqId>(&self, h: &T) -> Result<HtsRegion, HtsError> {
        self.make_region(h, 0, None)
    }

    fn make_region<T: IdMap + SeqId>(
        &self,
        h: &T,
        x: HtsPos,
        y: Option<HtsPos>,
    ) -> Result<HtsRegion, HtsError> {
        
        let i = self.to_cstr().and_then(|s| h.seq_id(s))
            .or_else(|| {
            h.seq_id(CString::new(self.as_str()).expect("Bad contig name").as_c_str())
        }).ok_or(HtsError::UnknownContig(self.to_cstr().unwrap_or(c"?").to_owned()))?;
        // We panic here because this indicates an internal error
        let len = h.seq_len(i).expect("Missing length") as HtsPos;

        let y = y.unwrap_or(len);
        if y <= x || y > len {
            Err(HtsError::InvalidRegion)
        } else {
            Ok(HtsRegion::new(i as c_int, x, y))
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for &'a RegContig {
    type Error = HtsError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        RegContig::parse_from_u8_slice(value).map(|(r, _, _)| r)
    }
}

impl<'a, const N: usize> TryFrom<&'a [u8; N]> for &'a RegContig {
    type Error = HtsError;

    fn try_from(value: &'a [u8; N]) -> Result<Self, Self::Error> {
        RegContig::parse_from_u8_slice(value).map(|(r, _, _)| r)
    }
}

impl<'a> TryFrom<&'a CStr> for &'a RegContig {
    type Error = HtsError;

    fn try_from(value: &'a CStr) -> Result<Self, Self::Error> {
        RegContig::parse_from_u8_slice(value.to_bytes()).map(|(r, _, _)| r)
    }
}

impl<'a> TryFrom<&'a str> for &'a RegContig {
    type Error = HtsError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        RegContig::parse_from_u8_slice(value.as_bytes()).map(|(r, _, _)| r)
    }
}

impl fmt::Display for RegContig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.as_str();

        if f.alternate() && s.contains(':') {
            write!(f, "{{{s}}}")
        } else {
            write!(f, "{s}")
        }
    }
}

#[derive(Debug)]
pub enum Region {
    Contig(RegionContig),
    Open(RegionContig, usize),
    Closed(RegionContig, usize, NonZero<usize>),
    All,
    Unmapped,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contig(a) => write!(f, "{a}"),
            Self::Open(a, x) => write!(f, "{a}:{}-", x + 1),
            Self::Closed(a, x, y) if *x == 0 => write!(f, "{a}:-{y}"),
            Self::Closed(a, x, y) => write!(f, "{a}:{}-{y}", x + 1),
            Self::Unmapped => write!(f, "*"),
            Self::All => write!(f, "."),
        }
    }
}

impl Region {
    pub fn from_reg(reg: &Reg) -> Self {
        match reg {
            Reg::Contig(a) => Self::Contig((*a).to_owned()),
            Reg::Open(a, x) => Self::Open((*a).to_owned(), *x),
            Reg::Closed(a, x, y) => Self::Closed((*a).to_owned(), *x, *y),
            Reg::Unmapped => Self::Unmapped,
            Reg::All => Self::All,
        }
    }

    pub fn to_reg<'a>(&'a self) -> Reg<'a> {
        match self {
            Self::Contig(a) => Reg::Contig(a.as_ref()),
            Self::Open(a, x) => Reg::Open(a.as_ref(), *x),
            Self::Closed(a, x, y) => Reg::Closed(a.as_ref(), *x, *y),
            Self::Unmapped => Reg::Unmapped,
            Self::All => Reg::All,
        }
    }
    pub fn make_htslib_region<T: IdMap + SeqId>(&self, h: &T) -> Result<HtsRegion, HtsError> {
        match self {
            Self::Contig(c) => c.make_full_region(h),
            Self::Open(c, x) => c.make_open_region(h, *x as HtsPos),
            Self::Closed(c, x, y) => c.make_closed_region(h, *x as HtsPos, y.get() as HtsPos),
            Self::All => Ok(HtsRegion::new(HTS_IDX_START, 0, 1)),
            Self::Unmapped => Ok(HtsRegion::new(HTS_IDX_NOCOOR, 0, 1)),
        }
    }
}

impl RegCtgName for Region {
    #[inline]
    fn contig_name(&self) -> &str {
        match self {
            Self::Contig(s) | Self::Open(s, _) | Self::Closed(s, _, _) => s.as_str(),
            Self::All => ".",
            Self::Unmapped => "*",
        }
    }
}

impl RegCoords for Region {
    #[inline]
    fn coords(&self) -> (Option<HtsPos>, Option<HtsPos>) {
        match self {
            Self::Closed(_, a, b) => (Some(*a as HtsPos), Some(b.get() as HtsPos)),
            Self::Open(_, a) => (Some(*a as HtsPos), None),
            _ => (None, None),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Reg<'a> {
    Contig(&'a RegContig),
    Open(&'a RegContig, usize),
    Closed(&'a RegContig, usize, NonZero<usize>),
    All,
    Unmapped,
}

impl fmt::Display for Reg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reg::Contig(a) => write!(f, "{a}"),
            Reg::Open(a, x) => write!(f, "{a}:{}-", x + 1),
            Reg::Closed(a, x, y) if *x == 0 => write!(f, "{a}:-{y}"),
            Reg::Closed(a, x, y) => write!(f, "{a}:{}-{y}", x + 1),
            Reg::Unmapped => write!(f, "*"),
            Reg::All => write!(f, "."),
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for Reg<'a> {
    type Error = HtsError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::from_u8_slice(value)
    }
}

impl<'a, const N: usize> TryFrom<&'a [u8; N]> for Reg<'a> {
    type Error = HtsError;

    fn try_from(value: &'a [u8; N]) -> Result<Self, Self::Error> {
        Self::from_u8_slice(value)
    }
}

impl<'a> TryFrom<&'a CStr> for Reg<'a> {
    type Error = HtsError;

    fn try_from(value: &'a CStr) -> Result<Self, Self::Error> {
        Self::from_u8_slice(value.to_bytes())
    }
}

impl<'a> TryFrom<&'a str> for Reg<'a> {
    type Error = HtsError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_u8_slice(value.as_bytes())
    }
}

impl<'a> Reg<'a> {
    pub fn new_region(s: &'a CStr, mut coords: Option<&RegionCoords>) -> Result<Self, HtsError> {
        let ctg = RegContig::from_u8_slice(s.to_bytes_with_nul())?;
        Ok(if let Some(rc) = coords.take() {
            match (rc.start(), rc.end()) {
                (x, Some(y)) => Self::Closed(ctg, x as usize, NonZero::new(y as usize).unwrap()),
                (x, None) => Self::Open(ctg, x as usize),
            }
        } else {
            Self::Contig(ctg)
        })
    }

    pub fn from_u8_slice(s: &'a [u8]) -> Result<Self, HtsError> {
        match s {
            b"." => Ok(Self::All),
            b"*" => Ok(Self::Unmapped),
            _ => {
                let (ctg, s, colon) = RegContig::parse_from_u8_slice(s)?;
                match (colon, s) {
                    (_, &[]) => Ok(Self::Contig(ctg)),
                    (false, _) => Err(HtsError::TrailingGarbage),
                    (true, s) => Self::parse_range(s, ctg),
                }
            }
        }
    }

    #[inline]
    pub fn to_owned(&self) -> Region {
        Region::from_reg(self)
    }

    #[inline]
    pub fn to_region(&self) -> Region {
        Region::from_reg(self)
    }

    fn parse_range(s: &[u8], ctg: &'a RegContig) -> Result<Self, HtsError> {
        let mk_nz = |i: i64| unsafe { NonZero::new_unchecked(i as usize) };

        let (x, s) = parse_decimal(s, false)?;
        let s = skip_space(s);
        match (x, s) {
            // Note that we can use NonZero::new_unchecked as we have just verifed that x < 0 so -x > 0
            (x, &[]) if x < 0 => Ok(Self::Closed(ctg, 0, mk_nz(-x))),
            (x, &[]) | (x, b"-") => Ok(Self::Open(ctg, (x - 1).max(0) as usize)),
            (x, s) if s[0] == b'-' => match parse_decimal(&s[1..], false)? {
                (y, &[]) if y <= x => Err(HtsError::InvalidRegion),
                (y, &[]) => Ok(Self::Closed(ctg, (x - 1).max(0) as usize, mk_nz(y))),
                (_, _) => Err(HtsError::TrailingGarbage),
            },
            (_, _) => Err(HtsError::TrailingGarbage),
        }
    }

    pub fn parse_bed_from_str(s: &'a str) -> Result<Self, HtsError> {
        Self::parse_bed_from_u8_slice(s.as_bytes())
    }

    pub fn parse_bed_from_u8_slice(s: &'a [u8]) -> Result<Self, HtsError> {
        let (ctg, s, _) = RegContig::parse_from_u8_slice(s)?;
        if s.is_empty() {
            Ok(Self::Contig(ctg))
        } else {
            Self::parse_bed_range(s, ctg)
        }
    }

    fn parse_bed_range(s: &[u8], ctg: &'a RegContig) -> Result<Self, HtsError> {
        let mk_nz = |i: i64| unsafe { NonZero::new_unchecked(i as usize) };

        let mut iter = s.split(|c| *c == b'\t');
        let x = match iter.next().map(|s| parse_decimal(s, true)) {
            Some(Err(e)) => return Err(HtsError::ParseInumError(e)),
            Some(Ok((x, &[]))) => x,
            _ => return Err(HtsError::TrailingGarbage),
        };
        let y = match iter.next().map(|s| parse_decimal(s, true)) {
            Some(Err(e)) => return Err(HtsError::ParseInumError(e)),
            Some(Ok((y, &[]))) => NonZero::new(y as usize),
            None => None,
            _ => return Err(HtsError::TrailingGarbage),
        };

        match (x, y) {
            (x, None) => Ok(Self::Closed(ctg, x as usize, mk_nz(x + 1))),
            (x, Some(y)) => Ok(Self::Closed(ctg, x as usize, y)),
        }
    }

    pub fn new_all() -> Self {
        Self::All
    }

    pub fn new_unmapped() -> Self {
        Self::Unmapped
    }
    
    pub fn reg_contig(&self) -> Option<&'a RegContig> {
        match self {
            Self::Contig(s) | Self::Open(s, _) | Self::Closed(s, _, _) => Some(*s),
            _ => None,
        }
    }
    
    pub fn make_htslib_region<T: IdMap + SeqId>(&self, h: &T) -> Result<HtsRegion, HtsError> {
        match self {
            Self::Contig(c) => c.make_full_region(h),
            Self::Open(c, x) => c.make_open_region(h, *x as HtsPos),
            Self::Closed(c, x, y) => c.make_closed_region(h, *x as HtsPos, y.get() as HtsPos),
            Self::All => Ok(HtsRegion::new(HTS_IDX_START, 0, 1)),
            Self::Unmapped => Ok(HtsRegion::new(HTS_IDX_NOCOOR, 0, 1)),
        }
    }
    
    pub fn to_cstr(&self) -> Option<&CStr> {
        match self {
            Self::Contig(s) | Self::Open(s, _) | Self::Closed(s, _, _) => s.to_cstr(),
            Self::All => Some(c"."),
            Self::Unmapped => Some(c"*"),
        }
    }
}

impl RegCtgName for Reg<'_> {
    #[inline]
    fn contig_name(&self) -> &str {
        match self {
            Self::Contig(s) | Self::Open(s, _) | Self::Closed(s, _, _) => s.as_str(),
            Self::All => ".",
            Self::Unmapped => "*",
        }
    }
}

impl RegCoords for Reg<'_> {
    #[inline]
    fn coords(&self) -> (Option<HtsPos>, Option<HtsPos>) {
        match self {
            Self::Closed(_, a, b) => (Some(*a as HtsPos), Some(b.get() as HtsPos)),
            Self::Open(_, a) => (Some(*a as HtsPos), None),
            _ => (None, None),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;
    use std::num::NonZero;

    #[test]
    fn test_parse_reg_contig() {
        let (ctg, s, colon) = RegContig::parse_from_u8_slice(b"chr5:1.2M-1.43M").unwrap();
        assert_eq!(ctg.as_str(), "chr5");
        assert_eq!(s, b"1.2M-1.43M");
        assert!(colon);

        let rctg = ctg.to_owned();
        let ctg1 = rctg.as_ref();
        assert_eq!(ctg.as_str(), "chr5");

        let (ctg, s, colon) = RegContig::parse_from_u8_slice(b"chr5").unwrap();
        assert!(s.is_empty());
        assert!(!colon);

        let (ctg, s, colon) = RegContig::parse_from_u8_slice(b"{chr5:1}:1.2M-1.43M").unwrap();
        assert_eq!(ctg.as_str(), "chr5:1");
        assert_eq!(s, b"1.2M-1.43M");
        assert!(colon);
    }

    #[test]
    fn test_parse_reg() {
        let reg = Reg::try_from("chr5:1.2M-1.43M").unwrap();
        eprintln!("{reg}");
        assert_eq!(reg.contig_name(), "chr5");
        let y: NonZero<usize> = NonZero::new(1430000).unwrap();
        assert!(matches!(reg, Reg::Closed(_, 1199999, y)));
        assert!(matches!(reg.coords(), (Some(1199999), Some(1430000))));

        let reg = Reg::try_from("chr7.1").unwrap();
        eprintln!("{reg}");
        assert_eq!(reg.contig_name(), "chr7.1");
        assert!(matches!(reg, Reg::Contig(_)));

        let reg = Reg::try_from("chrX:1.234m").unwrap();
        eprintln!("{reg}");
        assert!(matches!(reg, Reg::Open(_, 1233999)));

        let reg = Reg::try_from("chrX:1000-500");
        assert!(matches!(reg, Err(HtsError::InvalidRegion)));

        let reg = Reg::try_from(b"chrX:1000,");
        assert!(matches!(reg, Err(HtsError::TrailingGarbage)));

        let reg = Reg::try_from(b"*").unwrap();
        eprintln!("{reg}");
        assert!(matches!(reg, Reg::Unmapped));
    }

    #[test]
    fn test_parse_bed() {
        let reg = Reg::parse_bed_from_str("chr5\t1199999\t1430000").unwrap();
        eprintln!("{reg}");
        assert_eq!(reg.contig_name(), "chr5");
        let y: NonZero<usize> = NonZero::new(1430000).unwrap();
        assert!(matches!(reg, Reg::Closed(_, 1199999, y)));
    }
}
