use std::{ffi::CStr, iter::FusedIterator, marker::PhantomData};

use crate::{AuxError, LeBytes};

/// This holds the binary data relating to an individual aux tag from a Bam record
/// The length of the data slice is always at least 3 (2 byte tag + type)
#[derive(Debug)]
pub struct BamAuxTagData<'a> {
    data: &'a [u8],
}

impl BamAuxTagData<'_> {
    pub fn id(&self) -> Result<&str, AuxError> {
        let s = std::str::from_utf8(&self.data[..2])?;
        Ok(s)
    }

    #[inline]
    pub fn get_val(&self) -> Result<BamAuxVal, AuxError> {
        BamAuxVal::from_u8_slice(&self.data[2..])
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BamAuxTagType {
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
        } else {
            match data.iter().position(|c| *c == 0) {
                Some(x) if x + 1 == l => Ok(Self { data }),
                Some(_) | None => Err(AuxError::CorruptBamTag),
            }
        }
    }
    
    #[inline]
    pub fn to_cstr(&self) -> Result<&CStr, AuxError> {
        let s = CStr::from_bytes_with_nul(self.data)?;
        Ok(s)
    } 
}

pub struct AuxArray<'a, T> {
    data: &'a [u8],
    marker: PhantomData<T>,
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
    inner: AuxArray<'a, T>
}

impl <'a, T> AuxIntArray<'a, T> {
    fn new(data: &'a [u8]) -> Self {
        Self { inner: AuxArray::new(data) }
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

pub enum BamAuxVal<'a> {
    Char(u8),
    Int(i64),
    Float32(f32),
    Float64(f64),
    String(&'a CStr),
    HexString(HexString<'a>),
    CharArray(&'a [u8]),
    IntArray(Box<dyn Iterator<Item = i64> + 'a>),
    Float32Array(AuxArray<'a, f32>),
    Float64Array(AuxArray<'a, f64>),
}

impl<'a> BamAuxVal<'a> {
    fn from_u8_slice(s: &'a [u8]) -> Result<Self, AuxError> {
        if s.len() < 2 {
            Err(AuxError::CorruptBamTag)
        } else {
            match s[0] {
                b'A' => Ok(Self::Char(s[1])),
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
                b'c' => Ok(Self::IntArray(Box::new(mk_aux_int_array::<u8>(&s[5..])))),
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

pub struct BamAuxIter<'a> {
    data: &'a [u8],
}

impl<'a> BamAuxIter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> Iterator for BamAuxIter<'a> {
    type Item = Result<BamAuxTagData<'a>, AuxError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            Some(get_bam_tag_length(self.data).map(|l| {
                let (data, s) = self.data.split_at(l);
                self.data = s;
                BamAuxTagData { data }
            }))
        }
    }
}

impl FusedIterator for BamAuxIter<'_> {}
