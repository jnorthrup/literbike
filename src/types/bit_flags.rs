/// BitFlags for various protocol features
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitFlags {
    bits: u16,
}

impl BitFlags {
    pub const NONE: Self = Self { bits: 0 };
    pub const ENCRYPTED: Self = Self { bits: 1 << 0 };
}

impl BitFlags {
    pub fn new() -> Self {
        Self::NONE
    }
}