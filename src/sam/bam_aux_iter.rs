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

pub struct AuxIntArray<'a> {
    val_type: BamAuxTagType,
    data: &'a [u8],
}

pub struct AuxFloatArray<'a> {
    val_type: BamAuxTagType,
    data: &'a [u8],
}

struct AuxArray<'a, T> {
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
            let x = T::from_le(
                self.data[..n]
                    .try_into()
                    .map_err(|_| AuxError::InternalError)
                    .unwrap(),
            );
            self.data = &self.data[n..];
            Some(x)
        }
    }
}

pub struct AuxCharArray<'a> {
    data: &'a [u8],
}

pub enum BamAuxVal<'a> {
    Char(u8),
    Int(i64),
    Float32(f32),
    Float64(f64),
    String(&'a CStr),
    HexString(HexString<'a>),
    CharArray(AuxCharArray<'a>),
    IntArray(AuxIntArray<'a>),
}

impl <'a>BamAuxVal<'a> {
    fn from_u8_slice(s: &'a [u8]) -> Result<Self, AuxError> {
        let l = s.len();
        if l < 2 {
            Err(AuxError::CorruptBamTag)
        } else {
            match s[0] {
                b'A' => Ok(Self::Char(s[1])),
                b'c' => Ok(Self::Int((s[1] as i8) as i64)),
                b'C' => Ok(Self::Int(s[1] as i64)),
                
                
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
