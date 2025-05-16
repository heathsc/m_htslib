use std::convert::TryFrom;

pub trait LeBytes
where
    Self: Sized,
{
    type OutArray: AsRef<[u8]> + for<'a> TryFrom<&'a [u8]>;

    fn to_le(&self) -> Self::OutArray;

    fn from_le(bytes: Self::OutArray) -> Self;
}

impl LeBytes for u8 {
    type OutArray = [u8; 1];

    fn to_le(&self) -> Self::OutArray {
        [*self]
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        bytes[0]
    }
}

impl LeBytes for i8 {
    type OutArray = [u8; 1];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        bytes[0] as i8
    }
}

impl LeBytes for u16 {
    type OutArray = [u8; 2];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i16 {
    type OutArray = [u8; 2];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for u32 {
    type OutArray = [u8; 4];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i32 {
    type OutArray = [u8; 4];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for u64 {
    type OutArray = [u8; 8];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for i64 {
    type OutArray = [u8; 8];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for f32 {
    type OutArray = [u8; 4];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl LeBytes for f64 {
    type OutArray = [u8; 8];

    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }

    fn from_le(bytes: Self::OutArray) -> Self {
        Self::from_le_bytes(bytes)
    }
}
