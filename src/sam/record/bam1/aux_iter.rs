use std::{collections::HashSet, ffi::CStr, fmt, iter::FusedIterator, marker::PhantomData};

use super::bam_type_code::BamTypeCode;
use crate::{AuxError, LeBytes};

/// This holds the binary data relating to an individual aux tag from a Bam record
/// The length of the data slice is always at least 3 (2 byte tag + type)
#[derive(Debug)]
pub struct BamAuxTag<'a> {
    data: &'a [u8],
}

impl fmt::Display for BamAuxTag<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.data;
        write!(
            f,
            "{}{}:{}",
            s[0] as char,
            s[1] as char,
            self.get_val().expect("Corrupt Bam record")
        )?;
        Ok(())
    }
}

impl BamAuxTag<'_> {
    pub fn id(&self) -> Result<&str, AuxError> {
        let s = std::str::from_utf8(&self.data[..2])?;
        Ok(s)
    }

    #[inline]
    pub fn get_val(&self) -> Result<BamAuxVal, AuxError> {
        BamAuxVal::from_u8_slice(&self.data[2..])
    }

    #[inline]
    pub fn get_type(&self) -> Result<(BamAuxTagType, Option<BamAuxTagType>), AuxError> {
        Ok(match get_tag_info(self.data[2])?.0 {
            BamAuxTagType::Array => (BamAuxTagType::Array, Some(get_tag_info(self.data[3])?.0)),
            t => (t, None),
        })
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        self.data
    }

    pub fn validate(&self) -> Result<[u8; 2], AuxError> {
        // Check that we can get the tag id and value.
        // Throw away the return values and just return the id or any errors
        let id = self.id()?;
        self.get_val().map(|_| {
            let b = id.as_bytes();
            [b[0], b[1]]
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BamAuxTagType {
    Char,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
    Float64,
    String,
    HexArray,
    Array,
}

/// Get the tag type and size in bytes of the payload for different
/// BAM aux tage types. Types without explicit lengths ('Z', 'H', 'B')
/// have explicit size set to zero as we need to perform further
/// inpection to find the length
fn get_tag_info(c: u8) -> Result<(BamAuxTagType, u8), AuxError> {
    let ret = match c {
        b'A' => (BamAuxTagType::Char, 1),
        b'c' => (BamAuxTagType::Int8, 1),
        b'C' => (BamAuxTagType::UInt8, 1),
        b's' => (BamAuxTagType::Int16, 2),
        b'S' => (BamAuxTagType::UInt16, 2),
        b'i' => (BamAuxTagType::Int32, 4),
        b'I' => (BamAuxTagType::UInt32, 4),
        b'f' => (BamAuxTagType::Float32, 4),
        b'd' => (BamAuxTagType::Float64, 8),
        b'Z' => (BamAuxTagType::String, 0),
        b'H' => (BamAuxTagType::HexArray, 0),
        b'B' => (BamAuxTagType::Array, 0),
        _ => return Err(AuxError::BadBamTagFormat(c)),
    };

    Ok(ret)
}

/// Get length in bytes of Aux tag in a BAM record (including 2 character tag)
fn get_bam_tag_length(s: &[u8]) -> Result<usize, AuxError> {
    let s_len = s.len();
    if s_len < 4 {
        Err(AuxError::CorruptBamTag)
    } else {
        let (tag_type, l) = get_tag_info(s[2])?;
        if l > 0 {
            // Implicit length
            let tag_len = 3 + l as usize;
            if tag_len > s_len {
                Err(AuxError::CorruptBamTag)
            } else {
                Ok(tag_len)
            }
        } else {
            let l = match tag_type {
                BamAuxTagType::String | BamAuxTagType::HexArray => {
                    s[3..].iter().position(|c| *c == 0).map(|x| 4 + x)
                }
                BamAuxTagType::Array => {
                    let (_, l1) = get_tag_info(s[3])?;
                    if l1 == 0 || s_len < 8 {
                        return Err(AuxError::CorruptBamTag);
                    }
                    let num_elem = u32::from_le_bytes(s[4..8].try_into().unwrap());
                    let l = l1 as usize * num_elem as usize + 8;
                    if l <= s_len { Some(l) } else { None }
                }
                _ => panic!("Unexpected tag type here"),
            }
            .ok_or(AuxError::CorruptBamTag)?;

            if tag_type == BamAuxTagType::HexArray && (l & 1) != 0 {
                Err(AuxError::CorruptBamTag)
            } else {
                Ok(l)
            }
        }
    }
}

pub struct HexString<'a> {
    data: &'a [u8],
}

impl fmt::Display for HexString<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", std::str::from_utf8(self.data).unwrap())
    }
}

