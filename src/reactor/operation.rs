//! I/O operation and interest flag semantics for the reactor.

use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IOOperation {
    Read,
    Write,
    Accept,
    Connect,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InterestSet(u8);

impl InterestSet {
    pub const NONE: Self = Self(0);
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const ACCEPT: Self = Self(1 << 2);
    pub const CONNECT: Self = Self(1 << 3);
    pub const ERROR: Self = Self(1 << 4);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub const fn from_operation(op: IOOperation) -> Self {
        match op {
            IOOperation::Read => Self::READ,
            IOOperation::Write => Self::WRITE,
            IOOperation::Accept => Self::ACCEPT,
            IOOperation::Connect => Self::CONNECT,
            IOOperation::Error => Self::ERROR,
        }
    }
}

impl fmt::Debug for InterestSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("InterestSet(NONE)");
        }
        let mut parts = Vec::new();
        if self.contains(Self::READ) {
            parts.push("READ");
        }
        if self.contains(Self::WRITE) {
            parts.push("WRITE");
        }
        if self.contains(Self::ACCEPT) {
            parts.push("ACCEPT");
        }
        if self.contains(Self::CONNECT) {
            parts.push("CONNECT");
        }
        if self.contains(Self::ERROR) {
            parts.push("ERROR");
        }
        write!(f, "InterestSet({})", parts.join("|"))
    }
}

impl From<IOOperation> for InterestSet {
    fn from(value: IOOperation) -> Self {
        Self::from_operation(value)
    }
}

impl BitOr for InterestSet {
    type Output = InterestSet;

    fn bitor(self, rhs: Self) -> Self::Output {
        InterestSet(self.0 | rhs.0)
    }
}

impl BitOrAssign for InterestSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for InterestSet {
    type Output = InterestSet;

    fn bitand(self, rhs: Self) -> Self::Output {
        InterestSet(self.0 & rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_set_combines_and_checks_bits() {
        let set = InterestSet::READ | InterestSet::WRITE;
        assert!(set.contains(InterestSet::READ));
        assert!(set.contains(InterestSet::WRITE));
        assert!(set.intersects(InterestSet::WRITE | InterestSet::ERROR));
        assert!(!set.contains(InterestSet::ERROR));
    }
}
