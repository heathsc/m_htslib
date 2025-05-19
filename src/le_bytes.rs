use std::{convert::TryFrom, mem::size_of};

/// A trait for numeric types that can be converted to and from a byte array in little endian order.
/// We use this to allow us to have generic methods to read and write from binary hts files (BAM/BCF etc.)
/// for different numeric types ([i8], [u16], [f32] etc.)
pub trait LeBytes
where
    Self: Sized,
{
    /// Conversions are to and from a byte array [u8; N] where N depends on the type size.
    /// We want to be able to convert the array to and from a slice, hence the AsRef and TryFrom
    /// constraints.
    type ByteArray: AsRef<[u8]> + for<'a> TryFrom<&'a [u8]>;
    
    /// Convert Self to [Self::ByteArray] in LE format
    fn to_le(&self) -> Self::ByteArray;

    /// Convert [Self::ByteArray] to Self
    fn from_le(bytes: Self::ByteArray) -> Self;
}

impl LeBytes for u8 {
    type ByteArray = [u8; size_of::<u8>()];

    fn to_le(&self) -> Self::ByteArray {
        [*self]
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        bytes[0]
    }
}

impl LeBytes for i8 {
    type ByteArray = [u8; size_of::<i8>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        bytes[0] as i8
    }
}

impl LeBytes for u16 {
    type ByteArray = [u8; size_of::<u16>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i16 {
    type ByteArray = [u8; size_of::<i16>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for u32 {
    type ByteArray = [u8; size_of::<u32>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i32 {
    type ByteArray = [u8; size_of::<i32>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for u64 {
    type ByteArray = [u8; size_of::<u64>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i64 {
    type ByteArray = [u8; size_of::<i64>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for f32 {
    type ByteArray = [u8; size_of::<f32>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for f64 {
    type ByteArray = [u8; size_of::<f64>()];

    fn to_le(&self) -> Self::ByteArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::ByteArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}