impl<'a> HexString<'a> {
    /// Convert a [u8] slice to a HexString from a Bam record. Note that the
    /// number of digits must be even and the slice should finish with a zero,
    /// so the slice length should be odd. An error is returned if the length is
    /// not odd or if the last element of the slice is not zero or if there is a
    /// zero anywhere in the slice apart from the last element.
    fn from_u8_slice(data: &'a [u8]) -> Result<Self, AuxError> {
        let l = data.len();
        if (l & 1) == 0 {
            Err(AuxError::OddHexDigits)
        } else if data[l - 1] != 0 {
            Err(AuxError::CorruptBamTag)
        } else if data[..l - 1].iter().any(|c| !c.is_ascii_hexdigit()) {
            Err(AuxError::IllegalHexCharacters)
        } else {
            Ok(Self { data })
        }
    }

    #[inline]
    pub fn to_cstr(&self) -> Result<&CStr, AuxError> {
        let s = CStr::from_bytes_with_nul(self.data)?;
        Ok(s)
    }

    #[inline]
    pub fn bytes(&self) -> HexIter {
        HexIter { data: self.data }
    }
}

pub struct HexIter<'a> {
    data: &'a [u8],
}

impl Iterator for HexIter<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let get_x = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'A'..=b'F' => Some(c - b'A' + 10),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };

        self.data.split_at_checked(2).and_then(|(s1, s2)| {
            self.data = s2;
            match (get_x(s1[0]), get_x(s1[1])) {
                (Some(a), Some(b)) => Some((a << 4) | b),
                _ => None,
            }
        })
    }
}

pub struct AuxArray<'a, T> {
    data: &'a [u8],
    marker: PhantomData<T>,
}

impl<T: Sized + LeBytes + BamTypeCode + fmt::Display> fmt::Display for AuxArray<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", T::type_code() as char)?;
        let n = std::mem::size_of::<T>();
        for s in self.data.chunks_exact(n) {
            let x: T = get_single_aux_val(s);
            write!(f, ",{}", x)?
        }
        Ok(())
    }
}

impl<'a, T> AuxArray<'a, T> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            marker: PhantomData,
        }
    }
}

impl<T: Sized + LeBytes> Iterator for AuxArray<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            let n = std::mem::size_of::<T>();
            assert!(self.data.len() >= n);
            let (s1, s2) = self.data.split_at(n);
            let x: T = get_single_aux_val(s1);
            self.data = s2;
            Some(x)
        }
    }
}

pub struct AuxIntArray<'a, T> {
    inner: AuxArray<'a, T>,
}

impl<T: Sized + LeBytes + Into<i64> + fmt::Display + BamTypeCode> AuxArrayIter
    for AuxIntArray<'_, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<'a, T> AuxIntArray<'a, T> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            inner: AuxArray::new(data),
        }
    }
}

#[inline(always)]
fn mk_aux_int_array<T: Sized + LeBytes + Into<i64>>(d: &[u8]) -> AuxIntArray<'_, T> {
    AuxIntArray::new(d)
}

impl<T: Sized + LeBytes + Into<i64>> Iterator for AuxIntArray<'_, T> {
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| x.into())
    }
}

#[inline]
fn get_single_aux_val<T: Sized + LeBytes>(s: &[u8]) -> T {
    T::from_le(s.try_into().map_err(|_| AuxError::InternalError).unwrap())
}

#[inline]
fn get_string_val(s: &[u8]) -> Result<&CStr, AuxError> {
    let ret = CStr::from_bytes_with_nul(s)?;
    Ok(ret)
}

pub trait AuxArrayIter: Iterator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

