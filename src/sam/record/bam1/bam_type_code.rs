pub trait BamTypeCode {
    fn type_code() -> u8;
}

impl BamTypeCode for u8 {
    fn type_code() -> u8 {
        b'C'
    }
}

impl BamTypeCode for i8 {
    fn type_code() -> u8 {
        b'c'
    }
}

impl BamTypeCode for u16 {
    fn type_code() -> u8 {
        b'S'
    }
}

impl BamTypeCode for i16 {
    fn type_code() -> u8 {
        b's'
    }
}

impl BamTypeCode for u32 {
    fn type_code() -> u8 {
        b'I'
    }
}

impl BamTypeCode for i32 {
    fn type_code() -> u8 {
        b'i'
    }
}

impl BamTypeCode for f32 {
    fn type_code() -> u8 {
        b'f'
    }
}

impl BamTypeCode for f64 {
    fn type_code() -> u8 {
        b'd'
    }
}
