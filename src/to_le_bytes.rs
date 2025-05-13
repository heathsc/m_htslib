use std::ops::RangeInclusive;

pub trait ToLeBytes where Self: Sized {
    type OutArray: AsRef<[u8]>;
    
    fn to_le(&self) -> Self::OutArray;
}

impl ToLeBytes for u8 {
    type OutArray = [u8; 1];
    
    fn to_le(&self) -> Self::OutArray {
        [*self]
    }
}

impl ToLeBytes for i8 {
    type OutArray = [u8; 1];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for u16 {
    type OutArray = [u8; 2];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for i16 {
    type OutArray = [u8; 2];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for u32 {
    type OutArray = [u8; 4];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for i32 {
    type OutArray = [u8; 4];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for u64 {
    type OutArray = [u8; 8];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for i64 {
    type OutArray = [u8; 8];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}


impl ToLeBytes for f32 {
    type OutArray = [u8; 4];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}

impl ToLeBytes for f64 {
    type OutArray = [u8; 8];
    
    fn to_le(&self) -> Self::OutArray {
        self.to_le_bytes()
    }
}