pub enum BamAuxVal<'a> {
    Char(u8),
    Int(i64),
    Float32(f32),
    Float64(f64),
    String(&'a CStr),
    HexString(HexString<'a>),
    CharArray(&'a [u8]),
    IntArray(Box<dyn AuxArrayIter<Item = i64> + 'a>),
    Float32Array(AuxArray<'a, f32>),
    Float64Array(AuxArray<'a, f64>),
}

impl fmt::Display for BamAuxVal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(x) => write!(f, "A:{}", *x as char)?,
            Self::Int(x) => write!(f, "i:{x}")?,
            Self::Float32(x) => write!(f, "f:{x}")?,
            Self::Float64(x) => write!(f, "d:{x}")?,
            Self::String(s) => write!(f, "Z:{}", s.to_str().unwrap())?,
            Self::HexString(hs) => write!(f, "H:{hs}")?,
            Self::CharArray(s) => write!(f, "B:A:{}", std::str::from_utf8(s).unwrap())?,
            Self::Float32Array(a) => write!(f, "B:{}", a)?,
            Self::Float64Array(a) => write!(f, "B:{}", a)?,
            Self::IntArray(a) => a.fmt(f)?,
        }

        Ok(())
    }
}

impl<'a> BamAuxVal<'a> {
    fn from_u8_slice(s: &'a [u8]) -> Result<Self, AuxError> {
        if s.len() < 2 {
            Err(AuxError::CorruptBamTag)
        } else {
            match s[0] {
                b'A' => {
                    if s[1].is_ascii_graphic() {
                        Ok(Self::Char(s[1]))
                    } else {
                        Err(AuxError::IllegalCharacters)
                    }
                }
                b'c' => Ok(Self::Int((s[1] as i8) as i64)),
                b'C' => Ok(Self::Int(s[1] as i64)),
                b's' => Ok(Self::Int(get_single_aux_val::<i16>(&s[1..]) as i64)),
                b'S' => Ok(Self::Int(get_single_aux_val::<u16>(&s[1..]) as i64)),
                b'i' => Ok(Self::Int(get_single_aux_val::<i32>(&s[1..]) as i64)),
                b'I' => Ok(Self::Int(get_single_aux_val::<u32>(&s[1..]) as i64)),
                b'f' => Ok(Self::Float32(get_single_aux_val::<f32>(&s[1..]))),
                b'd' => Ok(Self::Float64(get_single_aux_val::<f64>(&s[1..]))),
                b'Z' => Ok(Self::String(get_string_val(&s[1..])?)),
                b'H' => Ok(Self::HexString(HexString::from_u8_slice(&s[1..])?)),
                b'B' => Self::get_array_var(&s[1..]),
                _ => Err(AuxError::CorruptBamTag),
            }
        }
    }

    fn get_array_var(s: &'a [u8]) -> Result<Self, AuxError> {
        if s.len() < 5 {
            Err(AuxError::CorruptBamTag)
        } else {
            match s[0] {
                b'A' => Ok(Self::CharArray(&s[5..])),
                b'c' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<i8>(&s[5..])))),
                b'C' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<u8>(&s[5..])))),
                b's' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<i16>(&s[5..])))),
                b'S' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<u16>(&s[5..])))),
                b'i' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<i32>(&s[5..])))),
                b'I' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<i32>(&s[5..])))),
                b'f' => Ok(Self::Float32Array(AuxArray::new(&s[5..]))),
                b'd' => Ok(Self::Float64Array(AuxArray::new(&s[5..]))),
                _ => Err(AuxError::CorruptBamTag),
            }
        }
    }
}

pub fn validate_aux_slice(data: &[u8], hset: &mut HashSet<[u8; 2]>) -> Result<(), AuxError> {
    hset.clear();
    for v in BamAuxIter::new(data) {
        let id = v.and_then(|val| val.validate())?;
        if !hset.insert(id) {
            return Err(AuxError::DuplicateTagId(id[0] as char, id[1] as char));
        }
    }
    Ok(())
}

pub struct BamAuxIter<'a> {
    data: &'a [u8],
}

impl<'a> BamAuxIter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> Iterator for BamAuxIter<'a> {
    type Item = Result<BamAuxTag<'a>, AuxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            Some(get_bam_tag_length(self.data).map(|l| {
                let (data, s) = self.data.split_at(l);
                self.data = s;
                BamAuxTag { data }
            }))
        }
    }
}

impl FusedIterator for BamAuxIter<'_> {}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    use crate::{
        SamError,
        hts::HtsFile,
        kstring::KString,
        sam::{BamAuxVal, SamHdrLine},
        sam::{BamRec, CigarBuf, SamHdr, SamParser, SequenceIter},
        sam_hdr_line,
    };

    fn make_header<'a>() -> Result<SamHdr<'a>, SamError> {
        let mut hdr = SamHdr::new();
        hdr.add_lines(c"@HD\tVN:1.6\tSO:coordinate")?;
        assert_eq!(hdr.length().unwrap(), 25);
        let nl = sam_hdr_line!("SQ", "SN", "chr1", "LN", "1009800")?;
        hdr.add_line(&nl)?;

        Ok(hdr)
    }

    #[test]
    fn test_tags() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();

        p.parse(&mut b, &mut hdr, b"read_id1\t147\tchr1\t412\t49\t11M\t=\t193\t-380\tCTGCAATACGC\tAAFJFFBCAFF\txa:Z:Hello world\txb:i:666")?;

        let mut tags = b.aux_tags();

        // Check first tag (Z)
        let tag = tags.next().unwrap()?;
        assert_eq!(tag.id()?, "xa");
        let x = tag.get_val()?;
        if let BamAuxVal::String(s) = x {
            assert_eq!(s, c"Hello world")
        } else {
            panic!("Wrong tag type")
        }

        // Check second tag (i)
        let tag = tags.next().unwrap()?;
        assert_eq!(tag.id()?, "xb");
        let x = tag.get_val()?;
        if let BamAuxVal::Int(i) = x {
            assert_eq!(i, 666)
        } else {
            panic!("Wrong tag type")
        }
        Ok(())
    }

    #[test]
    fn test_hex_tags() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();

        p.parse(
            &mut b,
            &mut hdr,
            b"read_id1\t4\t*\t0\t0\t*\t*\t0\t0\t*\t*\txa:H:1A93AF\txb:H:",
        )?;

        let mut it = b.aux_tags();

        let tag = it.next().unwrap()?;
        assert_eq!(tag.id()?, "xa");
        let x = tag.get_val()?;
        if let BamAuxVal::HexString(s) = x {
            assert_eq!(s.to_cstr()?, c"1A93AF");
            let v: Vec<_> = s.bytes().collect();
            assert_eq!(&v, &[0x1a, 0x93, 0xaf]);
        } else {
            panic!("Wrong tag type")
        }

        Ok(())
    }

    #[test]
    fn test_array_tags() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();
        
        p.parse(
            &mut b,
            &mut hdr,
            b"read_id1\t4\t*\t0\t0\t*\t*\t0\t0\t*\t*\txa:B:c,7782,43,-999,1023,42",
        )?;
        let mut it = b.aux_tags();
        
        let tag = it.next().unwrap()?;
        assert_eq!(tag.id()?, "xa");
        let ret = tag.get_type()?;
        assert_eq!(ret, (BamAuxTagType::Array, Some(BamAuxTagType::Int16)));

        let x = tag.get_val()?;
        if let BamAuxVal::IntArray(s) = x {
            let v: Vec<_> = s.collect();
            assert_eq!(&v, &[7782, 43, -999, 1023, 42]);
        } else {
            panic!("Wrong tag type")
        }

        Ok(())
    }
    #[test]
    fn test_array_tags2() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();

        p.parse(
            &mut b,
            &mut hdr,
            b"read_id1\t4\t*\t0\t0\t*\t*\t0\t0\t*\t*\txa:B:f,1.5,7.0e-2,6,8.2",
        )?;
        let mut it = b.aux_tags();

        let tag = it.next().unwrap()?;
        assert_eq!(tag.id()?, "xa");
        let ret = tag.get_type()?;
        assert_eq!(ret, (BamAuxTagType::Array, Some(BamAuxTagType::Float32)));

        let x = tag.get_val()?;
        if let BamAuxVal::Float32Array(s) = x {
            let v: Vec<_> = s.collect();
            assert_eq!(&v, &[1.5, 0.07, 6.0, 8.2]);
        } else {
            panic!("Wrong tag type")
        }
        Ok(())
    }

    #[test]
    fn test_get_tag() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();
         
        p.parse(
            &mut b,
            &mut hdr,
            b"read_id1\t4\t*\t0\t0\t*\t*\t0\t0\t*\t*\txa:i:4\txb:Z:Hi\txc:A:v\txd:B:c,41,8",
        )?;

        let tag = b.get_tag("xc")?.expect("Did not find xc tag");
        assert_eq!(tag.id()?, "xc");
        let ret = tag.get_type()?;
        assert_eq!(ret, (BamAuxTagType::Char, None));

        let x = tag.get_val()?;
        if let BamAuxVal::Char(s) = x {
            assert_eq!(s, b'v');
        } else {
            panic!("Wrong tag type")
        }

        // RG tag does not exist in this record
        assert!(b.get_tag("RG")?.is_none());

        drop(x);

        let n = b.del_tags(&["xb", "xc"])?;
        assert_eq!(n, 2);

        b.del_tag("xd")?;

        let t = b.aux_tags().next().unwrap()?;
        assert_eq!(t.id()?, "xa");

        Ok(())
    }

    #[test]
    fn test_tag_display() -> Result<(), SamError> {
        let mut hdr = make_header()?;

        let mut p = SamParser::new();
        let mut b = BamRec::new();

        p.parse(
            &mut b,
            &mut hdr,
            b"read_id1\t4\t*\t0\t0\t*\t*\t0\t0\t*\t*\txa:B:f,1.5,7.0e-2,6,8.2\tRG:Z:ReadGroup2\txb:i:-675",
        )?;

        let mut it = b.aux_tags();

        let tag = it.next().unwrap()?;
        let s = format!("{tag}");
        assert_eq!(&s, "xa:B:f,1.5,0.07,6,8.2");

        let tag = it.next().unwrap()?;
        let s = format!("{tag}");
        assert_eq!(&s, "RG:Z:ReadGroup2");

        let tag = it.next().unwrap()?;
        let s = format!("{tag}");
        assert_eq!(&s, "xb:i:-675");

        Ok(())
    }
}
